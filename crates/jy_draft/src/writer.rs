use crate::converter::project_to_draft;
use crate::error::DraftError;
use crate::templates::load_meta_template;
use camino::{Utf8Path, Utf8PathBuf};
use jy_schema::Project;
use std::collections::HashMap;
use uuid::Uuid;

/// 将一个完整工程写入指定目录，生成可被剪映识别的草稿文件。
///
/// 这个函数是 `jy_draft` 对外最核心的入口：
/// - 输入：统一的 `Project`
/// - 输出：草稿目录中的 `draft_content.json`、`draft_info.json` 和 `draft_meta_info.json`
pub fn write_draft(project: &Project, output_dir: &Utf8Path) -> Result<(), DraftError> {
    // 先确保输出目录存在。
    std::fs::create_dir_all(output_dir)?;

    // mac 上某些版本的剪映对工作区外素材访问更稳定，统一把本地素材复制到草稿目录内。
    let localized_project = localize_project_assets(project, output_dir)?;

    // 生成并写入主草稿内容。
    let draft = project_to_draft(&localized_project)?;
    let content_path = output_dir.join("draft_content.json");
    let content_str = serde_json::to_string_pretty(&draft)?;
    std::fs::write(&content_path, &content_str)?;
    std::fs::write(output_dir.join("draft_info.json"), &content_str)?;

    // 生成并写入草稿 meta 信息。
    // 这个文件字段不多，但剪映依赖它来识别草稿名称、ID、修改时间等信息。
    let mut meta = load_meta_template()?;
    let draft_id = Uuid::new_v4();
    let output_dir_str = normalize_path_for_draft(output_dir);
    meta["draft_id"] = json!(format!(
        "{{{}}}",
        draft_id.as_hyphenated().to_string().to_uppercase()
    ));
    meta["draft_name"] = json!(localized_project.name);
    meta["draft_root_path"] = json!(output_dir_str);
    meta["draft_fold_path"] = json!(output_dir_str);
    meta["tm_duration"] = json!(format!(
        "{}.000000",
        localized_project.duration / jy_schema::SEC
    ));
    meta["tm_draft_cloud_modified"] = json!(0);
    meta["tm_draft_modified"] = json!(chrono::Utc::now().timestamp());

    let meta_path = output_dir.join("draft_meta_info.json");
    let meta_str = serde_json::to_string_pretty(&meta)?;
    std::fs::write(&meta_path, meta_str)?;

    Ok(())
}

fn localize_project_assets(
    project: &Project,
    output_dir: &Utf8Path,
) -> Result<Project, DraftError> {
    let mut localized = project.clone();
    let assets_root = output_dir.join("_assets");
    let mut copied = HashMap::<Utf8PathBuf, Utf8PathBuf>::new();

    for (index, material) in localized.video_materials.iter_mut().enumerate() {
        if let Some(path) = localize_asset_path(
            &material.path,
            &assets_root.join("video"),
            "video",
            index,
            &mut copied,
        )? {
            material.path = path;
        }
    }

    for (index, material) in localized.audio_materials.iter_mut().enumerate() {
        if let Some(path) = localize_asset_path(
            &material.path,
            &assets_root.join("audio"),
            "audio",
            index,
            &mut copied,
        )? {
            material.path = path;
        }
    }

    Ok(localized)
}

fn localize_asset_path(
    source: &Utf8Path,
    category_dir: &Utf8Path,
    prefix: &str,
    index: usize,
    copied: &mut HashMap<Utf8PathBuf, Utf8PathBuf>,
) -> Result<Option<Utf8PathBuf>, DraftError> {
    if !source.exists() || !source.is_file() {
        return Ok(None);
    }

    if let Some(existing) = copied.get(source) {
        return Ok(Some(existing.clone()));
    }

    std::fs::create_dir_all(category_dir)?;

    let file_name = source.file_name().unwrap_or("asset");
    let destination = category_dir.join(format!("{prefix}_{index:04}_{file_name}"));
    std::fs::copy(source, &destination)?;

    copied.insert(source.to_path_buf(), destination.clone());
    Ok(Some(destination))
}

fn normalize_path_for_draft(path: &Utf8Path) -> String {
    if cfg!(windows) {
        path.as_str().replace('\\', "/")
    } else {
        path.as_str().to_string()
    }
}

use serde_json::json;
