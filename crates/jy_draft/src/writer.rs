use crate::converter::project_to_draft;
use crate::error::DraftError;
use crate::templates::load_meta_template;
use camino::Utf8Path;
use jy_schema::Project;
use uuid::Uuid;

/// 将一个完整工程写入指定目录，生成可被剪映识别的草稿文件。
///
/// 这个函数是 `jy_draft` 对外最核心的入口：
/// - 输入：统一的 `Project`
/// - 输出：草稿目录中的 `draft_content.json` 和 `draft_meta_info.json`
pub fn write_draft(project: &Project, output_dir: &Utf8Path) -> Result<(), DraftError> {
    // 先确保输出目录存在。
    std::fs::create_dir_all(output_dir)?;

    // 生成并写入主草稿内容。
    let draft = project_to_draft(project)?;
    let content_path = output_dir.join("draft_content.json");
    let content_str = serde_json::to_string_pretty(&draft)?;
    std::fs::write(&content_path, content_str)?;

    // 生成并写入草稿 meta 信息。
    // 这个文件字段不多，但剪映依赖它来识别草稿名称、ID、修改时间等信息。
    let mut meta = load_meta_template()?;
    let draft_id = Uuid::new_v4();
    meta["draft_id"] = json!(format!(
        "{{{}}}",
        draft_id.as_hyphenated().to_string().to_uppercase()
    ));
    meta["draft_name"] = json!(project.name);
    meta["tm_duration"] = json!(format!("{}.000000", project.duration / jy_schema::SEC));
    meta["tm_draft_cloud_modified"] = json!(0);
    meta["tm_draft_modified"] = json!(chrono::Utc::now().timestamp());

    let meta_path = output_dir.join("draft_meta_info.json");
    let meta_str = serde_json::to_string_pretty(&meta)?;
    std::fs::write(&meta_path, meta_str)?;

    Ok(())
}

use serde_json::json;
