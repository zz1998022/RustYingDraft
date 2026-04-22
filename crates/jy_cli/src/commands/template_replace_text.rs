use anyhow::Result;
use camino::Utf8Path;
use jy_template::{TemplateDraft, TrackSelector};
use serde_json::json;

use crate::output;

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
        output::emit_result(
            "template-replace-text",
            &format!("Updated template written to: {output_path}"),
            json!({
                "draft_path": draft_path.as_str(),
                "output_path": output_path.as_str(),
                "track_name": track_name,
                "track_index": track_index,
                "segment_index": segment_index,
                "text_count": text.len(),
                "recalc_style": recalc_style,
                "in_place": false,
            }),
        );
    } else {
        draft.save()?;
        output::emit_result(
            "template-replace-text",
            &format!("Updated template in place: {draft_path}"),
            json!({
                "draft_path": draft_path.as_str(),
                "output_path": draft_path.as_str(),
                "track_name": track_name,
                "track_index": track_index,
                "segment_index": segment_index,
                "text_count": text.len(),
                "recalc_style": recalc_style,
                "in_place": true,
            }),
        );
    }

    Ok(())
}
