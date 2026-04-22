use std::collections::HashMap;
use std::fs::File;
use std::io::Write;
use std::time::Duration;

use anyhow::{bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use jy_draft::writer::write_draft;
use jy_media::material::{create_audio_material, create_video_material};
use jy_schema::{
    Canvas, Clip, MaterialKind, TextAlign, TextBorder, TextShadow, TextStyle, TimeRange, TrackKind,
    Transform, VideoMaterialRef,
};
use jy_timeline::builder::ProjectBuilder;
use jy_timeline::clip::{make_audio_clip, make_image_clip, make_text_clip, make_video_clip};
use reqwest::blocking::Client;
use serde::Deserialize;
use url::Url;

/// 阿里云 VOD 导出的顶层时间轴结构。
///
/// 当前我们只解析第一版转换器真正用到的字段，剩余字段可按需继续补。
#[derive(Debug, Deserialize)]
struct VodProject {
    #[serde(rename = "FECanvas")]
    fe_canvas: Option<VodCanvas>,
    #[serde(rename = "OutputMediaConfig")]
    output_media_config: Option<VodOutputMediaConfig>,
    #[serde(rename = "SubtitleTracks", default)]
    subtitle_tracks: Vec<VodSubtitleTrack>,
    #[serde(rename = "VideoTracks", default)]
    video_tracks: Vec<VodVideoTrack>,
    #[serde(rename = "AudioTracks", default)]
    audio_tracks: Vec<VodAudioTrack>,
}

/// VOD 画布尺寸定义。
#[derive(Debug, Deserialize)]
struct VodCanvas {
    #[serde(rename = "Width")]
    width: u32,
    #[serde(rename = "Height")]
    height: u32,
}

/// VOD 输出分辨率定义。
#[derive(Debug, Deserialize)]
struct VodOutputMediaConfig {
    #[serde(rename = "Width")]
    width: u32,
    #[serde(rename = "Height")]
    height: u32,
}

/// VOD 视频轨。
#[derive(Debug, Deserialize, Default)]
struct VodVideoTrack {
    #[serde(rename = "VideoTrackClips", default)]
    clips: Vec<VodVisualClip>,
}

/// VOD 音频轨。
#[derive(Debug, Deserialize, Default)]
struct VodAudioTrack {
    #[serde(rename = "AudioTrackClips", default)]
    clips: Vec<VodAudioClipDef>,
}

/// VOD 字幕轨。
#[derive(Debug, Deserialize, Default)]
struct VodSubtitleTrack {
    #[serde(rename = "SubtitleTrackClips", default)]
    clips: Vec<VodSubtitleClip>,
}

/// 视频轨上的视觉片段定义。
///
/// 这里既承载普通视频片段，也承载 `GlobalImage` 这类全局覆盖层。
#[derive(Debug, Deserialize, Clone)]
struct VodVisualClip {
    #[serde(rename = "Type")]
    clip_type: Option<String>,
    #[serde(rename = "MediaURL")]
    media_url: String,
    #[serde(rename = "TimelineIn")]
    timeline_in: Option<f64>,
    #[serde(rename = "TimelineOut")]
    timeline_out: Option<f64>,
    #[serde(rename = "AdaptMode")]
    adapt_mode: Option<String>,
    #[serde(rename = "X")]
    x: Option<f64>,
    #[serde(rename = "Y")]
    y: Option<f64>,
    #[serde(rename = "Width")]
    width: Option<f64>,
    #[serde(rename = "Height")]
    height: Option<f64>,
}

/// 音频轨上的片段定义。
#[derive(Debug, Deserialize, Clone)]
struct VodAudioClipDef {
    #[serde(rename = "MediaURL")]
    media_url: String,
    #[serde(rename = "TimelineIn")]
    timeline_in: Option<f64>,
    #[serde(rename = "TimelineOut")]
    timeline_out: Option<f64>,
}

/// 字幕片段定义。
#[derive(Debug, Deserialize, Clone)]
struct VodSubtitleClip {
    #[serde(rename = "Content")]
    content: String,
    #[serde(rename = "TimelineIn")]
    timeline_in: f64,
    #[serde(rename = "TimelineOut")]
    timeline_out: f64,
    #[serde(rename = "FontSize")]
    font_size: Option<f64>,
    #[serde(rename = "FontColor")]
    font_color: Option<String>,
    #[serde(rename = "X")]
    x: Option<f64>,
    #[serde(rename = "Y")]
    y: Option<f64>,
    #[serde(rename = "Alignment")]
    alignment: Option<String>,
    #[serde(rename = "TextWidth")]
    text_width: Option<f64>,
    #[serde(rename = "Outline")]
    outline: Option<f64>,
    #[serde(rename = "OutlineColour")]
    outline_colour: Option<String>,
    #[serde(rename = "FontColorOpacity")]
    font_color_opacity: Option<f64>,
    #[serde(rename = "AdaptMode")]
    adapt_mode: Option<String>,
    #[serde(rename = "FontFace")]
    font_face: Option<VodFontFace>,
}

/// 字体粗体/斜体/下划线等布尔样式。
#[derive(Debug, Deserialize, Clone)]
struct VodFontFace {
    #[serde(rename = "Bold")]
    bold: Option<bool>,
    #[serde(rename = "Italic")]
    italic: Option<bool>,
    #[serde(rename = "Underline")]
    underline: Option<bool>,
}

/// 经过归一化后的时间位置。
///
/// `position_clip` 会把 VOD 里的浮点秒值统一映射为微秒，
/// 并处理“未显式给出 TimelineIn 时沿用轨道游标继续向后排布”的逻辑。
#[derive(Debug, Clone)]
struct PositionedClip {
    start: u64,
    end: u64,
}

/// 读取阿里云 VOD JSON，下载远程素材，并输出为剪映草稿。
///
/// 当前这是“云端时间轴 -> 本地剪映草稿”的主要入口。
pub fn run(
    config: &Utf8Path,
    assets_dir: Option<&Utf8Path>,
    output: &Utf8Path,
    name: Option<&str>,
) -> Result<()> {
    let content = std::fs::read_to_string(config)?;
    let project: VodProject = serde_json::from_str(&content)?;

    // 所有远程素材都会先落到本地，因为剪映草稿依赖本机绝对路径。
    let assets_dir = assets_dir
        .map(Utf8PathBuf::from)
        .unwrap_or_else(|| output.join("_assets"));
    std::fs::create_dir_all(&assets_dir)?;

    let project_name = name
        .map(str::to_string)
        .or_else(|| output.file_name().map(str::to_string))
        .unwrap_or_else(|| "vod_json_draft".to_string());

    let canvas = resolve_canvas(&project);
    let client = Client::builder()
        .timeout(Duration::from_secs(120))
        .build()
        .context("failed to create HTTP client")?;

    let mut fetcher = AssetFetcher::new(client, assets_dir);

    // 先用字幕轨估一个总时长，后面在读视频/音频时继续取最大值。
    let mut duration = project
        .subtitle_tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .map(|clip| seconds_to_us(clip.timeline_out))
        .max()
        .unwrap_or(0);

    let mut timed_video_entries = Vec::new();
    let mut overlay_entries = Vec::new();
    for (track_idx, track) in project.video_tracks.iter().enumerate() {
        let mut cursor = 0;
        for clip in &track.clips {
            let material = fetcher.resolve_video_material(&clip.media_url)?;
            // `GlobalImage` 这种 clip 不走普通时间轴排布，而是转为覆盖层。
            let is_overlay = clip
                .clip_type
                .as_deref()
                .map(|kind| kind.eq_ignore_ascii_case("GlobalImage"))
                .unwrap_or(false);

            if is_overlay {
                overlay_entries.push((track_idx, clip.clone(), material));
                continue;
            }

            let positioned = position_clip(
                &clip.timeline_in,
                &clip.timeline_out,
                material.duration,
                &mut cursor,
            )?;
            duration = duration.max(positioned.end);
            timed_video_entries.push((track_idx, clip.clone(), material, positioned));
        }
    }

    let mut audio_entries = Vec::new();
    for (track_idx, track) in project.audio_tracks.iter().enumerate() {
        let mut cursor = 0;
        for clip in &track.clips {
            let material = fetcher.resolve_audio_material(&clip.media_url)?;
            let positioned = position_clip(
                &clip.timeline_in,
                &clip.timeline_out,
                material.duration,
                &mut cursor,
            )?;
            duration = duration.max(positioned.end);
            audio_entries.push((track_idx, clip.clone(), material, positioned));
        }
    }

    // 先把轨道骨架全部建好，后面统一往对应轨道里塞 clip。
    let mut builder = ProjectBuilder::new(&project_name, canvas).maintrack_adsorb(true);

    for (idx, _) in project.video_tracks.iter().enumerate() {
        builder = builder.add_track(TrackKind::Video, &format!("video_{idx}"), idx as i32)?;
    }
    for (idx, _) in project.audio_tracks.iter().enumerate() {
        builder = builder.add_track(TrackKind::Audio, &format!("audio_{idx}"), idx as i32)?;
    }
    for (idx, _) in project.subtitle_tracks.iter().enumerate() {
        builder = builder.add_track(
            TrackKind::Text,
            &format!("subtitle_{idx}"),
            999 + idx as i32,
        )?;
    }

    for (_, _, material, _) in &timed_video_entries {
        builder = builder.add_video_material(material.clone());
    }
    for (_, _, material) in &overlay_entries {
        builder = builder.add_video_material(material.clone());
    }
    for (_, _, material, _) in &audio_entries {
        builder = builder.add_audio_material(material.clone());
    }

    for (track_idx, clip, material, positioned) in timed_video_entries {
        let target = TimeRange::new(positioned.start, positioned.end - positioned.start);
        let transform = build_visual_transform(&clip, &material, &project, false);
        let built_clip = match material.kind {
            MaterialKind::Photo => make_image_clip(&material, target, transform),
            _ => make_video_clip(
                &material,
                target,
                Some(TimeRange::new(0, target.duration.min(material.duration))),
                None,
                1.0,
                transform,
            )?,
        };
        builder = builder.add_clip_to_track(&format!("video_{track_idx}"), built_clip)?;
    }

    // 全局覆盖层默认铺满整段成片时长。
    for (track_idx, clip, material) in overlay_entries {
        let target = TimeRange::new(0, duration.max(material.duration.min(duration.max(1))));
        let transform = build_visual_transform(&clip, &material, &project, true);
        let built_clip = match material.kind {
            MaterialKind::Photo => make_image_clip(&material, target, transform),
            _ => make_video_clip(
                &material,
                target,
                Some(TimeRange::new(0, target.duration.min(material.duration))),
                None,
                1.0,
                transform,
            )?,
        };
        builder = builder.add_clip_to_track(&format!("video_{track_idx}"), built_clip)?;
    }

    for (track_idx, _, material, positioned) in audio_entries {
        let target = TimeRange::new(positioned.start, positioned.end - positioned.start);
        let built_clip = make_audio_clip(
            &material,
            target,
            Some(TimeRange::new(0, target.duration.min(material.duration))),
            None,
            1.0,
        )?;
        builder = builder.add_clip_to_track(&format!("audio_{track_idx}"), built_clip)?;
    }

    // 字幕转换目前走“基础样式 + 基础位置”的策略，优先保证可见和位置大致一致。
    for (track_idx, track) in project.subtitle_tracks.iter().enumerate() {
        for cue in &track.clips {
            let style = build_subtitle_style(cue);
            let transform = Transform {
                x: cue.x.unwrap_or(0.5),
                y: cue.y.unwrap_or(0.78),
                ..Default::default()
            };
            let mut text_clip = make_text_clip(
                &cue.content,
                TimeRange::new(
                    seconds_to_us(cue.timeline_in),
                    seconds_to_us(cue.timeline_out - cue.timeline_in),
                ),
                Some(style),
                Some(transform),
            );
            if let Clip::Text(ref mut clip) = text_clip {
                if cue.outline.unwrap_or(0.0) > 0.0 {
                    clip.border = Some(TextBorder {
                        alpha: 1.0,
                        color: parse_hex_rgb(cue.outline_colour.as_deref().unwrap_or("#000000")),
                        width: 0.08,
                    });
                }
                clip.shadow = Some(TextShadow {
                    alpha: 0.35,
                    color: (0.0, 0.0, 0.0),
                    diffuse: 18.0,
                    distance: 5.0,
                    angle: -45.0,
                });
            }
            builder = builder.add_clip_to_track(&format!("subtitle_{track_idx}"), text_clip)?;
        }
    }

    let draft = builder.build();
    write_draft(&draft, output)?;
    println!("Generated draft from VOD JSON: {output}");
    Ok(())
}

/// 解析 VOD 画布信息，优先使用 FECanvas，其次使用输出分辨率。
fn resolve_canvas(project: &VodProject) -> Canvas {
    if let Some(canvas) = &project.fe_canvas {
        Canvas::new(canvas.width, canvas.height, 30)
    } else if let Some(output) = &project.output_media_config {
        Canvas::new(output.width, output.height, 30)
    } else {
        Canvas::default()
    }
}

/// 将 VOD 字幕样式映射到 `TextStyle`。
///
/// 这里先做基础字段映射，后续如果要补字体资源映射，可以从这里继续扩展。
fn build_subtitle_style(cue: &VodSubtitleClip) -> TextStyle {
    TextStyle {
        size: cue.font_size.unwrap_or(72.0) / 10.0,
        bold: cue.font_face.as_ref().and_then(|f| f.bold).unwrap_or(false),
        italic: cue
            .font_face
            .as_ref()
            .and_then(|f| f.italic)
            .unwrap_or(false),
        underline: cue
            .font_face
            .as_ref()
            .and_then(|f| f.underline)
            .unwrap_or(false),
        color: parse_hex_rgb(cue.font_color.as_deref().unwrap_or("#ffffff")),
        alpha: cue.font_color_opacity.unwrap_or(1.0),
        align: match cue.alignment.as_deref() {
            Some("Center") => TextAlign::Center,
            Some("Right") => TextAlign::Right,
            _ => TextAlign::Left,
        },
        auto_wrapping: cue.adapt_mode.as_deref() == Some("AutoWrap"),
        max_line_width: cue.text_width.unwrap_or(0.82),
        ..Default::default()
    }
}

/// 构建视觉变换信息。
///
/// - 对普通视频片段，当前先保守处理，不额外变换
/// - 对 `GlobalImage` 覆盖层，则根据像素坐标和目标尺寸近似换算到剪映坐标
fn build_visual_transform(
    clip: &VodVisualClip,
    material: &VideoMaterialRef,
    project: &VodProject,
    use_pixel_geometry: bool,
) -> Option<Transform> {
    if use_pixel_geometry {
        let canvas = resolve_canvas(project);
        let target_width = clip.width.unwrap_or(material.width as f64);
        let target_height = clip.height.unwrap_or(material.height as f64);
        let x = clip.x.unwrap_or(0.0);
        let y = clip.y.unwrap_or(0.0);
        return Some(Transform {
            x: ((x + target_width / 2.0) / canvas.width as f64).clamp(0.0, 1.5),
            y: ((y + target_height / 2.0) / canvas.height as f64).clamp(-0.5, 1.5),
            scale_x: if material.width > 0 {
                target_width / material.width as f64
            } else {
                1.0
            },
            scale_y: if material.height > 0 {
                target_height / material.height as f64
            } else {
                1.0
            },
            ..Default::default()
        });
    }

    match clip.adapt_mode.as_deref() {
        // 第一版里 `Contain` 暂时不做裁剪和额外填充，保持默认行为。
        Some("Contain") => None,
        _ => None,
    }
}

/// 根据 `TimelineIn / TimelineOut` 和轨道游标计算一个片段的最终时间范围。
///
/// 若 `TimelineIn` 缺失，则表示接在前一个片段尾部继续向后排。
fn position_clip(
    timeline_in: &Option<f64>,
    timeline_out: &Option<f64>,
    default_duration: u64,
    cursor: &mut u64,
) -> Result<PositionedClip> {
    let start = timeline_in.map_or(*cursor, seconds_to_us);
    let mut end = timeline_out
        .map(seconds_to_us)
        .unwrap_or(start + default_duration);
    if end <= start {
        end = start + default_duration;
    }
    *cursor = end;
    Ok(PositionedClip { start, end })
}

/// 将 VOD JSON 中的浮点秒值转换为剪映内部使用的微秒。
fn seconds_to_us(value: f64) -> u64 {
    (value * 1_000_000.0) as u64
}

/// 将 `#RRGGBB` 颜色转换为 `0.0 ~ 1.0` 的 RGB 三元组。
fn parse_hex_rgb(value: &str) -> (f64, f64, f64) {
    let hex = value.trim().trim_start_matches('#');
    if hex.len() < 6 {
        return (1.0, 1.0, 1.0);
    }

    let parse_pair = |slice: &str| u8::from_str_radix(slice, 16).unwrap_or(255) as f64 / 255.0;
    (
        parse_pair(&hex[0..2]),
        parse_pair(&hex[2..4]),
        parse_pair(&hex[4..6]),
    )
}

/// 远程素材下载与缓存管理器。
///
/// 作用：
/// 1. URL 去重，避免同一素材重复下载
/// 2. 统一把远程资源落到本地素材目录
/// 3. 为后续 `create_video_material/create_audio_material` 提供本地绝对路径
struct AssetFetcher {
    client: Client,
    assets_dir: Utf8PathBuf,
    downloads: HashMap<String, Utf8PathBuf>,
}

impl AssetFetcher {
    /// 创建一个素材抓取器。
    fn new(client: Client, assets_dir: Utf8PathBuf) -> Self {
        Self {
            client,
            assets_dir,
            downloads: HashMap::new(),
        }
    }

    /// 解析一个视频或图片 URL/路径为本地视频素材引用。
    fn resolve_video_material(&mut self, source: &str) -> Result<VideoMaterialRef> {
        let local = self.resolve_local_path(source)?;
        create_video_material(&local, None).map_err(Into::into)
    }

    /// 解析一个音频 URL/路径为本地音频素材引用。
    fn resolve_audio_material(&mut self, source: &str) -> Result<jy_schema::AudioMaterialRef> {
        let local = self.resolve_local_path(source)?;
        create_audio_material(&local, None).map_err(Into::into)
    }

    /// 将输入路径统一解析为本地路径。
    ///
    /// - 远程 URL：先下载
    /// - 本地路径：直接透传
    fn resolve_local_path(&mut self, source: &str) -> Result<Utf8PathBuf> {
        if let Some(existing) = self.downloads.get(source) {
            return Ok(existing.clone());
        }

        let local = if source.starts_with("http://") || source.starts_with("https://") {
            self.download_remote(source)?
        } else {
            Utf8PathBuf::from(source)
        };
        self.downloads.insert(source.to_string(), local.clone());
        Ok(local)
    }

    /// 下载远程素材到素材目录。
    fn download_remote(&self, source: &str) -> Result<Utf8PathBuf> {
        let url = Url::parse(source)?;
        let file_name = build_remote_file_name(&url);
        let path = self.assets_dir.join(file_name);

        if path.exists() {
            return Ok(path);
        }

        std::fs::create_dir_all(&self.assets_dir)?;
        let mut response = self
            .client
            .get(source)
            .send()
            .with_context(|| format!("failed to download asset: {source}"))?
            .error_for_status()
            .with_context(|| format!("remote asset returned error: {source}"))?;

        let mut file = File::create(path.as_std_path())?;
        let bytes = response.copy_to(&mut file)?;
        file.flush()?;
        if bytes == 0 {
            bail!("downloaded empty asset from {source}");
        }

        Ok(path)
    }
}

/// 根据远程 URL 推导出一个尽量稳定、可落地的本地文件名。
fn build_remote_file_name(url: &Url) -> String {
    let path = url.path();
    let base = path
        .rsplit('/')
        .next()
        .filter(|segment| !segment.is_empty())
        .unwrap_or("asset.bin");
    let sanitized: String = base
        .chars()
        .map(|ch| {
            if ch.is_ascii_alphanumeric()
                || matches!(ch, '.' | '_' | '-' | '%')
                || ('\u{4e00}'..='\u{9fff}').contains(&ch)
            {
                ch
            } else {
                '_'
            }
        })
        .collect();
    sanitized
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn positions_clip_with_explicit_out_and_implicit_in() {
        let mut cursor = 5_000_000;
        let positioned = position_clip(&None, &Some(8.5), 2_000_000, &mut cursor).unwrap();
        assert_eq!(positioned.start, 5_000_000);
        assert_eq!(positioned.end, 8_500_000);
        assert_eq!(cursor, 8_500_000);
    }

    #[test]
    fn parses_hex_rgb() {
        assert_eq!(parse_hex_rgb("#ffffff"), (1.0, 1.0, 1.0));
        assert_eq!(parse_hex_rgb("#000000"), (0.0, 0.0, 0.0));
    }
}
