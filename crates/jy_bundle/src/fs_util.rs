use std::path::PathBuf;

use anyhow::{anyhow, bail, Result};
use camino::{Utf8Path, Utf8PathBuf};
use jy_schema::SEC;
use url::Url;

pub(crate) fn ensure_output_dir_ready(output: &Utf8Path) -> Result<()> {
    if output.exists() {
        let mut entries = std::fs::read_dir(output)?;
        if entries.next().transpose()?.is_some() {
            bail!("output directory is not empty: {output}");
        }
    } else {
        std::fs::create_dir_all(output)?;
    }
    Ok(())
}

pub(crate) fn utf8_path_buf(path: PathBuf) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path).map_err(|path| anyhow!("non-utf8 path: {}", path.display()))
}

pub(crate) fn seconds_to_micros(value: f64, label: &str) -> Result<u64> {
    if !value.is_finite() || value < 0.0 {
        bail!("{label} must be non-negative seconds");
    }
    let micros = value * SEC as f64;
    if micros > u64::MAX as f64 {
        bail!("{label} is too large");
    }
    Ok(micros.round() as u64)
}

pub(crate) fn resolve_simple_asset_path(
    relative: &str,
    bundle_root: &Utf8Path,
    assets_dir: Option<&str>,
) -> Result<Utf8PathBuf> {
    validate_simple_relative_path("asset path", relative)?;
    let assets_dir = assets_dir.unwrap_or("assets");
    validate_simple_relative_path("assets_dir", assets_dir)?;

    let path = bundle_root.join(assets_dir).join(relative);
    if !path.is_file() {
        bail!("simple_timeline_package asset not found: {relative}");
    }
    Ok(path)
}

pub(crate) fn resolve_pipeline_asset_path(
    label: &str,
    relative: &str,
    bundle_root: &Utf8Path,
    assets_dir: Option<&str>,
) -> Result<Utf8PathBuf> {
    validate_simple_relative_path(label, relative)?;
    let assets_dir = assets_dir.unwrap_or("assets");
    validate_simple_relative_path("assets_dir", assets_dir)?;

    let path = bundle_root.join(assets_dir).join(relative);
    if !path.is_file() {
        bail!("{label} not found: {relative}");
    }
    Ok(path)
}

pub(crate) fn validate_simple_relative_path(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{label} must not be empty");
    }
    if value.contains('\\') {
        bail!("{label} must use '/' as path separator");
    }
    let path = Utf8Path::new(value);
    if path.is_absolute() {
        bail!("{label} must be relative");
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        bail!("{label} contains an invalid path segment");
    }
    Ok(())
}

pub(crate) fn copy_dir_all(source: &Utf8Path, target: &Utf8Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = utf8_path_buf(entry.path())?;
        let target_path = target.join(entry.file_name().to_string_lossy().as_ref());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

pub(crate) fn normalize_path_for_draft(path: &Utf8Path) -> String {
    if cfg!(windows) {
        path.as_str().replace('\\', "/")
    } else {
        path.as_str().to_string()
    }
}

pub(crate) fn download_file_name(url: &str, asset_id: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back().map(ToString::to_string))
        })
        .filter(|name| !name.is_empty())
        .map(|name| sanitize_file_name(&name))
        .unwrap_or_else(|| format!("{asset_id}.bin"))
}

pub(crate) fn sanitize_file_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "asset.bin".to_string()
    } else {
        sanitized
    }
}
