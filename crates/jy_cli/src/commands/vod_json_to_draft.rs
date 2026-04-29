use std::collections::{HashMap, HashSet};
use std::fs::File;
use std::io::{Read, Write};
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
use serde_json::json;
use url::Url;

use crate::output;

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
    #[serde(rename = "Shadow")]
    shadow: Option<f64>,
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
#[derive(Debug, Clone, Copy)]
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
    use_internal_url: bool,
) -> Result<()> {
    let content = std::fs::read_to_string(config)?;
    let project: VodProject = serde_json::from_str(&content)?;
    let remote_sources = collect_remote_sources(&project);

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

    if remote_sources.is_empty() {
        output::emit_progress(
            "vod-json-to-draft",
            "resolve-assets",
            "No remote assets found in VOD config. Using local paths directly.",
            json!({
                "remote_asset_count": 0,
                "assets_dir": assets_dir.as_str(),
                "use_internal_url": use_internal_url,
            }),
        );
    } else {
        output::emit_progress(
            "vod-json-to-draft",
            "resolve-assets",
            &format!(
                "Preparing to resolve {} remote assets into: {}",
                remote_sources.len(),
                assets_dir
            ),
            json!({
                "remote_asset_count": remote_sources.len(),
                "assets_dir": assets_dir.as_str(),
                "use_internal_url": use_internal_url,
            }),
        );
    }

    let mut fetcher = AssetFetcher::new(client, assets_dir, remote_sources.len(), use_internal_url);

    // 先用字幕轨估一个总时长，后面在读视频/音频时继续取最大值。
    let mut duration = project
        .subtitle_tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .map(|clip| seconds_to_us(clip.timeline_out))
        .max()
        .unwrap_or(0);

    let mut video_lanes = HashMap::<usize, Vec<Vec<PositionedClip>>>::new();
    let mut timed_video_entries = Vec::new();
    for (track_idx, track) in project.video_tracks.iter().enumerate() {
        let mut cursor = 0;
        for clip in &track.clips {
            // 水印/全局图片交给用户在剪映里处理，CLI 不再转换坐标或生成覆盖层。
            let is_global_image = clip
                .clip_type
                .as_deref()
                .map(|kind| kind.eq_ignore_ascii_case("GlobalImage"))
                .unwrap_or(false);
            if is_global_image {
                continue;
            }

            let material = fetcher.resolve_video_material(&clip.media_url)?;
            let positioned = position_clip(
                &clip.timeline_in,
                &clip.timeline_out,
                material.duration,
                &mut cursor,
            )?;
            duration = duration.max(positioned.end);
            let lane_idx =
                assign_timeline_lane(video_lanes.entry(track_idx).or_default(), positioned);
            timed_video_entries.push((track_idx, lane_idx, clip.clone(), material, positioned));
        }
    }

    let mut audio_lanes = HashMap::<usize, Vec<Vec<PositionedClip>>>::new();
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
            let lane_idx =
                assign_timeline_lane(audio_lanes.entry(track_idx).or_default(), positioned);
            audio_entries.push((track_idx, lane_idx, clip.clone(), material, positioned));
        }
    }

    let mut subtitle_lanes = HashMap::<usize, Vec<Vec<PositionedClip>>>::new();
    let mut subtitle_entries = Vec::new();
    for (track_idx, track) in project.subtitle_tracks.iter().enumerate() {
        for cue in &track.clips {
            let positioned = PositionedClip {
                start: seconds_to_us(cue.timeline_in),
                end: seconds_to_us(cue.timeline_out),
            };
            duration = duration.max(positioned.end);
            let lane_idx =
                assign_timeline_lane(subtitle_lanes.entry(track_idx).or_default(), positioned);
            subtitle_entries.push((track_idx, lane_idx, cue.clone(), positioned));
        }
    }

    // 先把轨道骨架全部建好，后面统一往对应轨道里塞 clip。
    let mut builder = ProjectBuilder::new(&project_name, canvas.clone()).maintrack_adsorb(true);

    for (idx, _) in project.video_tracks.iter().enumerate() {
        let lane_count = lane_count_for_track(&video_lanes, idx);
        for lane_idx in 0..lane_count {
            builder = builder.add_track(
                TrackKind::Video,
                &vod_track_name("video", idx, lane_idx),
                render_index_offset(idx, lane_idx, project.video_tracks.len(), 0),
            )?;
        }
    }
    for (idx, _) in project.audio_tracks.iter().enumerate() {
        let lane_count = lane_count_for_track(&audio_lanes, idx);
        for lane_idx in 0..lane_count {
            builder = builder.add_track(
                TrackKind::Audio,
                &vod_track_name("audio", idx, lane_idx),
                render_index_offset(idx, lane_idx, project.audio_tracks.len(), 0),
            )?;
        }
    }
    for (idx, _) in project.subtitle_tracks.iter().enumerate() {
        let lane_count = lane_count_for_track(&subtitle_lanes, idx);
        for lane_idx in 0..lane_count {
            builder = builder.add_track(
                TrackKind::Text,
                &vod_track_name("subtitle", idx, lane_idx),
                render_index_offset(idx, lane_idx, project.subtitle_tracks.len(), 999),
            )?;
        }
    }

    for (_, _, _, material, _) in &timed_video_entries {
        builder = builder.add_video_material(material.clone());
    }
    for (_, _, _, material, _) in &audio_entries {
        builder = builder.add_audio_material(material.clone());
    }

    for (track_idx, lane_idx, clip, material, positioned) in timed_video_entries {
        let target = TimeRange::new(positioned.start, positioned.end - positioned.start);
        let transform = build_visual_transform(&clip);
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
        builder =
            builder.add_clip_to_track(&vod_track_name("video", track_idx, lane_idx), built_clip)?;
    }

    for (track_idx, lane_idx, _, material, positioned) in audio_entries {
        let target = TimeRange::new(positioned.start, positioned.end - positioned.start);
        let built_clip = make_audio_clip(
            &material,
            target,
            Some(TimeRange::new(0, target.duration.min(material.duration))),
            None,
            1.0,
        )?;
        builder =
            builder.add_clip_to_track(&vod_track_name("audio", track_idx, lane_idx), built_clip)?;
    }

    // 字幕转换目前走“基础样式 + 基础位置”的策略，优先保证可见和位置大致一致。
    for (track_idx, lane_idx, cue, positioned) in subtitle_entries {
        let style = build_subtitle_style(&cue);
        let transform = build_subtitle_transform(&cue, &canvas);
        let mut text_clip = make_text_clip(
            &cue.content,
            TimeRange::new(positioned.start, positioned.end - positioned.start),
            Some(style),
            Some(transform),
        );
        if let Clip::Text(ref mut clip) = text_clip {
            if cue.outline.unwrap_or(0.0) > 0.0 {
                clip.border = Some(TextBorder {
                    alpha: 1.0,
                    color: parse_hex_rgb(cue.outline_colour.as_deref().unwrap_or("#000000")),
                    width: vod_outline_to_jy_border_width(cue.outline.unwrap_or(0.0)),
                });
            }
            if cue.shadow.unwrap_or(0.0) > 0.0 {
                clip.shadow = Some(TextShadow {
                    alpha: 0.35,
                    color: (0.0, 0.0, 0.0),
                    diffuse: 18.0,
                    distance: 5.0,
                    angle: -45.0,
                });
            }
        }
        builder = builder
            .add_clip_to_track(&vod_track_name("subtitle", track_idx, lane_idx), text_clip)?;
    }

    let draft = builder.build();
    write_draft(&draft, output)?;
    if fetcher.remote_total() > 0 {
        output::emit_progress(
            "vod-json-to-draft",
            "resolve-assets",
            &format!(
                "Resolved remote assets: {}/{}",
                fetcher.completed_downloads(),
                fetcher.remote_total()
            ),
            json!({
                "resolved_remote_assets": fetcher.completed_downloads(),
                "remote_asset_count": fetcher.remote_total(),
            }),
        );
    }
    output::emit_result(
        "vod-json-to-draft",
        &format!("Generated draft from VOD JSON: {output}"),
        json!({
            "config_path": config.as_str(),
            "draft_dir": output.as_str(),
            "assets_dir": fetcher.assets_dir().as_str(),
            "name": project_name,
            "duration": draft.duration,
            "video_track_count": draft.tracks.iter().filter(|track| track.kind == TrackKind::Video).count(),
            "audio_track_count": draft.tracks.iter().filter(|track| track.kind == TrackKind::Audio).count(),
            "text_track_count": draft.tracks.iter().filter(|track| track.kind == TrackKind::Text).count(),
            "video_material_count": draft.video_materials.len(),
            "audio_material_count": draft.audio_materials.len(),
            "remote_asset_count": fetcher.remote_total(),
            "resolved_remote_assets": fetcher.completed_downloads(),
            "use_internal_url": use_internal_url,
        }),
    );
    Ok(())
}

