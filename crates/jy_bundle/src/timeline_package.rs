use std::collections::HashMap;

use anyhow::{anyhow, bail, Context, Result};
use camino::Utf8Path;
use jy_draft::writer::write_draft;
use jy_media::material::{create_audio_material, create_video_material};
use jy_schema::{
    parse_time_str, Canvas, Clip, TextBackground, TextBorder, TextShadow, TextStyle, TimeRange,
    TrackKind, Transform, SEC,
};
use jy_timeline::builder::ProjectBuilder;
use jy_timeline::clip::{make_audio_clip, make_image_clip, make_text_clip, make_video_clip};
use serde::Deserialize;

use crate::api::{
    BundleInspection, ImportBundleOptions, ImportBundleProgress, ImportBundleSummary,
};
use crate::asset::{resolve_asset_source, AssetKind, AssetSourceSpec, ImportedMaterial};
use crate::fs_util::{ensure_output_dir_ready, utf8_path_buf};
use crate::manifest::BundleManifest;
use crate::source::{read_json, PreparedSource};

#[derive(Debug, Deserialize)]
struct TimelineManifest {
    project: TimelineProject,
    #[serde(default)]
    canvas: Canvas,
    #[serde(default = "default_maintrack_adsorb")]
    maintrack_adsorb: bool,
    #[serde(default)]
    assets: Vec<AssetSpec>,
    #[serde(default)]
    tracks: Vec<TrackSpec>,
}

