use anyhow::{bail, Context, Result};
use camino::Utf8Path;
use jy_draft::writer::write_draft;
use jy_media::material::{create_audio_material, create_video_material};
use jy_schema::{
    AudioFade, Canvas, Clip, TextAlign, TextBorder, TextShadow, TextStyle, TimeRange, TrackKind,
    Transform, SEC,
};
use jy_timeline::builder::ProjectBuilder;
use jy_timeline::clip::{make_audio_clip, make_image_clip, make_text_clip, make_video_clip};
use serde_json::json;
use uuid::Uuid;

use crate::output;

/// 解析后的 SRT 字幕片段。
///
/// `start/end` 统一使用剪映内部的微秒单位，避免后续在构建 `TimeRange`
/// 时重复做秒到微秒的换算。
#[derive(Debug, Clone, PartialEq, Eq)]
struct SubtitleCue {
    start: u64,
    end: u64,
    text: String,
}

/// 生成一个无连字符的 UUID，统一用于素材/效果等临时对象 ID。
fn new_id() -> String {
    Uuid::new_v4().as_simple().to_string()
}

/// 生成“本地素材 + SRT”形式的 demo 草稿。
///
/// 这个命令是一个高层便捷入口，目的不是暴露完整的 schema，
/// 而是让我们能快速验证 Rust 版草稿生成在真实素材上的效果。
pub fn run(
    name: &str,
    video: &Utf8Path,
    dubbing: &Utf8Path,
    bgm: &Utf8Path,
    subtitle: &Utf8Path,
    watermark: &Utf8Path,
    output: &Utf8Path,
) -> Result<()> {
    // 第一步先把素材探测成统一的 MaterialRef，后续 timeline 层只依赖统一模型。
    let video_mat = create_video_material(video, None)
        .with_context(|| format!("failed to load video material: {video}"))?;
    let dubbing_mat = create_audio_material(dubbing, None)
        .with_context(|| format!("failed to load dubbing material: {dubbing}"))?;
    let bgm_mat = create_audio_material(bgm, None)
        .with_context(|| format!("failed to load bgm material: {bgm}"))?;
    let watermark_mat = create_video_material(watermark, None)
        .with_context(|| format!("failed to load watermark material: {watermark}"))?;

    let subtitle_cues = parse_srt(subtitle)
        .with_context(|| format!("failed to parse subtitle file: {subtitle}"))?;
    if subtitle_cues.is_empty() {
        bail!("subtitle file contained no cues: {subtitle}");
    }
    let subtitle_count = subtitle_cues.len();

    // 当前 demo 直接沿用主视频的尺寸作为草稿画布。
    let duration = video_mat.duration;
    let canvas = Canvas::new(video_mat.width, video_mat.height, 30);

    // 主视频作为底片轨，默认把原声静音，方便直接听配音 + BGM 的组合效果。
    let main_video = make_video_clip(
        &video_mat,
        TimeRange::new(0, duration),
        Some(TimeRange::new(0, duration)),
        None,
        0.0,
        None,
    )?;

    // 水印作为一张覆盖在整段时间线上的图片片段处理。
    let watermark_clip = make_image_clip(
        &watermark_mat,
        TimeRange::new(0, duration),
        Some(Transform {
            x: 0.86,
            y: 0.11,
            scale_x: 0.22,
            scale_y: 0.22,
            opacity: 0.82,
            ..Default::default()
        }),
    );

    // 配音和 BGM 都是音频轨；这里加一点淡入淡出，避免试听时过于生硬。
    let dubbing_clip = add_audio_fade(
        make_audio_clip(
            &dubbing_mat,
            TimeRange::new(0, duration.min(dubbing_mat.duration)),
            None,
            None,
            1.0,
        )?,
        300_000,
        300_000,
    )?;

    let bgm_clip = add_audio_fade(
        make_audio_clip(
            &bgm_mat,
            TimeRange::new(0, duration.min(bgm_mat.duration)),
            None,
            None,
            0.22,
        )?,
        SEC,
        1_500_000,
    )?;

    // 字幕样式用一套固定配置，尽量接近此前 Python demo 的视觉效果。
    let subtitle_style = TextStyle {
        size: 7.2,
        color: (1.0, 1.0, 1.0),
        align: TextAlign::Center,
        auto_wrapping: true,
        max_line_width: 0.82,
        ..Default::default()
    };
    let subtitle_border = TextBorder {
        alpha: 1.0,
        color: (0.0, 0.0, 0.0),
        width: 55.0 / 100.0 * 0.2,
    };
    let subtitle_shadow = TextShadow {
        alpha: 0.5,
        color: (0.0, 0.0, 0.0),
        diffuse: 20.0,
        distance: 6.0,
        angle: -45.0,
    };
    let subtitle_transform = Transform {
        x: 0.5,
        y: 0.09,
        ..Default::default()
    };

    // ProjectBuilder 负责做轨道去重、轨道名校验、片段重叠校验和总时长维护。
    let mut builder = ProjectBuilder::new(name, canvas)
        .maintrack_adsorb(true)
        .add_track(TrackKind::Video, "main_video", 0)?
        .add_track(TrackKind::Video, "watermark", 100)?
        .add_track(TrackKind::Audio, "dubbing", 0)?
        .add_track(TrackKind::Audio, "bgm", 1)?
        .add_track(TrackKind::Text, "subtitle", 999)?
        .add_video_material(video_mat)
        .add_video_material(watermark_mat)
        .add_audio_material(dubbing_mat)
        .add_audio_material(bgm_mat)
        .add_clip_to_track("main_video", main_video)?
        .add_clip_to_track("watermark", watermark_clip)?
        .add_clip_to_track("dubbing", dubbing_clip)?
        .add_clip_to_track("bgm", bgm_clip)?;

    // 每条字幕 cue 都转成一个独立的 TextClip，方便后续继续做模板替换或样式重算。
    for cue in subtitle_cues {
        let text_clip = add_text_decoration(
            make_text_clip(
                &cue.text,
                TimeRange::new(cue.start, cue.end - cue.start),
                Some(subtitle_style.clone()),
                Some(subtitle_transform.clone()),
            ),
            Some(subtitle_border.clone()),
            Some(subtitle_shadow.clone()),
        )?;
        builder = builder.add_clip_to_track("subtitle", text_clip)?;
    }

    let project = builder.build();
    let summary = json!({
        "draft_dir": output.as_str(),
        "name": name,
        "duration": project.duration,
        "track_count": project.tracks.len(),
        "video_material_count": project.video_materials.len(),
        "audio_material_count": project.audio_materials.len(),
        "subtitle_count": subtitle_count,
        "inputs": {
            "video": video.as_str(),
            "dubbing": dubbing.as_str(),
            "bgm": bgm.as_str(),
            "subtitle": subtitle.as_str(),
            "watermark": watermark.as_str(),
        }
    });

    write_draft(&project, output)?;
    output::emit_result(
        "generate-demo",
        &format!("Generated demo draft: {output}"),
        summary,
    );
    Ok(())
}

