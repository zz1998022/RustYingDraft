use camino::Utf8PathBuf;
use jy_bundle::{
    import_bundle, inspect_bundle_source as inspect_bundle_project, ImportBundleOptions,
};
use serde::Serialize;
use std::process::Command;

#[derive(Debug, Serialize)]
struct AppBundleInspection {
    source: String,
    bundle_root: String,
    bundle_type: String,
    timeline_file: Option<String>,
    source_draft_dir: Option<String>,
    project_id: Option<String>,
    project_name: Option<String>,
    asset_count: usize,
    track_count: usize,
    asset_kinds: Vec<String>,
}

#[derive(Debug, Serialize)]
struct AppImportSummary {
    source: String,
    bundle_root: String,
    bundle_type: String,
    timeline_file: Option<String>,
    source_draft_dir: Option<String>,
    draft_dir: String,
    project_id: String,
    name: String,
    duration: u64,
    track_count: usize,
    asset_count: usize,
    video_material_count: usize,
    audio_material_count: usize,
}

#[tauri::command]
fn detect_draft_box_dir() -> Option<String> {
    detect_known_draft_box_dirs()
        .into_iter()
        .find(|candidate| candidate.exists() && candidate.is_dir())
        .and_then(|path| path.to_str().map(str::to_string))
}

#[tauri::command]
fn detect_bundle_source_near_app() -> Option<String> {
    detect_bundle_candidates()
        .into_iter()
        .find(|candidate| candidate.join("bundle.json").exists())
        .and_then(|path| path.to_str().map(str::to_string))
}

#[tauri::command]
fn inspect_bundle_source(source: String) -> Result<AppBundleInspection, String> {
    let source = Utf8PathBuf::from(source);
    let inspection = inspect_bundle_project(&source).map_err(|error| error.to_string())?;
    Ok(AppBundleInspection {
        source: inspection.source,
        bundle_root: inspection.bundle_root,
        bundle_type: inspection.bundle_type,
        timeline_file: inspection.timeline_file,
        source_draft_dir: inspection.source_draft_dir,
        project_id: inspection.project_id,
        project_name: inspection.project_name,
        asset_count: inspection.asset_count,
        track_count: inspection.track_count,
        asset_kinds: inspection.asset_kinds,
    })
}

#[tauri::command]
fn import_bundle_to_draft_box(
    source: String,
    draft_box_dir: String,
    draft_name: String,
) -> Result<AppImportSummary, String> {
    let source = Utf8PathBuf::from(source);
    let draft_box_dir = Utf8PathBuf::from(draft_box_dir);
    if draft_name.trim().is_empty() {
        return Err("draft_name must not be empty".to_string());
    }

    let draft_name = sanitize_draft_name(&draft_name);
    let output = draft_box_dir.join(&draft_name);

    let summary = import_bundle(&ImportBundleOptions {
        source,
        output,
        name_override: Some(draft_name),
    })
    .map_err(|error| error.to_string())?;

    Ok(AppImportSummary {
        source: summary.source,
        bundle_root: summary.bundle_root,
        bundle_type: summary.bundle_type,
        timeline_file: summary.timeline_file,
        source_draft_dir: summary.source_draft_dir,
        draft_dir: summary.draft_dir,
        project_id: summary.project_id,
        name: summary.name,
        duration: summary.duration,
        track_count: summary.track_count,
        asset_count: summary.asset_count,
        video_material_count: summary.video_material_count,
        audio_material_count: summary.audio_material_count,
    })
}

#[tauri::command]
fn open_path_in_file_manager(path: String) -> Result<(), String> {
    let path = Utf8PathBuf::from(path);
    if !path.exists() {
        return Err(format!("path not found: {path}"));
    }

    let status = if cfg!(target_os = "macos") {
        Command::new("open").arg(path.as_str()).status()
    } else if cfg!(target_os = "windows") {
        Command::new("explorer").arg(path.as_str()).status()
    } else {
        Command::new("xdg-open").arg(path.as_str()).status()
    }
    .map_err(|error| error.to_string())?;

    if status.success() {
        Ok(())
    } else {
        Err(format!("failed to open path: {path}"))
    }
}

fn sanitize_draft_name(value: &str) -> String {
    let trimmed = value.trim();
    let sanitized = trimmed
        .chars()
        .map(|ch| match ch {
            '\\' | '/' | ':' | '*' | '?' | '"' | '<' | '>' | '|' => '_',
            _ => ch,
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join("_");

    if sanitized.is_empty() {
        "imported_bundle".to_string()
    } else {
        sanitized
    }
}

fn detect_known_draft_box_dirs() -> Vec<std::path::PathBuf> {
    let Some(home) = home_dir() else {
        return Vec::new();
    };

    let mut candidates = Vec::new();

    if cfg!(target_os = "macos") {
        candidates.push(
            home.join("Movies")
                .join("JianyingPro")
                .join("User Data")
                .join("Projects")
                .join("com.lveditor.draft"),
        );
        candidates.push(
            home.join("Movies")
                .join("CapCut")
                .join("User Data")
                .join("Projects")
                .join("com.lveditor.draft"),
        );
    }

    if cfg!(target_os = "windows") {
        candidates.push(
            home.join("AppData")
                .join("Local")
                .join("JianyingPro")
                .join("User Data")
                .join("Projects")
                .join("com.lveditor.draft"),
        );
        candidates.push(
            home.join("AppData")
                .join("Local")
                .join("CapCut")
                .join("User Data")
                .join("Projects")
                .join("com.lveditor.draft"),
        );
    }

    candidates
}

fn detect_bundle_candidates() -> Vec<std::path::PathBuf> {
    let mut candidates = Vec::new();

    if let Ok(current_dir) = std::env::current_dir() {
        candidates.push(current_dir);
    }

    if let Ok(current_exe) = std::env::current_exe() {
        for ancestor in current_exe.ancestors().skip(1).take(6) {
            candidates.push(ancestor.to_path_buf());
        }
    }

    dedup_paths(candidates)
}

fn dedup_paths(paths: Vec<std::path::PathBuf>) -> Vec<std::path::PathBuf> {
    let mut result = Vec::new();
    for path in paths {
        if !result.iter().any(|existing| existing == &path) {
            result.push(path);
        }
    }
    result
}

fn home_dir() -> Option<std::path::PathBuf> {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .or_else(|| std::env::var_os("USERPROFILE").map(std::path::PathBuf::from))
        .or_else(|| {
            let drive = std::env::var_os("HOMEDRIVE")?;
            let path = std::env::var_os("HOMEPATH")?;
            let mut full = std::path::PathBuf::from(drive);
            full.push(path);
            Some(full)
        })
}

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            detect_draft_box_dir,
            detect_bundle_source_near_app,
            inspect_bundle_source,
            import_bundle_to_draft_box,
            open_path_in_file_manager
        ])
        .run(tauri::generate_context!())
        .expect("error while running yingdraft companion");
}
