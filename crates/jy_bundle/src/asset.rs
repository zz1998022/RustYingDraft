use std::fs::File;
use std::io::{self, Write};

use anyhow::{anyhow, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;
use serde_json::json;

use crate::api::ImportBundleProgress;
use crate::fs_util::download_file_name;

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AssetKind {
    Video,
    Audio,
    Image,
}

impl AssetKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Image => "image",
        }
    }
}

#[derive(Debug, Deserialize, Default)]
pub(crate) struct AssetSourceSpec {
    #[serde(rename = "type")]
    pub(crate) source_type: AssetSourceType,
    pub(crate) path: Option<String>,
    pub(crate) url: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum AssetSourceType {
    #[default]
    BundlePath,
    LocalPath,
    Url,
}

#[derive(Debug, Clone)]
pub(crate) enum ImportedMaterial {
    Video(jy_schema::VideoMaterialRef),
    Audio(jy_schema::AudioMaterialRef),
}

pub(crate) fn resolve_asset_source<F>(
    asset_id: &str,
    source: &AssetSourceSpec,
    bundle_root: &Utf8Path,
    assets_dir: Option<&str>,
    cache_root: &Utf8Path,
    asset_kind: AssetKind,
    progress: &mut F,
) -> Result<Utf8PathBuf>
where
    F: FnMut(ImportBundleProgress),
{
    match source.source_type {
        AssetSourceType::BundlePath => {
            let relative = source
                .path
                .as_deref()
                .ok_or_else(|| anyhow!("asset '{}' is missing source.path", asset_id))?;
            Ok(resolve_bundle_relative_path(
                relative,
                bundle_root,
                assets_dir,
            ))
        }
        AssetSourceType::LocalPath => {
            let raw_path = source
                .path
                .as_deref()
                .ok_or_else(|| anyhow!("asset '{}' is missing source.path", asset_id))?;
            let path = Utf8PathBuf::from(raw_path);
            Ok(if path.is_absolute() {
                path
            } else {
                bundle_root.join(path)
            })
        }
        AssetSourceType::Url => {
            let url = source
                .url
                .as_deref()
                .ok_or_else(|| anyhow!("asset '{}' is missing source.url", asset_id))?;
            download_asset(url, asset_id, cache_root, asset_kind, progress)
        }
    }
}

fn resolve_bundle_relative_path(
    relative: &str,
    bundle_root: &Utf8Path,
    assets_dir: Option<&str>,
) -> Utf8PathBuf {
    let base = assets_dir
        .map(|dir| bundle_root.join(dir))
        .unwrap_or_else(|| bundle_root.to_path_buf());
    let preferred = base.join(relative);
    let fallback = bundle_root.join(relative);
    if preferred.exists() {
        preferred
    } else if fallback.exists() {
        fallback
    } else {
        preferred
    }
}

fn download_asset<F>(
    url: &str,
    asset_id: &str,
    cache_root: &Utf8Path,
    asset_kind: AssetKind,
    progress: &mut F,
) -> Result<Utf8PathBuf>
where
    F: FnMut(ImportBundleProgress),
{
    progress(ImportBundleProgress {
        stage: "download_asset".to_string(),
        message: format!("Downloading asset {} from {}", asset_id, url),
        data: json!({
            "asset_id": asset_id,
            "kind": asset_kind.as_str(),
            "url": url,
        }),
    });

    let downloads_dir = cache_root.join("downloads");
    std::fs::create_dir_all(&downloads_dir)?;

    let file_name = download_file_name(url, asset_id);
    let destination = downloads_dir.join(file_name);

    let mut response = reqwest::blocking::get(url)
        .with_context(|| format!("failed to request asset url: {url}"))?
        .error_for_status()
        .with_context(|| format!("asset download returned non-success status: {url}"))?;
    let mut file = File::create(&destination)
        .with_context(|| format!("failed to create downloaded asset file: {destination}"))?;
    io::copy(&mut response, &mut file)
        .with_context(|| format!("failed to save downloaded asset: {destination}"))?;
    file.flush()?;

    Ok(destination)
}