/// 给音频片段附加淡入淡出效果。
///
/// 这里之所以单独包装一层，是为了让 demo 命令保持高层语义，
/// 不需要在 `run()` 里手工展开对 `Clip::Audio` 的 match。
fn add_audio_fade(clip: Clip, in_duration: u64, out_duration: u64) -> Result<Clip> {
    match clip {
        Clip::Audio(mut ac) => {
            ac.fade = Some(AudioFade {
                id: new_id(),
                in_duration,
                out_duration,
            });
            Ok(Clip::Audio(ac))
        }
        other => bail!("expected audio clip, got {:?}", clip_kind(&other)),
    }
}

/// 给文本片段补充描边和阴影效果。
fn add_text_decoration(
    clip: Clip,
    border: Option<TextBorder>,
    shadow: Option<TextShadow>,
) -> Result<Clip> {
    match clip {
        Clip::Text(mut tc) => {
            tc.border = border;
            tc.shadow = shadow;
            Ok(Clip::Text(tc))
        }
        other => bail!("expected text clip, got {:?}", clip_kind(&other)),
    }
}

/// 仅用于报错信息，让 CLI 在类型不匹配时输出更易读的类型名。
fn clip_kind(clip: &Clip) -> &'static str {
    match clip {
        Clip::Video(_) => "video",
        Clip::Audio(_) => "audio",
        Clip::Text(_) => "text",
        Clip::Image(_) => "image",
    }
}

