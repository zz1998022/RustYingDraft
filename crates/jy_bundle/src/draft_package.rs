use anyhow::{bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde_json::{json, Value};
use uuid::Uuid;

use crate::api::{
    BundleInspection, ImportBundleOptions, ImportBundleProgress, ImportBundleSummary,
};
use crate::asset::{resolve_asset_source, AssetKind, AssetSourceSpec, AssetSourceType};
use crate::fs_util::{
    copy_dir_all, ensure_output_dir_ready, normalize_path_for_draft, utf8_path_buf,
};
use crate::manifest::{BundleManifest, DraftAssetBinding, DraftMatchKey};
use crate::source::PreparedSource;

pub(crate) fn import_draft_package<F>(
    options: &ImportBundleOptions,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
    progress: &mut F,
) -> Result<ImportBundleSummary>
where
    F: FnMut(ImportBundleProgress),
{
    if bundle.match_key != DraftMatchKey::Name {
        bail!("unsupported match_key for draft_package");
    }
    if bundle.assets.is_empty() {
        bail!("draft_package requires bundle.assets");
    }

    let source_draft_dir = prepared
        .bundle_root
        .join(bundle.draft_dir.as_deref().unwrap_or("draft"));
    if !source_draft_dir.exists() {
        bail!("draft source directory not found: {source_draft_dir}");
    }

    ensure_output_dir_ready(&options.output)?;
    copy_dir_all(&source_draft_dir, &options.output)?;

    let cache_root = utf8_path_buf(prepared.temp_dir.path().to_path_buf())?;

    let mut replacements = Vec::new();
    for (index, asset) in bundle.assets.iter().enumerate() {
        let replacement = resolve_draft_binding(
            asset,
            &prepared.bundle_root,
            bundle.assets_dir.as_deref(),
            &cache_root,
            &options.output,
            index,
            progress,
        )?;
        replacements.push(replacement);
    }

    let final_name = options
        .name_override
        .clone()
        .or_else(|| bundle.project_name.clone())
        .unwrap_or_else(|| "imported_bundle".to_string());

    rewrite_draft_package_snapshots(&options.output, &replacements)?;
    rewrite_meta_info(&options.output, &final_name)?;

    let draft_json: Value = serde_json::from_str(&std::fs::read_to_string(
        options.output.join("draft_content.json"),
    )?)?;
    let track_count = draft_json
        .get("tracks")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let video_material_count = draft_json
        .pointer("/materials/videos")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let audio_material_count = draft_json
        .pointer("/materials/audios")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let duration = draft_json
        .get("duration")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    Ok(ImportBundleSummary {
        source: options.source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "draft_package".to_string(),
        timeline_file: None,
        source_draft_dir: Some(source_draft_dir.as_str().to_string()),
        draft_dir: options.output.as_str().to_string(),
        project_id: bundle
            .project_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().as_simple().to_string()),
        name: final_name,
        duration,
        track_count,
        asset_count: bundle.assets.len(),
        video_material_count,
        audio_material_count,
    })
}

pub(crate) fn inspect_draft_package(
    source: &Utf8Path,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
) -> Result<BundleInspection> {
    let source_draft_dir = prepared
        .bundle_root
        .join(bundle.draft_dir.as_deref().unwrap_or("draft"));
    let draft_json: Value = serde_json::from_str(&std::fs::read_to_string(
        source_draft_dir.join("draft_content.json"),
    )?)?;
    let track_count = draft_json
        .get("tracks")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);

    Ok(BundleInspection {
        source: source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "draft_package".to_string(),
        timeline_file: None,
        source_draft_dir: Some(source_draft_dir.as_str().to_string()),
        project_id: bundle.project_id.clone(),
        project_name: bundle.project_name.clone(),
        asset_count: bundle.assets.len(),
        track_count,
        asset_kinds: bundle
            .assets
            .iter()
            .map(|asset| asset.kind.as_str().to_string())
            .collect(),
    })
}