/// 预扫描 VOD 配置中的远程素材 URL，便于展示总进度。
fn collect_remote_sources(project: &VodProject) -> Vec<String> {
    let mut seen = HashSet::new();
    let mut ordered = Vec::new();

    let mut push_source = |source: &str| {
        if (source.starts_with("http://") || source.starts_with("https://"))
            && seen.insert(source.to_string())
        {
            ordered.push(source.to_string());
        }
    };

    for track in &project.video_tracks {
        for clip in &track.clips {
            if clip
                .clip_type
                .as_deref()
                .map(|kind| kind.eq_ignore_ascii_case("GlobalImage"))
                .unwrap_or(false)
            {
                continue;
            }
            push_source(&clip.media_url);
        }
    }
    for track in &project.audio_tracks {
        for clip in &track.clips {
            push_source(&clip.media_url);
        }
    }

    ordered
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
        size: cue.font_size.unwrap_or(72.0),
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
        auto_wrapping: is_auto_wrapping_adapt_mode(cue.adapt_mode.as_deref()),
        max_line_width: cue.text_width.unwrap_or(0.82),
        ..Default::default()
    }
}

fn is_auto_wrapping_adapt_mode(adapt_mode: Option<&str>) -> bool {
    matches!(adapt_mode, Some("AutoWrap") | Some("AutoWrapAtSpaces"))
}

