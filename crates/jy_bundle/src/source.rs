use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use serde::Deserialize;
use tempfile::TempDir;
use zip::ZipArchive;

use crate::fs_util::utf8_path_buf;
use crate::manifest::BundleManifest;

#[derive(Debug)]
pub(crate) struct PreparedSource {
    pub(crate) bundle_root: Utf8PathBuf,
    pub(crate) temp_dir: TempDir,
}

impl PreparedSource {
    pub(crate) fn from_source(source: &Utf8Path) -> Result<Self> {
        if source.is_dir() {
            let temp_dir = TempDir::new().context("failed to create temporary import workspace")?;
            return Ok(Self {
                bundle_root: find_bundle_root(source)?,
                temp_dir,
            });
        }

        if source.is_file() && source.file_name() == Some("bundle.json") {
            let temp_dir = TempDir::new().context("failed to create temporary import workspace")?;
            let bundle_root = source
                .parent()
                .ok_or_else(|| anyhow!("bundle.json must have a parent directory"))?
                .to_path_buf();
            return Ok(Self {
                bundle_root,
                temp_dir,
            });
        }

        if source.is_file() {
            let temp_dir =
                TempDir::new().context("failed to create temporary extraction directory")?;
            extract_zip_archive(source, temp_dir.path())?;
            let temp_root = utf8_path_buf(temp_dir.path().to_path_buf())?;
            return Ok(Self {
                bundle_root: find_bundle_root(&temp_root)?,
                temp_dir,
            });
        }

        bail!("bundle source does not exist: {source}");
    }

    pub(crate) fn manifest(&self) -> Result<BundleManifest> {
        read_json(&self.bundle_root.join("bundle.json"))
    }
}

pub(crate) fn read_json<T: for<'de> Deserialize<'de>>(path: &Utf8Path) -> Result<T> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read json file: {path}"))?;
    serde_json::from_str(&content).with_context(|| format!("failed to parse json file: {path}"))
}

fn find_bundle_root(start: &Utf8Path) -> Result<Utf8PathBuf> {
    if start.join("bundle.json").exists() {
        return Ok(start.to_path_buf());
    }

    let mut stack = vec![start.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)
            .with_context(|| format!("failed to inspect directory: {dir}"))?
        {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                let utf8_dir = utf8_path_buf(path)?;
                if utf8_dir.join("bundle.json").exists() {
                    return Ok(utf8_dir);
                }
                stack.push(utf8_dir);
            }
        }
    }

    bail!("bundle.json not found under source: {start}");
}

fn extract_zip_archive(source: &Utf8Path, destination: &Path) -> Result<()> {
    let file =
        File::open(source).with_context(|| format!("failed to open bundle archive: {source}"))?;
    let mut archive =
        ZipArchive::new(file).with_context(|| format!("failed to read zip archive: {source}"))?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(relative) = entry.enclosed_name().map(PathBuf::from) else {
            continue;
        };
        let output_path = destination.join(relative);

        if entry.is_dir() {
            std::fs::create_dir_all(&output_path)?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut output_file = File::create(&output_path)?;
        io::copy(&mut entry, &mut output_file)?;
        output_file.flush()?;
    }

    Ok(())
}