fn resolve_draft_binding<F>(
    asset: &DraftAssetBinding,
    bundle_root: &Utf8Path,
    assets_dir: Option<&str>,
    cache_root: &Utf8Path,
    output_draft_dir: &Utf8Path,
    material_index: usize,
    progress: &mut F,
) -> Result<DraftMaterialReplacement>
where
    F: FnMut(ImportBundleProgress),
{
    let source = AssetSourceSpec {
        source_type: AssetSourceType::BundlePath,
        path: Some(asset.relative_path.clone()),
        url: None,
    };
    let resolved_path = resolve_asset_source(
        &asset.match_value,
        &source,
        bundle_root,
        assets_dir,
        cache_root,
        asset.kind,
        progress,
    )?;
    let localized_path =
        localize_draft_package_asset(&resolved_path, output_draft_dir, asset.kind, material_index)?;

    Ok(DraftMaterialReplacement {
        kind: asset.kind,
        name: asset.match_value.clone(),
        path: normalize_path_for_draft(&localized_path),
    })
}

#[derive(Debug, Clone)]
struct DraftMaterialReplacement {
    kind: AssetKind,
    name: String,
    path: String,
}

fn localize_draft_package_asset(
    source: &Utf8Path,
    output_draft_dir: &Utf8Path,
    kind: AssetKind,
    index: usize,
) -> Result<Utf8PathBuf> {
    if !source.exists() || !source.is_file() || source.starts_with(output_draft_dir) {
        return Ok(source.to_path_buf());
    }

    let (category, prefix) = match kind {
        AssetKind::Video | AssetKind::Image => ("video", "video"),
        AssetKind::Audio => ("audio", "audio"),
    };
    let file_name = source.file_name().unwrap_or("asset");
    let destination = output_draft_dir
        .join("_assets")
        .join(category)
        .join(format!("{prefix}_{index:04}_{file_name}"));

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, &destination).with_context(|| {
        format!("failed to copy draft package asset: {source} -> {destination}")
    })?;

    Ok(destination)
}

fn rewrite_draft_package_snapshots(
    draft_dir: &Utf8Path,
    replacements: &[DraftMaterialReplacement],
) -> Result<()> {
    let mut snapshot_files = Vec::new();
    collect_draft_snapshot_files(draft_dir, &mut snapshot_files)?;

    for snapshot_file in snapshot_files {
        rewrite_draft_snapshot(&snapshot_file, replacements)
            .with_context(|| format!("failed to rewrite draft snapshot: {snapshot_file}"))?;
    }

    Ok(())
}

fn collect_draft_snapshot_files(
    current_dir: &Utf8Path,
    snapshot_files: &mut Vec<Utf8PathBuf>,
) -> Result<()> {
    for entry in std::fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = utf8_path_buf(entry.path())?;
        if entry.file_type()?.is_dir() {
            collect_draft_snapshot_files(&path, snapshot_files)?;
            continue;
        }

        if is_draft_snapshot_file(&path) {
            snapshot_files.push(path);
        }
    }

    Ok(())
}

fn is_draft_snapshot_file(path: &Utf8Path) -> bool {
    matches!(
        path.file_name(),
        Some("draft_content.json" | "draft_info.json" | "draft_info.json.bak" | "template-2.tmp")
    )
}

fn rewrite_draft_snapshot(
    snapshot_file: &Utf8Path,
    replacements: &[DraftMaterialReplacement],
) -> Result<()> {
    let content = std::fs::read_to_string(snapshot_file)?;
    let mut draft: Value = match serde_json::from_str(&content) {
        Ok(draft) => draft,
        Err(_) => return Ok(()),
    };

    let mut changed = false;
    for replacement in replacements {
        changed |= rewrite_material_path_by_name(&mut draft, replacement);
    }

    if changed {
        std::fs::write(snapshot_file, serde_json::to_string_pretty(&draft)?)?;
    }

    Ok(())
}