/// 参考 pyJianYingDraft，剪映界面里的描边宽度需要转成草稿内部宽度。
fn vod_outline_to_jy_border_width(width: f64) -> f64 {
    width / 100.0 * 0.2
}

/// VOD 字幕的 X/Y 由后端按剪映界面位移值写死。
///
/// 参考 pyJianYingDraft，草稿里的 `clip.transform.x/y` 不是像素，而是剪映内部
/// 位移比例；剪映界面会再乘画布宽度展示像素值。因此这里先把后端给的像素位移
/// 除以画布宽度，再反向适配项目内部的 `Transform`。
fn build_subtitle_transform(cue: &VodSubtitleClip, canvas: &Canvas) -> Transform {
    Transform {
        x: jy_transform_to_internal(vod_subtitle_position_to_jy_transform(
            cue.x.unwrap_or(0.0),
            canvas,
        )),
        y: jy_transform_to_internal(vod_subtitle_position_to_jy_transform(
            cue.y.unwrap_or(0.0),
            canvas,
        )),
        ..Default::default()
    }
}

fn vod_subtitle_position_to_jy_transform(value: f64, canvas: &Canvas) -> f64 {
    value / canvas.width.max(1) as f64
}

fn jy_transform_to_internal(value: f64) -> f64 {
    value / 2.0 + 0.5
}

