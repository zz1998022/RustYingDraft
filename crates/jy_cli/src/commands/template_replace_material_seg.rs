use anyhow::Result;
use camino::Utf8Path;
use jy_schema::{tim, TimeRange, TrackKind};
use jy_template::{ExtendMode, ReplacementMaterial, ShrinkMode, TemplateDraft, TrackSelector};

/// 将 CLI 层的轨道参数转换为 schema 层的轨道类型。
fn to_track_kind(arg: crate::EditableTrackKindArg) -> TrackKind {
    match arg {
        crate::EditableTrackKindArg::Video => TrackKind::Video,
        crate::EditableTrackKindArg::Audio => TrackKind::Audio,
        crate::EditableTrackKindArg::Text => TrackKind::Text,
    }
}

/// 按“轨道 + 片段位置”替换模板中的素材。
///
/// 适合模板里素材没有稳定命名，但轨道位置和片段位置稳定的场景。
pub fn run(
    draft_path: &Utf8Path,
    track_kind: crate::EditableTrackKindArg,
    track_name: Option<&str>,
    track_index: Option<usize>,
    segment_index: usize,
    media_type: crate::MediaTypeArg,
    source: &Utf8Path,
    material_name: Option<&str>,
    source_start: Option<&str>,
    source_duration: Option<&str>,
    output: Option<&Utf8Path>,
) -> Result<()> {
    let mut draft = TemplateDraft::load(draft_path)?;
    // 先构造待替换素材。
    let material = match media_type {
        crate::MediaTypeArg::Video => ReplacementMaterial::Video(
            jy_media::material::create_video_material(source, material_name)?,
        ),
        crate::MediaTypeArg::Audio => ReplacementMaterial::Audio(
            jy_media::material::create_audio_material(source, material_name)?,
        ),
    };

    // 允许外部传入一段源时间范围，只替换素材中的某一段内容。
    let source_timerange = match (source_start, source_duration) {
        (Some(start), Some(duration)) => Some(TimeRange::new(tim(start), tim(duration))),
        (None, None) => None,
        _ => anyhow::bail!("source_start and source_duration must be provided together"),
    };

    // 缩短时默认裁掉尾部；拉长时优先裁剪素材尾部，再推后后续片段。
    draft.replace_material_by_seg(
        to_track_kind(track_kind),
        &TrackSelector {
            name: track_name.map(str::to_string),
            index: track_index,
        },
        segment_index,
        &material,
        source_timerange,
        ShrinkMode::CutTail,
        &[ExtendMode::CutMaterialTail, ExtendMode::PushTail],
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