fn rewrite_material_path_by_name(
    draft: &mut Value,
    replacement: &DraftMaterialReplacement,
) -> bool {
    let Some(materials) = draft.get_mut("materials").and_then(Value::as_object_mut) else {
        return false;
    };

    let (list_key, name_key) = match replacement.kind {
        AssetKind::Video | AssetKind::Image => ("videos", "material_name"),
        AssetKind::Audio => ("audios", "name"),
    };

    let Some(items) = materials.get_mut(list_key).and_then(Value::as_array_mut) else {
        return false;
    };

    let mut changed = false;
    for item in items {
        if item.get(name_key).and_then(Value::as_str) == Some(replacement.name.as_str()) {
            item["path"] = json!(replacement.path);
            changed = true;
        }
    }

    changed
}

fn rewrite_meta_info(draft_dir: &Utf8Path, final_name: &str) -> Result<()> {
    let meta_path = draft_dir.join("draft_meta_info.json");
    if !meta_path.exists() {
        return Ok(());
    }

    let mut meta: Value = serde_json::from_str(&std::fs::read_to_string(&meta_path)?)?;
    let output_dir_str = normalize_path_for_draft(draft_dir);
    meta["draft_name"] = json!(final_name);
    meta["draft_root_path"] = json!(output_dir_str);
    meta["draft_fold_path"] = json!(normalize_path_for_draft(draft_dir));
    meta["tm_draft_modified"] = json!(chrono::Utc::now().timestamp());
    meta["tm_draft_cloud_modified"] = json!(0);
    meta["draft_id"] = json!(format!(
        "{{{}}}",
        Uuid::new_v4().as_hyphenated().to_string().to_uppercase()
    ));
    std::fs::write(meta_path, serde_json::to_string_pretty(&meta)?)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{import_bundle, ImportBundleOptions};
    use camino::Utf8PathBuf;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn import_draft_package_rewrites_material_paths() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("draft_bundle")).unwrap();
        let draft_dir = bundle_dir.join("draft");
        let assets_dir = bundle_dir.join("assets");
        fs::create_dir_all(&draft_dir)?;
        fs::create_dir_all(&assets_dir)?;

        fs::write(
            draft_dir.join("draft_content.json"),
            serde_json::to_string_pretty(&json!({
                "duration": 1000,
                "tracks": [],
                "materials": {"videos": [{"material_name": "old_video", "path": "old.mp4"}], "audios": []}
            }))?,
        )?;
        fs::write(
            draft_dir.join("draft_info.json"),
            fs::read_to_string(draft_dir.join("draft_content.json"))?,
        )?;
        fs::write(
            draft_dir.join("draft_meta_info.json"),
            serde_json::to_string_pretty(
                &json!({"draft_name": "old", "draft_root_path": "", "draft_fold_path": ""}),
            )?,
        )?;
        fs::write(assets_dir.join("video.mp4"), b"fake")?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_type": "draft_package",
                "project_id": "proj_draft",
                "project_name": "Draft Package",
                "draft_dir": "draft",
                "assets_dir": "assets",
                "assets": [{"kind": "video", "match_value": "old_video", "relative_path": "video.mp4"}]
            }))?,
        )?;

        let summary = import_bundle(&ImportBundleOptions {
            source: bundle_dir,
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft_output")).unwrap(),
            name_override: Some("New Draft".to_string()),
        })?;

        assert_eq!(summary.bundle_type, "draft_package");
        let output_dir = Utf8PathBuf::from(summary.draft_dir.as_str());
        let content = fs::read_to_string(output_dir.join("draft_content.json"))?;
        assert!(content.contains("_assets"));
        assert!(fs::read_to_string(output_dir.join("draft_meta_info.json"))?.contains("New Draft"));

        Ok(())
    }
}