/// 构建视觉变换信息。
///
/// 对普通视频片段，当前先保守处理，不额外变换。
fn build_visual_transform(clip: &VodVisualClip) -> Option<Transform> {
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

/// 把同一条 VOD 轨上的重叠片段自动拆到额外 lane。
///
/// 阿里云 VOD 时间轴允许某些片段在同一轨道中出现交叠，剪映草稿则要求同一轨
/// 内片段不能重叠。这里保持原始时间不裁剪，只在需要时创建同类型额外轨道承载。
fn assign_timeline_lane(lanes: &mut Vec<Vec<PositionedClip>>, clip: PositionedClip) -> usize {
    for (lane_idx, lane) in lanes.iter_mut().enumerate() {
        if lane
            .iter()
            .all(|existing| !positioned_clips_overlap(*existing, clip))
        {
            lane.push(clip);
            return lane_idx;
        }
    }

    lanes.push(vec![clip]);
    lanes.len() - 1
}

fn positioned_clips_overlap(left: PositionedClip, right: PositionedClip) -> bool {
    !(left.end <= right.start || right.end <= left.start)
}

fn lane_count_for_track(
    lanes_by_track: &HashMap<usize, Vec<Vec<PositionedClip>>>,
    track_idx: usize,
) -> usize {
    lanes_by_track
        .get(&track_idx)
        .map(Vec::len)
        .unwrap_or(0)
        .max(1)
}

fn vod_track_name(prefix: &str, track_idx: usize, lane_idx: usize) -> String {
    if lane_idx == 0 {
        format!("{prefix}_{track_idx}")
    } else {
        format!("{prefix}_{track_idx}_overlap_{lane_idx}")
    }
}

fn render_index_offset(
    track_idx: usize,
    lane_idx: usize,
    base_track_count: usize,
    base_offset: i32,
) -> i32 {
    base_offset + (track_idx + lane_idx * base_track_count.max(1)) as i32
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
    remote_total: usize,
    completed_remote: usize,
    use_internal_url: bool,
}

impl AssetFetcher {
    /// 创建一个素材抓取器。
    fn new(
        client: Client,
        assets_dir: Utf8PathBuf,
        remote_total: usize,
        use_internal_url: bool,
    ) -> Self {
        Self {
            client,
            assets_dir,
            downloads: HashMap::new(),
            remote_total,
            completed_remote: 0,
            use_internal_url,
        }
    }

    fn remote_total(&self) -> usize {
        self.remote_total
    }

    fn completed_downloads(&self) -> usize {
        self.completed_remote
    }

    fn assets_dir(&self) -> &Utf8Path {
        &self.assets_dir
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
    fn download_remote(&mut self, source: &str) -> Result<Utf8PathBuf> {
        let url = Url::parse(source)?;
        let download_url = if self.use_internal_url {
            rewrite_aliyun_oss_url_to_internal(&url)
        } else {
            url.clone()
        };
        let internal_rewritten = download_url != url;
        let file_name = build_remote_file_name(&url);
        let path = self.assets_dir.join(file_name);
        let ordinal = self.completed_remote + 1;

        if path.exists() {
            output::emit_progress(
                "vod-json-to-draft",
                "reuse-asset",
                &format!(
                    "[asset {}/{}] Reusing existing file: {}",
                    ordinal, self.remote_total, path
                ),
                json!({
                    "ordinal": ordinal,
                    "total_assets": self.remote_total,
                    "source": source,
                    "download_source": download_url.as_str(),
                    "path": path.as_str(),
                    "reused": true,
                    "use_internal_url": self.use_internal_url,
                    "internal_rewritten": internal_rewritten,
                }),
            );
            self.completed_remote += 1;
            return Ok(path);
        }

        std::fs::create_dir_all(&self.assets_dir)?;
        output::emit_progress(
            "vod-json-to-draft",
            "download-asset",
            &format!(
                "[asset {}/{}] Downloading: {}",
                ordinal, self.remote_total, source
            ),
            json!({
                "ordinal": ordinal,
                "total_assets": self.remote_total,
                "source": source,
                "download_source": download_url.as_str(),
                "path": path.as_str(),
                "use_internal_url": self.use_internal_url,
                "internal_rewritten": internal_rewritten,
            }),
        );
        output::emit_progress(
            "vod-json-to-draft",
            "save-asset",
            &format!("Saving to: {}", path),
            json!({
                "ordinal": ordinal,
                "total_assets": self.remote_total,
                "source": source,
                "download_source": download_url.as_str(),
                "path": path.as_str(),
            }),
        );

        let mut response = self
            .client
            .get(download_url.as_str())
            .send()
            .with_context(|| format!("failed to download asset: {}", download_url.as_str()))?
            .error_for_status()
            .with_context(|| format!("remote asset returned error: {}", download_url.as_str()))?;
        let total_bytes = response.content_length();

        let mut file = File::create(path.as_std_path())?;
        let mut downloaded: u64 = 0;
        let mut buffer = [0u8; 64 * 1024];
        let mut next_report_at = total_bytes
            .map(|total| (total / 20).max(1))
            .unwrap_or(2 * 1024 * 1024);

        loop {
            let read = response.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            file.write_all(&buffer[..read])?;
            downloaded += read as u64;

            if downloaded >= next_report_at {
                print_download_progress(
                    ordinal,
                    self.remote_total,
                    source,
                    &path,
                    downloaded,
                    total_bytes,
                );
                next_report_at = total_bytes
                    .map(|total| (next_report_at + (total / 20).max(1)).min(total))
                    .unwrap_or(next_report_at + 2 * 1024 * 1024);
            }
        }
        file.flush()?;
        if downloaded == 0 {
            bail!("downloaded empty asset from {source}");
        }

        print_download_progress(
            ordinal,
            self.remote_total,
            source,
            &path,
            downloaded,
            total_bytes,
        );
        output::emit_progress(
            "vod-json-to-draft",
            "asset-finished",
            &format!(
                "[asset {}/{}] Finished: {}",
                ordinal, self.remote_total, path
            ),
            json!({
                "ordinal": ordinal,
                "total_assets": self.remote_total,
                "source": source,
                "download_source": download_url.as_str(),
                "path": path.as_str(),
                "bytes": downloaded,
                "use_internal_url": self.use_internal_url,
                "internal_rewritten": internal_rewritten,
            }),
        );
        self.completed_remote += 1;

        Ok(path)
    }
}

/// 将阿里云 OSS/VOD 常见公网 Endpoint 改写为同地域内网 Endpoint。
///
/// 例如：
/// `outin-xxx.oss-cn-shanghai.aliyuncs.com`
/// -> `outin-xxx.oss-cn-shanghai-internal.aliyuncs.com`
///
/// 自定义域名、加速域名、已经是 internal 的域名会保持原样。
fn rewrite_aliyun_oss_url_to_internal(url: &Url) -> Url {
    let Some(host) = url.host_str() else {
        return url.clone();
    };
    let Some(oss_marker) = host.find(".oss-") else {
        return url.clone();
    };

    let oss_start = oss_marker + 1;
    let endpoint = &host[oss_start..];
    if !endpoint.ends_with(".aliyuncs.com")
        || endpoint.contains("-internal.aliyuncs.com")
        || endpoint.starts_with("oss-accelerate")
    {
        return url.clone();
    }

    let internal_endpoint = endpoint
        .strip_suffix(".aliyuncs.com")
        .map(|prefix| format!("{prefix}-internal.aliyuncs.com"));
    let Some(internal_endpoint) = internal_endpoint else {
        return url.clone();
    };

    let new_host = format!("{}{}", &host[..oss_start], internal_endpoint);
    let mut rewritten = url.clone();
    if rewritten.set_host(Some(&new_host)).is_err() {
        return url.clone();
    }
    rewritten
}

fn print_download_progress(
    ordinal: usize,
    total_assets: usize,
    source: &str,
    path: &Utf8Path,
    downloaded: u64,
    total_bytes: Option<u64>,
) {
    if let Some(total_bytes) = total_bytes {
        let percent = if total_bytes == 0 {
            100.0
        } else {
            downloaded as f64 / total_bytes as f64 * 100.0
        };
        output::emit_progress(
            "vod-json-to-draft",
            "download-progress",
            &format!(
                "[asset {}/{}] {:.1}% ({}/{})",
                ordinal,
                total_assets,
                percent.clamp(0.0, 100.0),
                format_bytes(downloaded),
                format_bytes(total_bytes)
            ),
            json!({
                "ordinal": ordinal,
                "total_assets": total_assets,
                "source": source,
                "path": path.as_str(),
                "downloaded_bytes": downloaded,
                "total_bytes": total_bytes,
                "percent": percent.clamp(0.0, 100.0),
            }),
        );
    } else {
        output::emit_progress(
            "vod-json-to-draft",
            "download-progress",
            &format!(
                "[asset {}/{}] Downloaded {}",
                ordinal,
                total_assets,
                format_bytes(downloaded)
            ),
            json!({
                "ordinal": ordinal,
                "total_assets": total_assets,
                "source": source,
                "path": path.as_str(),
                "downloaded_bytes": downloaded,
            }),
        );
    }
}

fn format_bytes(bytes: u64) -> String {
    const KB: f64 = 1024.0;
    const MB: f64 = KB * 1024.0;
    const GB: f64 = MB * 1024.0;

    let bytes_f = bytes as f64;
    if bytes_f >= GB {
        format!("{:.2} GB", bytes_f / GB)
    } else if bytes_f >= MB {
        format!("{:.2} MB", bytes_f / MB)
    } else if bytes_f >= KB {
        format!("{:.2} KB", bytes_f / KB)
    } else {
        format!("{bytes} B")
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
    fn assigns_overlapping_ranges_to_extra_lanes() {
        let mut lanes = Vec::new();
        assert_eq!(
            assign_timeline_lane(
                &mut lanes,
                PositionedClip {
                    start: 0,
                    end: 5_000_000,
                },
            ),
            0
        );
        assert_eq!(
            assign_timeline_lane(
                &mut lanes,
                PositionedClip {
                    start: 4_000_000,
                    end: 6_000_000,
                },
            ),
            1
        );
        assert_eq!(
            assign_timeline_lane(
                &mut lanes,
                PositionedClip {
                    start: 6_000_000,
                    end: 8_000_000,
                },
            ),
            0
        );
    }

    #[test]
    fn vod_subtitle_overlap_generates_extra_text_track() {
        let temp = tempfile::tempdir().unwrap();
        let config = Utf8PathBuf::from_path_buf(temp.path().join("vod.json")).unwrap();
        let output = Utf8PathBuf::from_path_buf(temp.path().join("draft")).unwrap();

        std::fs::write(
            &config,
            serde_json::to_string_pretty(&json!({
                "OutputMediaConfig": { "Width": 1080, "Height": 1920 },
                "SubtitleTracks": [
                    {
                        "SubtitleTrackClips": [
                            {
                                "Content": "第一条字幕",
                                "TimelineIn": 213.112,
                                "TimelineOut": 218.186,
                                "X": 0.0,
                                "Y": -899.0,
                                "FontSize": 65.0,
                                "Outline": 10,
                                "OutlineColour": "#000000",
                                "Shadow": 1,
                                "AdaptMode": "AutoWrapAtSpaces"
                            },
                            {
                                "Content": "重叠字幕",
                                "TimelineIn": 217.0,
                                "TimelineOut": 219.0,
                                "X": 0.0,
                                "Y": -899.0,
                                "FontSize": 65.0,
                                "Outline": 10,
                                "OutlineColour": "#000000",
                                "Shadow": 1
                            }
                        ]
                    }
                ]
            }))
            .unwrap(),
        )
        .unwrap();

        run(&config, None, &output, Some("overlap"), false).unwrap();

        let draft: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(output.join("draft_content.json")).unwrap(),
        )
        .unwrap();
        let text_tracks = draft["tracks"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|track| track["type"].as_str() == Some("text"))
            .count();
        assert_eq!(text_tracks, 2);

        let first_transform = draft["tracks"]
            .as_array()
            .unwrap()
            .iter()
            .find(|track| track["type"].as_str() == Some("text"))
            .and_then(|track| track["segments"].as_array())
            .and_then(|segments| segments.first())
            .map(|segment| &segment["clip"]["transform"])
            .unwrap();
        assert_eq!(first_transform["x"].as_f64(), Some(0.0));
        assert_eq!(first_transform["y"].as_f64(), Some(-899.0 / 1080.0));

        let first_text_content = draft["materials"]["texts"][0]["content"].as_str().unwrap();
        let first_text_content: serde_json::Value =
            serde_json::from_str(first_text_content).unwrap();
        let first_style = &first_text_content["styles"][0];
        assert_eq!(first_style["size"].as_f64(), Some(65.0));
        let stroke_width = first_style["strokes"][0]["width"].as_f64().unwrap();
        assert!((stroke_width - 0.02).abs() < f64::EPSILON);
        assert_eq!(
            draft["materials"]["texts"][0]["type"].as_str(),
            Some("subtitle")
        );
    }

    #[test]
    fn parses_hex_rgb() {
        assert_eq!(parse_hex_rgb("#ffffff"), (1.0, 1.0, 1.0));
        assert_eq!(parse_hex_rgb("#000000"), (0.0, 0.0, 0.0));
    }

    #[test]
    fn collects_unique_remote_sources() {
        let project = VodProject {
            fe_canvas: None,
            output_media_config: None,
            subtitle_tracks: vec![],
            video_tracks: vec![VodVideoTrack {
                clips: vec![
                    VodVisualClip {
                        clip_type: None,
                        media_url: "https://example.com/a.mp4".to_string(),
                        timeline_in: None,
                        timeline_out: None,
                        adapt_mode: None,
                    },
                    VodVisualClip {
                        clip_type: None,
                        media_url: "https://example.com/a.mp4".to_string(),
                        timeline_in: None,
                        timeline_out: None,
                        adapt_mode: None,
                    },
                    VodVisualClip {
                        clip_type: Some("GlobalImage".to_string()),
                        media_url: "https://example.com/watermark.png".to_string(),
                        timeline_in: None,
                        timeline_out: None,
                        adapt_mode: None,
                    },
                ],
            }],
            audio_tracks: vec![VodAudioTrack {
                clips: vec![VodAudioClipDef {
                    media_url: "https://example.com/b.wav".to_string(),
                    timeline_in: None,
                    timeline_out: None,
                }],
            }],
        };

        assert_eq!(
            collect_remote_sources(&project),
            vec![
                "https://example.com/a.mp4".to_string(),
                "https://example.com/b.wav".to_string()
            ]
        );
    }

    #[test]
    fn rewrites_aliyun_oss_endpoint_to_internal() {
        let url =
            Url::parse("https://outin-123.oss-cn-shanghai.aliyuncs.com/video/a.mp4?Expires=1")
                .unwrap();
        let rewritten = rewrite_aliyun_oss_url_to_internal(&url);

        assert_eq!(
            rewritten.as_str(),
            "https://outin-123.oss-cn-shanghai-internal.aliyuncs.com/video/a.mp4?Expires=1"
        );
    }

    #[test]
    fn does_not_rewrite_existing_internal_or_custom_domain() {
        let internal =
            Url::parse("https://outin-123.oss-cn-shanghai-internal.aliyuncs.com/a.mp4").unwrap();
        assert_eq!(rewrite_aliyun_oss_url_to_internal(&internal), internal);

        let custom = Url::parse("https://media.example.com/a.mp4").unwrap();
        assert_eq!(rewrite_aliyun_oss_url_to_internal(&custom), custom);
    }
}