/// 读取并解析 SRT 文件。
///
/// 这里做了两件兼容处理：
/// 1. 统一换行风格，兼容 `\r\n` 和 `\n`
/// 2. 兼容带序号和不带序号的 block
fn parse_srt(path: &Utf8Path) -> Result<Vec<SubtitleCue>> {
    let content = std::fs::read_to_string(path)?;
    let normalized = content.replace("\r\n", "\n");
    let mut cues = Vec::new();

    for block in normalized.split("\n\n") {
        let mut lines = block.lines().map(str::trim).filter(|line| !line.is_empty());
        let Some(first_line) = lines.next() else {
            continue;
        };

        let timestamp_line = if first_line.chars().all(|ch| ch.is_ascii_digit()) {
            lines
                .next()
                .context("subtitle block is missing timestamp line after index")?
        } else {
            first_line
        };

        let (start, end) = parse_srt_timerange(timestamp_line)?;
        let text_lines: Vec<&str> = lines.collect();
        if text_lines.is_empty() {
            continue;
        }

        cues.push(SubtitleCue {
            start,
            end,
            text: text_lines.join("\n"),
        });
    }

    Ok(cues)
}

/// 解析一行 `00:00:01,000 --> 00:00:02,500` 形式的 SRT 时间范围。
fn parse_srt_timerange(line: &str) -> Result<(u64, u64)> {
    let (start, end) = line
        .split_once("-->")
        .context("invalid srt timerange line")?;
    let start = parse_srt_timestamp(start.trim())?;
    let end = parse_srt_timestamp(end.trim())?;
    if end <= start {
        bail!("subtitle cue end must be greater than start: {line}");
    }
    Ok((start, end))
}

/// 将单个 SRT 时间戳转换成微秒。
fn parse_srt_timestamp(value: &str) -> Result<u64> {
    let value = value.replace(',', ".");
    let parts: Vec<&str> = value.split(':').collect();
    if parts.len() != 3 {
        bail!("invalid srt timestamp: {value}");
    }

    let hours: u64 = parts[0].parse().context("invalid srt hour component")?;
    let minutes: u64 = parts[1].parse().context("invalid srt minute component")?;
    let seconds: f64 = parts[2].parse().context("invalid srt second component")?;

    Ok((hours * 3600 * SEC as u64) + (minutes * 60 * SEC as u64) + (seconds * SEC as f64) as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_srt_timestamp_to_microseconds() {
        assert_eq!(parse_srt_timestamp("00:00:21,674").unwrap(), 21_674_000);
        assert_eq!(parse_srt_timestamp("01:02:03.500").unwrap(), 3_723_500_000);
    }

    #[test]
    fn parses_srt_blocks_with_index_and_multiline_text() {
        let tempdir = tempfile::tempdir().unwrap();
        let path = Utf8Path::from_path(tempdir.path())
            .unwrap()
            .join("sample.srt");
        std::fs::write(
            path.as_std_path(),
            "1\n00:00:01,000 --> 00:00:02,500\n第一行\n第二行\n\n2\n00:00:03,000 --> 00:00:04,000\n第三行\n",
        )
        .unwrap();

        let cues = parse_srt(&path).unwrap();
        assert_eq!(
            cues,
            vec![
                SubtitleCue {
                    start: 1_000_000,
                    end: 2_500_000,
                    text: "第一行\n第二行".into(),
                },
                SubtitleCue {
                    start: 3_000_000,
                    end: 4_000_000,
                    text: "第三行".into(),
                }
            ]
        );
    }
}
