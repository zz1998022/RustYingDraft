use camino::Utf8PathBuf;
use serde::Serialize;
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct ImportBundleOptions {
    pub source: Utf8PathBuf,
    pub output: Utf8PathBuf,
    pub name_override: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportBundleSummary {
    pub source: String,
    pub bundle_root: String,
    pub bundle_type: String,
    pub timeline_file: Option<String>,
    pub source_draft_dir: Option<String>,
    pub draft_dir: String,
    pub project_id: String,
    pub name: String,
    pub duration: u64,
    pub track_count: usize,
    pub asset_count: usize,
    pub video_material_count: usize,
    pub audio_material_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BundleInspection {
    pub source: String,
    pub bundle_root: String,
    pub bundle_type: String,
    pub timeline_file: Option<String>,
    pub source_draft_dir: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub asset_count: usize,
    pub track_count: usize,
    pub asset_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportBundleProgress {
    pub stage: String,
    pub message: String,
    pub data: Value,
}
