use anyhow::Result;
use camino::Utf8Path;
use jy_template::{TemplateDraft, TrackSelector};

/// 替换模板中的文本片段。
///
/// `text` 支持传入多个值，用于多段文本模板。
pub fn run(
    draft_path: &Utf8Path,
    track_name: Option<&str>,
    track_index: Option<usize>,
    segment_index: usize,
    text: &[String],
    recalc_style: bool,
    output: Option<&Utf8Path>,
) -> Result<()> {
    let mut draft = TemplateDraft::load(draft_path)?;
    // 模板层内部会自动判断是单文本替换，还是多段 text_template 替换。
    draft.replace_texts(
        &TrackSelector {
            name: track_name.map(str::to_string),
            index: track_index,
        },
        segment_index,
        text,
        recalc_style,
    )?;

    if let Some(output_path) = output {
        draft.write_to(output_path)?;
        println!("Updated template written to: {}", output_path);
    } else {
        draft.save()?;
        println!("Updated template in place: {}", draft_path);
    }

    Ok(())
}