#[derive(Debug, Deserialize)]
struct TimelineProject {
    id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct AssetSpec {
    id: String,
    kind: AssetKind,
    #[serde(default)]
    source: AssetSourceSpec,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TrackSpec {
    id: Option<String>,
    kind: ImportTrackKind,
    name: String,
    render_index: Option<i32>,
    #[serde(default)]
    mute: bool,
    #[serde(default)]
    clips: Vec<ClipSpec>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImportTrackKind {
    Video,
    Audio,
    Text,
}

#[derive(Debug, Deserialize)]
struct ClipSpec {
    id: Option<String>,
    #[serde(rename = "type")]
    clip_type: ImportClipType,
    asset_id: Option<String>,
    timeline_in: TimeValue,
    timeline_out: TimeValue,
    source_in: Option<TimeValue>,
    source_out: Option<TimeValue>,
    text: Option<String>,
    volume: Option<f64>,
    speed: Option<f64>,
    transform: Option<Transform>,
    style: Option<TextStyle>,
    border: Option<TextBorder>,
    background: Option<TextBackground>,
    shadow: Option<TextShadow>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImportClipType {
    Video,
    Audio,
    Image,
    Text,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum TimeValue {
    Integer(u64),
    Float(f64),
    String(String),
}

fn default_maintrack_adsorb() -> bool {
    true
}

pub(crate) fn import_timeline_package<F>(
    options: &ImportBundleOptions,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
    progress: &mut F,
) -> Result<ImportBundleSummary>
where
    F: FnMut(ImportBundleProgress),
{
    let timeline_file = bundle
        .timeline_file
        .as_deref()
        .unwrap_or("timeline.json")
        .to_string();
    let timeline = read_json::<TimelineManifest>(&prepared.bundle_root.join(&timeline_file))?;

    let project_name = options
        .name_override
        .clone()
        .or_else(|| timeline.project.name.clone())
        .or_else(|| bundle.project_name.clone())
        .unwrap_or_else(|| "imported_bundle".to_string());

    let mut builder = ProjectBuilder::new(&project_name, timeline.canvas.clone())
        .maintrack_adsorb(timeline.maintrack_adsorb);

    for track in &timeline.tracks {
        builder = builder.add_track(
            track.kind.into(),
            &track.name,
            track_render_index_offset(track.kind, track.render_index),
        )?;
    }

    let cache_root = utf8_path_buf(prepared.temp_dir.path().to_path_buf())?;
    let mut resolved_assets = HashMap::new();
    for asset in &timeline.assets {
        let material = resolve_asset(
            asset,
            &prepared.bundle_root,
            bundle.assets_dir.as_deref(),
            &cache_root,
            progress,
        )?;
        match &material {
            ImportedMaterial::Video(video) => {
                builder = builder.add_video_material(video.clone());
            }
            ImportedMaterial::Audio(audio) => {
                builder = builder.add_audio_material(audio.clone());
            }
        }
        resolved_assets.insert(asset.id.clone(), material);
    }

    for track in &timeline.tracks {
        for clip in &track.clips {
            let built_clip = build_clip(clip, &resolved_assets)
                .with_context(|| format!("failed to build clip on track '{}'", track.name))?;
            builder = builder.add_clip_to_track(&track.name, built_clip)?;
        }
    }

    let mut project = builder.build();
    project.id = timeline
        .project
        .id
        .clone()
        .or(bundle.project_id.clone())
        .unwrap_or(project.id);

    for project_track in &mut project.tracks {
        if let Some(track_spec) = timeline
            .tracks
            .iter()
            .find(|track| track.name == project_track.name)
        {
            project_track.mute = track_spec.mute;
            if let Some(id) = &track_spec.id {
                project_track.id = id.clone();
            }
            if let Some(render_index) = track_spec.render_index {
                project_track.render_index = render_index;
            }
        }
    }

    ensure_output_dir_ready(&options.output)?;
    write_draft(&project, &options.output)?;

    Ok(ImportBundleSummary {
        source: options.source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "timeline_package".to_string(),
        timeline_file: Some(timeline_file),
        source_draft_dir: None,
        draft_dir: options.output.as_str().to_string(),
        project_id: project.id,
        name: project.name,
        duration: project.duration,
        track_count: project.tracks.len(),
        asset_count: timeline.assets.len(),
        video_material_count: project.video_materials.len(),
        audio_material_count: project.audio_materials.len(),
    })
}

pub(crate) fn inspect_timeline_package(
    source: &Utf8Path,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
) -> Result<BundleInspection> {
    let timeline_file = bundle
        .timeline_file
        .as_deref()
        .unwrap_or("timeline.json")
        .to_string();
    let timeline = read_json::<TimelineManifest>(&prepared.bundle_root.join(&timeline_file))?;

    Ok(BundleInspection {
        source: source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "timeline_package".to_string(),
        timeline_file: Some(timeline_file),
        source_draft_dir: None,
        project_id: timeline.project.id.or(bundle.project_id.clone()),
        project_name: timeline.project.name.or(bundle.project_name.clone()),
        asset_count: timeline.assets.len(),
        track_count: timeline.tracks.len(),
        asset_kinds: timeline
            .assets
            .iter()
            .map(|asset| asset.kind.as_str().to_string())
            .collect(),
    })
}

fn resolve_asset<F>(
    asset: &AssetSpec,
    bundle_root: &Utf8Path,
    assets_dir: Option<&str>,
    cache_root: &Utf8Path,
    progress: &mut F,
) -> Result<ImportedMaterial>
where
    F: FnMut(ImportBundleProgress),
{
    let resolved_path = resolve_asset_source(
        &asset.id,
        &asset.source,
        bundle_root,
        assets_dir,
        cache_root,
        asset.kind,
        progress,
    )?;

    match asset.kind {
        AssetKind::Video | AssetKind::Image => Ok(ImportedMaterial::Video(
            create_video_material(&resolved_path, asset.name.as_deref()).with_context(|| {
                format!(
                    "failed to create video/image material for asset '{}'",
                    asset.id
                )
            })?,
        )),
        AssetKind::Audio => Ok(ImportedMaterial::Audio(
            create_audio_material(&resolved_path, asset.name.as_deref()).with_context(|| {
                format!("failed to create audio material for asset '{}'", asset.id)
            })?,
        )),
    }
}

impl From<ImportTrackKind> for TrackKind {
    fn from(value: ImportTrackKind) -> Self {
        match value {
            ImportTrackKind::Video => TrackKind::Video,
            ImportTrackKind::Audio => TrackKind::Audio,
            ImportTrackKind::Text => TrackKind::Text,
        }
    }
}

impl TimeValue {
    fn as_micros(&self) -> Result<u64> {
        match self {
            TimeValue::Integer(value) => Ok(*value),
            TimeValue::Float(value) => {
                if *value < 0.0 {
                    bail!("time value must not be negative");
                }
                Ok((value * SEC as f64) as u64)
            }
            TimeValue::String(value) => {
                if let Ok(raw) = value.parse::<u64>() {
                    Ok(raw)
                } else {
                    parse_time_str(value)
                        .map_err(|error| anyhow!("invalid time value '{value}': {error}"))
                }
            }
        }
    }
}

fn build_clip(clip: &ClipSpec, assets: &HashMap<String, ImportedMaterial>) -> Result<Clip> {
    let target = build_time_range(&clip.timeline_in, &clip.timeline_out)
        .context("invalid clip target timerange")?;
    let source = match (&clip.source_in, &clip.source_out) {
        (Some(start), Some(end)) => {
            Some(build_time_range(start, end).context("invalid clip source timerange")?)
        }
        (None, None) => None,
        _ => bail!("source_in and source_out must be provided together"),
    };

    let mut built = match clip.clip_type {
        ImportClipType::Video => {
            let material = lookup_video_material(clip.asset_id.as_deref(), assets)?;
            make_video_clip(
                material,
                target,
                source,
                clip.speed,
                clip.volume.unwrap_or(1.0),
                clip.transform.clone(),
            )?
        }
        ImportClipType::Image => {
            let material = lookup_video_material(clip.asset_id.as_deref(), assets)?;
            make_image_clip(material, target, clip.transform.clone())
        }
        ImportClipType::Audio => {
            let material = lookup_audio_material(clip.asset_id.as_deref(), assets)?;
            make_audio_clip(
                material,
                target,
                source,
                clip.speed,
                clip.volume.unwrap_or(1.0),
            )?
        }
        ImportClipType::Text => {
            let text = clip
                .text
                .as_deref()
                .ok_or_else(|| anyhow!("text clip is missing text content"))?;
            make_text_clip(text, target, clip.style.clone(), clip.transform.clone())
        }
    };

    if let Some(id) = &clip.id {
        assign_clip_id(&mut built, id);
    }

    if let Clip::Text(text_clip) = &mut built {
        text_clip.border = clip.border.clone();
        text_clip.background = clip.background.clone();
        text_clip.shadow = clip.shadow.clone();
    }

    Ok(built)
}

fn build_time_range(start: &TimeValue, end: &TimeValue) -> Result<TimeRange> {
    let start_us = start.as_micros()?;
    let end_us = end.as_micros()?;
    if end_us <= start_us {
        bail!("time range end must be greater than start");
    }
    Ok(TimeRange::new(start_us, end_us - start_us))
}

fn lookup_video_material<'a>(
    asset_id: Option<&str>,
    assets: &'a HashMap<String, ImportedMaterial>,
) -> Result<&'a jy_schema::VideoMaterialRef> {
    let asset_id = asset_id.ok_or_else(|| anyhow!("clip is missing asset_id"))?;
    match assets.get(asset_id) {
        Some(ImportedMaterial::Video(material)) => Ok(material),
        Some(ImportedMaterial::Audio(_)) => bail!("asset '{asset_id}' is audio, not video/image"),
        None => bail!("asset '{asset_id}' not found"),
    }
}

fn lookup_audio_material<'a>(
    asset_id: Option<&str>,
    assets: &'a HashMap<String, ImportedMaterial>,
) -> Result<&'a jy_schema::AudioMaterialRef> {
    let asset_id = asset_id.ok_or_else(|| anyhow!("clip is missing asset_id"))?;
    match assets.get(asset_id) {
        Some(ImportedMaterial::Audio(material)) => Ok(material),
        Some(ImportedMaterial::Video(_)) => bail!("asset '{asset_id}' is video/image, not audio"),
        None => bail!("asset '{asset_id}' not found"),
    }
}

fn assign_clip_id(clip: &mut Clip, id: &str) {
    match clip {
        Clip::Video(video) => video.id = id.to_string(),
        Clip::Audio(audio) => audio.id = id.to_string(),
        Clip::Text(text) => text.id = id.to_string(),
        Clip::Image(image) => image.id = id.to_string(),
    }
}

fn track_render_index_offset(kind: ImportTrackKind, render_index: Option<i32>) -> i32 {
    let default_render = TrackKind::from(kind).default_render_index();
    render_index.unwrap_or(default_render) - default_render
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{test_png_bytes, write_test_png};
    use crate::{import_bundle, inspect_bundle_source, ImportBundleOptions};
    use camino::Utf8PathBuf;
    use serde_json::json;
    use std::fs::{self, File};
    use std::io::Write;
    use tempfile::tempdir;
    use zip::write::FileOptions;

    #[test]
    fn import_bundle_from_directory_generates_draft() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("bundle")).unwrap();
        fs::create_dir_all(bundle_dir.join("assets").join("image"))?;
        write_test_png(&bundle_dir.join("assets").join("image").join("poster.png"))?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "project_id": "proj_dir",
                "project_name": "Directory Bundle",
                "timeline_file": "timeline.json",
                "assets_dir": "assets"
            }))?,
        )?;
        fs::write(
            bundle_dir.join("timeline.json"),
            serde_json::to_string_pretty(&json!({
                "project": { "id": "proj_dir", "name": "Directory Bundle" },
                "canvas": { "width": 1280, "height": 720, "fps": 30 },
                "assets": [{
                    "id": "poster",
                    "kind": "image",
                    "source": { "type": "bundle_path", "path": "image/poster.png" }
                }],
                "tracks": [
                    {"kind": "video", "name": "visual", "clips": [{"type": "image", "asset_id": "poster", "timeline_in": 0, "timeline_out": 2000000}]},
                    {"kind": "text", "name": "caption", "clips": [{"type": "text", "timeline_in": "0s", "timeline_out": "2s", "text": "bundle import works"}]}
                ]
            }))?,
        )?;

        let summary = import_bundle(&ImportBundleOptions {
            source: bundle_dir.clone(),
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft_dir")).unwrap(),
            name_override: Some("Imported Dir Draft".to_string()),
        })?;

        let output_dir = Utf8PathBuf::from(summary.draft_dir.as_str());
        assert!(output_dir.join("draft_content.json").exists());
        assert!(output_dir.join("draft_info.json").exists());
        assert!(output_dir.join("_assets").join("video").exists());
        assert!(fs::read_to_string(output_dir.join("draft_meta_info.json"))?
            .contains("Imported Dir Draft"));
        assert!(fs::read_to_string(output_dir.join("draft_content.json"))?
            .contains("bundle import works"));

        Ok(())
    }

    #[test]
    fn import_bundle_from_zip_generates_draft() -> Result<()> {
        let temp = tempdir()?;
        let zip_path = temp.path().join("bundle.zip");
        let bundle_json = serde_json::to_vec_pretty(&json!({
            "bundle_version": 1,
            "project_id": "proj_zip",
            "project_name": "Zip Bundle",
            "timeline_file": "timeline.json",
            "assets_dir": "assets"
        }))?;
        let timeline_json = serde_json::to_vec_pretty(&json!({
            "project": { "id": "proj_zip", "name": "Zip Bundle" },
            "canvas": { "width": 1080, "height": 1920, "fps": 30 },
            "assets": [{
                "id": "poster",
                "kind": "image",
                "source": { "type": "bundle_path", "path": "image/poster.png" }
            }],
            "tracks": [{"kind": "video", "name": "visual", "clips": [{"type": "image", "asset_id": "poster", "timeline_in": 0, "timeline_out": 1500000}]}]
        }))?;

        {
            let file = File::create(&zip_path)?;
            let mut writer = zip::ZipWriter::new(file);
            let options = FileOptions::default();
            writer.add_directory("sample_bundle/", options)?;
            writer.start_file("sample_bundle/bundle.json", options)?;
            writer.write_all(&bundle_json)?;
            writer.start_file("sample_bundle/timeline.json", options)?;
            writer.write_all(&timeline_json)?;
            writer.add_directory("sample_bundle/assets/image/", options)?;
            writer.start_file("sample_bundle/assets/image/poster.png", options)?;
            writer.write_all(&test_png_bytes())?;
            writer.finish()?;
        }

        let summary = import_bundle(&ImportBundleOptions {
            source: Utf8PathBuf::from_path_buf(zip_path).unwrap(),
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft_zip")).unwrap(),
            name_override: None,
        })?;
        let output_dir = Utf8PathBuf::from(summary.draft_dir.as_str());
        assert!(output_dir.join("draft_content.json").exists());
        assert!(fs::read_to_string(output_dir.join("draft_meta_info.json"))?.contains("Zip Bundle"));

        Ok(())
    }

    #[test]
    fn inspect_bundle_reports_project_metadata() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("bundle")).unwrap();
        fs::create_dir_all(&bundle_dir)?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "project_id": "proj_inspect",
                "project_name": "Inspect Bundle",
                "timeline_file": "timeline.json"
            }))?,
        )?;
        fs::write(
            bundle_dir.join("timeline.json"),
            serde_json::to_string_pretty(&json!({
                "project": { "id": "proj_inspect", "name": "Inspect Bundle" },
                "assets": [],
                "tracks": []
            }))?,
        )?;

        let inspection = inspect_bundle_source(&bundle_dir)?;
        assert_eq!(inspection.project_name.as_deref(), Some("Inspect Bundle"));
        assert_eq!(inspection.project_id.as_deref(), Some("proj_inspect"));
        assert_eq!(inspection.asset_count, 0);
        assert_eq!(inspection.track_count, 0);

        Ok(())
    }
}
