use anyhow::{bail, Context, Result};
use camino::Utf8Path;
use jy_draft::writer::write_draft;
use jy_media::material::{create_audio_material, create_video_material};
use jy_schema::{
    AnimationItem, AnimationRef, AudioFade, BackgroundFillType, BackgroundFillingRef, Canvas, Clip,
    FontRef, TextBubbleRef, TextEffectRef, TextStyle, TimeRange, TrackKind, Transform,
    TransitionRef, SEC,
};
use jy_timeline::builder::ProjectBuilder;
use jy_timeline::clip::{make_audio_clip, make_text_clip, make_video_clip};
use serde_json::json;
use std::process::Command;
use uuid::Uuid;

use crate::output;

/// 生成一个无连字符的 UUID，统一用于素材/效果等临时对象 ID。
fn new_id() -> String {
    Uuid::new_v4().as_simple().to_string()
}

/// 生成与 pyJianYingDraft 官方 demo.py 对齐的草稿。
///
/// 当前命令参数仍沿用历史版本：
/// - `video` 对应官方 demo 里的 video.mp4
/// - `dubbing` 对应官方 demo 里的 audio.mp3
/// - `watermark` 对应官方 demo 里的 sticker.gif
///
/// `bgm` 和 `subtitle` 暂时保留为兼容旧命令行形状，不参与官方 demo 生成。
pub fn run(
    name: &str,
    video: &Utf8Path,
    dubbing: &Utf8Path,
    _bgm: &Utf8Path,
    _subtitle: &Utf8Path,
    watermark: &Utf8Path,
    output: &Utf8Path,
) -> Result<()> {
    let video_mat = create_video_material(video, None)
        .with_context(|| format!("failed to load video material: {video}"))?;
    let audio_mat = create_audio_material(dubbing, None)
        .with_context(|| format!("failed to load audio material: {dubbing}"))?;
    let sticker_mat = create_video_material(watermark, None)
        .with_context(|| format!("failed to load sticker material: {watermark}"))?;

    let canvas = Canvas::new(1920, 1080, 30);
    let audio_duration = 5 * SEC;
    let video_duration = 4_200_000;
    let sticker_duration = py_compatible_gif_duration(watermark).unwrap_or(sticker_mat.duration);

    let audio_clip = add_audio_fade(
        make_audio_clip(
            &audio_mat,
            TimeRange::new(0, audio_duration.min(audio_mat.duration)),
            Some(TimeRange::new(0, audio_duration.min(audio_mat.duration))),
            None,
            0.6,
        )?,
        SEC,
        0,
    )?;

    let video_clip = add_py_demo_video_effects(make_video_clip(
        &video_mat,
        TimeRange::new(0, video_duration),
        Some(TimeRange::new(0, video_duration)),
        None,
        1.0,
        None,
    )?)?;

    let sticker_clip = add_background_blur(make_video_clip(
        &sticker_mat,
        TimeRange::new(video_duration, sticker_duration),
        Some(TimeRange::new(0, sticker_duration)),
        None,
        1.0,
        None,
    )?)?;

    let text_clip = add_py_demo_text_effects(make_text_clip(
        "据说pyJianYingDraft效果还不错?",
        TimeRange::new(0, video_duration),
        Some(TextStyle {
            color: (1.0, 1.0, 0.0),
            ..Default::default()
        }),
        Some(Transform {
            y: 0.1,
            ..Default::default()
        }),
    ))?;

    let builder = ProjectBuilder::new(name, canvas)
        .maintrack_adsorb(true)
        .add_track(TrackKind::Audio, "audio", 0)?
        .add_track(TrackKind::Video, "video", 0)?
        .add_track(TrackKind::Text, "text", 0)?
        .add_video_material(video_mat)
        .add_video_material(sticker_mat)
        .add_audio_material(audio_mat)
        .add_clip_to_track("audio", audio_clip)?
        .add_clip_to_track("video", video_clip)?
        .add_clip_to_track("video", sticker_clip)?
        .add_clip_to_track("text", text_clip)?;

    let project = builder.build();
    let summary = json!({
        "draft_dir": output.as_str(),
        "name": name,
        "duration": project.duration,
        "track_count": project.tracks.len(),
        "video_material_count": project.video_materials.len(),
        "audio_material_count": project.audio_materials.len(),
        "inputs": {
            "video": video.as_str(),
            "audio": dubbing.as_str(),
            "sticker": watermark.as_str(),
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

fn add_py_demo_video_effects(clip: Clip) -> Result<Clip> {
    match clip {
        Clip::Video(mut vc) => {
            vc.animations = Some(AnimationRef {
                id: new_id(),
                animations: vec![AnimationItem {
                    name: "斜切".into(),
                    effect_id: "10696371".into(),
                    animation_type: "in".into(),
                    resource_id: "7210657307938525751".into(),
                    start: 0,
                    duration: 700_000,
                    is_video_animation: true,
                }],
            });
            vc.transition = Some(TransitionRef {
                id: new_id(),
                name: "信号故障".into(),
                effect_id: "25265947".into(),
                resource_id: "7288149307197231676".into(),
                duration: 500_000,
                is_overlap: true,
            });
            Ok(Clip::Video(vc))
        }
        other => bail!("expected video clip, got {:?}", clip_kind(&other)),
    }
}

fn add_background_blur(clip: Clip) -> Result<Clip> {
    match clip {
        Clip::Video(mut vc) => {
            vc.background_filling = Some(BackgroundFillingRef {
                id: new_id(),
                fill_type: BackgroundFillType::Blur,
                blur: 0.0625,
                color: "#00000000".into(),
            });
            Ok(Clip::Video(vc))
        }
        other => bail!("expected video clip, got {:?}", clip_kind(&other)),
    }
}

fn add_py_demo_text_effects(clip: Clip) -> Result<Clip> {
    match clip {
        Clip::Text(mut tc) => {
            tc.font = Some(FontRef {
                resource_id: "7290445778273702455".into(),
            });
            tc.animations = Some(AnimationRef {
                id: new_id(),
                animations: vec![AnimationItem {
                    name: "故障闪动".into(),
                    effect_id: "15261509".into(),
                    animation_type: "out".into(),
                    resource_id: "7244102414377161276".into(),
                    start: 3_200_000,
                    duration: SEC,
                    is_video_animation: false,
                }],
            });
            tc.bubble = Some(TextBubbleRef {
                id: new_id(),
                effect_id: "361595".into(),
                resource_id: "6742029398926430728".into(),
            });
            tc.effect = Some(TextEffectRef {
                id: new_id(),
                effect_id: "7296357486490144036".into(),
                resource_id: "7296357486490144036".into(),
            });
            Ok(Clip::Text(tc))
        }
        other => bail!("expected text clip, got {:?}", clip_kind(&other)),
    }
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

/// 仅用于报错信息，让 CLI 在类型不匹配时输出更易读的类型名。
fn clip_kind(clip: &Clip) -> &'static str {
    match clip {
        Clip::Video(_) => "video",
        Clip::Audio(_) => "audio",
        Clip::Text(_) => "text",
        Clip::Image(_) => "image",
    }
}

fn py_compatible_gif_duration(path: &Utf8Path) -> Option<u64> {
    if !path
        .extension()
        .map(|ext| ext.eq_ignore_ascii_case("gif"))
        .unwrap_or(false)
    {
        return None;
    }

    let output = Command::new("ffprobe")
        .args([
            "-v",
            "error",
            "-select_streams",
            "v:0",
            "-show_entries",
            "stream=nb_frames,avg_frame_rate",
            "-of",
            "json",
        ])
        .arg(path.as_str())
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    #[derive(serde::Deserialize)]
    struct Probe {
        streams: Vec<ProbeStream>,
    }

    #[derive(serde::Deserialize)]
    struct ProbeStream {
        nb_frames: Option<String>,
        avg_frame_rate: Option<String>,
    }

    let probe: Probe = serde_json::from_slice(&output.stdout).ok()?;
    let stream = probe.streams.first()?;
    let frames = stream.nb_frames.as_ref()?.parse::<f64>().ok()?;
    let fps = parse_fraction(stream.avg_frame_rate.as_deref()?)?;
    if frames <= 0.0 || fps <= 0.0 {
        return None;
    }

    Some((frames / fps * SEC as f64).round() as u64)
}

fn parse_fraction(value: &str) -> Option<f64> {
    if let Some((numerator, denominator)) = value.split_once('/') {
        let numerator = numerator.parse::<f64>().ok()?;
        let denominator = denominator.parse::<f64>().ok()?;
        if denominator == 0.0 {
            None
        } else {
            Some(numerator / denominator)
        }
    } else {
        value.parse::<f64>().ok()
    }
}
