use anyhow::Result;
use camino::Utf8Path;
use jy_template::TemplateDraft;

/// 复制一份模板草稿目录。
///
/// 适合先把模板草稿复制一份，再在复制出的目录上继续做素材替换或文本替换。
pub fn run(template_dir: &Utf8Path, output_dir: &Utf8Path, allow_replace: bool) -> Result<()> {
    let draft = TemplateDraft::duplicate_draft_dir(template_dir, output_dir, allow_replace)?;
    draft.save_to_draft_dir(output_dir)?;
    println!("Duplicated template draft to: {}", output_dir);
    Ok(())
}
