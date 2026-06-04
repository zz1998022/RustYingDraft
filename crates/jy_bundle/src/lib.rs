use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Write};
use std::path::{Path, PathBuf};

use anyhow::{anyhow, bail, Context, Result};
use camino::{Utf8Path, Utf8PathBuf};
use jy_draft::writer::write_draft;
use jy_media::material::{create_audio_material, create_video_material};
use jy_schema::{
    parse_time_str, Canvas, Clip, MaterialKind, TextAlign, TextBackground, TextBorder, TextShadow,
    TextStyle, TimeRange, TrackKind, Transform, SEC,
};
use jy_timeline::builder::ProjectBuilder;
use jy_timeline::clip::{make_audio_clip, make_image_clip, make_text_clip, make_video_clip};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use tempfile::TempDir;
use url::Url;
use uuid::Uuid;
use zip::ZipArchive;

#[derive(Debug, Clone)]
pub struct ImportBundleOptions {
    pub source: Utf8PathBuf,
    pub output: Utf8PathBuf,
    pub name_override: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportBundleSummary {
    pub source: String,
    pub bundle_root: String,
    pub bundle_type: String,
    pub timeline_file: Option<String>,
    pub source_draft_dir: Option<String>,
    pub draft_dir: String,
    pub project_id: String,
    pub name: String,
    pub duration: u64,
    pub track_count: usize,
    pub asset_count: usize,
    pub video_material_count: usize,
    pub audio_material_count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BundleInspection {
    pub source: String,
    pub bundle_root: String,
    pub bundle_type: String,
    pub timeline_file: Option<String>,
    pub source_draft_dir: Option<String>,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub asset_count: usize,
    pub track_count: usize,
    pub asset_kinds: Vec<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImportBundleProgress {
    pub stage: String,
    pub message: String,
    pub data: Value,
}

#[derive(Debug, Deserialize)]
struct BundleManifest {
    #[serde(default)]
    bundle_version: u32,
    #[serde(default)]
    bundle_type: BundleType,
    project_id: Option<String>,
    project_name: Option<String>,
    timeline_file: Option<String>,
    assets_dir: Option<String>,
    draft_dir: Option<String>,
    #[serde(default)]
    match_key: DraftMatchKey,
    #[serde(default)]
    assets: Vec<DraftAssetBinding>,
    pipeline: Option<PipelineSpec>,
    #[serde(default)]
    subtitle_style: SimpleSubtitleStyle,
    #[serde(default)]
    audio_style: PipelineAudioStyle,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum BundleType {
    #[default]
    TimelinePackage,
    DraftPackage,
    SimpleTimelinePackage,
    PipelinePackage,
}

#[derive(Debug, Deserialize)]
struct TimelineManifest {
    project: TimelineProject,
    #[serde(default)]
    canvas: Canvas,
    #[serde(default = "default_maintrack_adsorb")]
    maintrack_adsorb: bool,
    #[serde(default)]
    assets: Vec<AssetSpec>,
    #[serde(default)]
    tracks: Vec<TrackSpec>,
}

#[derive(Debug, Deserialize)]
struct TimelineProject {
    id: Option<String>,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SimpleTimelineManifest {
    project: Option<TimelineProject>,
    #[serde(default)]
    canvas: Canvas,
    #[serde(default)]
    videos: Vec<SimpleVideoSpec>,
    #[serde(default)]
    subtitle_style: SimpleSubtitleStyle,
    #[serde(default)]
    subtitles: Vec<SimpleSubtitleCue>,
}

#[derive(Debug, Deserialize)]
struct SimpleVideoSpec {
    path: String,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct SimpleSubtitleStyle {
    #[serde(default = "default_simple_subtitle_font_size")]
    font_size: f64,
    #[serde(default = "default_simple_subtitle_x")]
    x: f64,
    #[serde(default = "default_simple_subtitle_y")]
    y: f64,
}

impl Default for SimpleSubtitleStyle {
    fn default() -> Self {
        Self {
            font_size: default_simple_subtitle_font_size(),
            x: default_simple_subtitle_x(),
            y: default_simple_subtitle_y(),
        }
    }
}

fn default_simple_subtitle_font_size() -> f64 {
    8.0
}

fn default_simple_subtitle_x() -> f64 {
    0.5
}

fn default_simple_subtitle_y() -> f64 {
    0.82
}

#[derive(Debug, Deserialize)]
struct SimpleSubtitleCue {
    start: f64,
    end: f64,
    text: String,
}

#[derive(Debug, Deserialize)]
struct PipelineSpec {
    concat_file: String,
    subtitle_file: String,
    #[serde(default)]
    narration_files: Vec<String>,
    audio_file: Option<String>,
}

#[derive(Debug, Deserialize)]
struct PipelineAudioStyle {
    #[serde(default = "default_pipeline_volume")]
    video_volume: f64,
    #[serde(default = "default_pipeline_volume")]
    narration_volume: f64,
    audio_volume: Option<f64>,
}

impl Default for PipelineAudioStyle {
    fn default() -> Self {
        Self {
            video_volume: default_pipeline_volume(),
            narration_volume: default_pipeline_volume(),
            audio_volume: None,
        }
    }
}

fn default_pipeline_volume() -> f64 {
    1.0
}

#[derive(Debug, Deserialize)]
struct AssetSpec {
    id: String,
    kind: AssetKind,
    #[serde(default)]
    source: AssetSourceSpec,
    name: Option<String>,
}

#[derive(Debug, Deserialize)]
struct DraftAssetBinding {
    kind: AssetKind,
    match_value: String,
    relative_path: String,
    #[serde(rename = "name")]
    _name: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
enum AssetKind {
    Video,
    Audio,
    Image,
}

impl AssetKind {
    fn as_str(&self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Image => "image",
        }
    }
}

#[derive(Debug, Deserialize, Default)]
struct AssetSourceSpec {
    #[serde(rename = "type")]
    source_type: AssetSourceType,
    path: Option<String>,
    url: Option<String>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum AssetSourceType {
    #[default]
    BundlePath,
    LocalPath,
    Url,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
enum DraftMatchKey {
    #[default]
    Name,
}

#[derive(Debug, Deserialize)]
struct TrackSpec {
    id: Option<String>,
    kind: ImportTrackKind,
    name: String,
    render_index: Option<i32>,
    #[serde(default)]
    mute: bool,
    #[serde(default)]
    clips: Vec<ClipSpec>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImportTrackKind {
    Video,
    Audio,
    Text,
}

#[derive(Debug, Deserialize)]
struct ClipSpec {
    id: Option<String>,
    #[serde(rename = "type")]
    clip_type: ImportClipType,
    asset_id: Option<String>,
    timeline_in: TimeValue,
    timeline_out: TimeValue,
    source_in: Option<TimeValue>,
    source_out: Option<TimeValue>,
    text: Option<String>,
    volume: Option<f64>,
    speed: Option<f64>,
    transform: Option<Transform>,
    style: Option<TextStyle>,
    border: Option<TextBorder>,
    background: Option<TextBackground>,
    shadow: Option<TextShadow>,
}

#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "snake_case")]
enum ImportClipType {
    Video,
    Audio,
    Image,
    Text,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
enum TimeValue {
    Integer(u64),
    Float(f64),
    String(String),
}

#[derive(Debug)]
struct PreparedSource {
    bundle_root: Utf8PathBuf,
    temp_dir: TempDir,
}

#[derive(Debug, Clone)]
enum ImportedMaterial {
    Video(jy_schema::VideoMaterialRef),
    Audio(jy_schema::AudioMaterialRef),
}

fn default_maintrack_adsorb() -> bool {
    true
}

pub fn import_bundle(options: &ImportBundleOptions) -> Result<ImportBundleSummary> {
    import_bundle_with_progress(options, |_| {})
}

pub fn import_bundle_with_progress<F>(
    options: &ImportBundleOptions,
    mut progress: F,
) -> Result<ImportBundleSummary>
where
    F: FnMut(ImportBundleProgress),
{
    let prepared = PreparedSource::from_source(&options.source)?;
    let bundle = read_json::<BundleManifest>(&prepared.bundle_root.join("bundle.json"))?;
    if bundle.bundle_version > 1 {
        bail!("unsupported bundle_version: {}", bundle.bundle_version);
    }
    match bundle.bundle_type {
        BundleType::TimelinePackage => {
            import_timeline_package(options, &prepared, &bundle, &mut progress)
        }
        BundleType::DraftPackage => {
            import_draft_package(options, &prepared, &bundle, &mut progress)
        }
        BundleType::SimpleTimelinePackage => {
            import_simple_timeline_package(options, &prepared, &bundle)
        }
        BundleType::PipelinePackage => import_pipeline_package(options, &prepared, &bundle),
    }
}

pub fn inspect_bundle_source(source: &Utf8Path) -> Result<BundleInspection> {
    let prepared = PreparedSource::from_source(source)?;
    let bundle = read_json::<BundleManifest>(&prepared.bundle_root.join("bundle.json"))?;
    if bundle.bundle_version > 1 {
        bail!("unsupported bundle_version: {}", bundle.bundle_version);
    }

    match bundle.bundle_type {
        BundleType::TimelinePackage => inspect_timeline_package(source, &prepared, &bundle),
        BundleType::DraftPackage => inspect_draft_package(source, &prepared, &bundle),
        BundleType::SimpleTimelinePackage => {
            inspect_simple_timeline_package(source, &prepared, &bundle)
        }
        BundleType::PipelinePackage => inspect_pipeline_package(source, &prepared, &bundle),
    }
}

fn import_timeline_package<F>(
    options: &ImportBundleOptions,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
    progress: &mut F,
) -> Result<ImportBundleSummary>
where
    F: FnMut(ImportBundleProgress),
{
    let timeline_file = bundle
        .timeline_file
        .as_deref()
        .unwrap_or("timeline.json")
        .to_string();
    let timeline = read_json::<TimelineManifest>(&prepared.bundle_root.join(&timeline_file))?;

    let project_name = options
        .name_override
        .clone()
        .or_else(|| timeline.project.name.clone())
        .or_else(|| bundle.project_name.clone())
        .unwrap_or_else(|| "imported_bundle".to_string());

    let mut builder = ProjectBuilder::new(&project_name, timeline.canvas.clone())
        .maintrack_adsorb(timeline.maintrack_adsorb);

    for track in &timeline.tracks {
        builder = builder.add_track(
            track.kind.into(),
            &track.name,
            track_render_index_offset(track.kind, track.render_index),
        )?;
    }

    let cache_root = utf8_path_buf(prepared.temp_dir.path().to_path_buf())?;
    let mut resolved_assets = HashMap::new();
    for asset in &timeline.assets {
        let material = resolve_asset(
            asset,
            &prepared.bundle_root,
            bundle.assets_dir.as_deref(),
            &cache_root,
            progress,
        )?;
        match &material {
            ImportedMaterial::Video(video) => {
                builder = builder.add_video_material(video.clone());
            }
            ImportedMaterial::Audio(audio) => {
                builder = builder.add_audio_material(audio.clone());
            }
        }
        resolved_assets.insert(asset.id.clone(), material);
    }

    for track in &timeline.tracks {
        for clip in &track.clips {
            let built_clip = build_clip(clip, &resolved_assets)
                .with_context(|| format!("failed to build clip on track '{}'", track.name))?;
            builder = builder.add_clip_to_track(&track.name, built_clip)?;
        }
    }

    let mut project = builder.build();
    project.id = timeline
        .project
        .id
        .clone()
        .or(bundle.project_id.clone())
        .unwrap_or(project.id);

    for project_track in &mut project.tracks {
        if let Some(track_spec) = timeline
            .tracks
            .iter()
            .find(|track| track.name == project_track.name)
        {
            project_track.mute = track_spec.mute;
            if let Some(id) = &track_spec.id {
                project_track.id = id.clone();
            }
            if let Some(render_index) = track_spec.render_index {
                project_track.render_index = render_index;
            }
        }
    }

    ensure_output_dir_ready(&options.output)?;
    write_draft(&project, &options.output)?;

    Ok(ImportBundleSummary {
        source: options.source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "timeline_package".to_string(),
        timeline_file: Some(timeline_file),
        source_draft_dir: None,
        draft_dir: options.output.as_str().to_string(),
        project_id: project.id,
        name: project.name,
        duration: project.duration,
        track_count: project.tracks.len(),
        asset_count: timeline.assets.len(),
        video_material_count: project.video_materials.len(),
        audio_material_count: project.audio_materials.len(),
    })
}

fn import_draft_package<F>(
    options: &ImportBundleOptions,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
    progress: &mut F,
) -> Result<ImportBundleSummary>
where
    F: FnMut(ImportBundleProgress),
{
    if bundle.match_key != DraftMatchKey::Name {
        bail!("unsupported match_key for draft_package");
    }
    if bundle.assets.is_empty() {
        bail!("draft_package requires bundle.assets");
    }

    let source_draft_dir = prepared
        .bundle_root
        .join(bundle.draft_dir.as_deref().unwrap_or("draft"));
    if !source_draft_dir.exists() {
        bail!("draft source directory not found: {source_draft_dir}");
    }

    ensure_output_dir_ready(&options.output)?;
    copy_dir_all(&source_draft_dir, &options.output)?;

    let cache_root = utf8_path_buf(prepared.temp_dir.path().to_path_buf())?;

    let mut replacements = Vec::new();
    for (index, asset) in bundle.assets.iter().enumerate() {
        let replacement = resolve_draft_binding(
            asset,
            &prepared.bundle_root,
            bundle.assets_dir.as_deref(),
            &cache_root,
            &options.output,
            index,
            progress,
        )?;
        replacements.push(replacement);
    }

    let final_name = options
        .name_override
        .clone()
        .or_else(|| bundle.project_name.clone())
        .unwrap_or_else(|| "imported_bundle".to_string());

    rewrite_draft_package_snapshots(&options.output, &replacements)?;
    rewrite_meta_info(&options.output, &final_name)?;

    let draft_json: Value = serde_json::from_str(&std::fs::read_to_string(
        options.output.join("draft_content.json"),
    )?)?;
    let track_count = draft_json
        .get("tracks")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let video_material_count = draft_json
        .pointer("/materials/videos")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let audio_material_count = draft_json
        .pointer("/materials/audios")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);
    let duration = draft_json
        .get("duration")
        .and_then(Value::as_u64)
        .unwrap_or(0);

    Ok(ImportBundleSummary {
        source: options.source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "draft_package".to_string(),
        timeline_file: None,
        source_draft_dir: Some(source_draft_dir.as_str().to_string()),
        draft_dir: options.output.as_str().to_string(),
        project_id: bundle
            .project_id
            .clone()
            .unwrap_or_else(|| Uuid::new_v4().as_simple().to_string()),
        name: final_name,
        duration,
        track_count,
        asset_count: bundle.assets.len(),
        video_material_count,
        audio_material_count,
    })
}

fn import_simple_timeline_package(
    options: &ImportBundleOptions,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
) -> Result<ImportBundleSummary> {
    let timeline_file = bundle
        .timeline_file
        .as_deref()
        .unwrap_or("timeline.json")
        .to_string();
    let timeline = read_json::<SimpleTimelineManifest>(&prepared.bundle_root.join(&timeline_file))?;

    validate_simple_timeline(&timeline)?;

    let project_name = options
        .name_override
        .clone()
        .or_else(|| {
            timeline
                .project
                .as_ref()
                .and_then(|project| project.name.clone())
        })
        .or_else(|| bundle.project_name.clone())
        .unwrap_or_else(|| "imported_bundle".to_string());

    let mut builder = ProjectBuilder::new(&project_name, timeline.canvas.clone())
        .maintrack_adsorb(true)
        .add_track(TrackKind::Video, "main_video", 0)?
        .add_track(TrackKind::Text, "subtitle", 0)?;

    let mut cursor = 0_u64;
    for (index, video) in timeline.videos.iter().enumerate() {
        let material_path = resolve_simple_asset_path(
            &video.path,
            &prepared.bundle_root,
            bundle.assets_dir.as_deref(),
        )
        .with_context(|| format!("invalid video[{index}] path"))?;
        let material = create_video_material(&material_path, video.name.as_deref())
            .with_context(|| format!("failed to load video[{index}]: {}", video.path))?;
        if material.kind != MaterialKind::Video {
            bail!("video[{index}] is not a video material: {}", video.path);
        }
        if material.duration == 0 {
            bail!("video[{index}] duration is zero: {}", video.path);
        }

        let target = TimeRange::new(cursor, material.duration);
        let clip = make_video_clip(&material, target, None, None, 1.0, None)
            .with_context(|| format!("failed to build video[{index}] clip"))?;
        cursor = cursor
            .checked_add(material.duration)
            .ok_or_else(|| anyhow!("simple_timeline_package duration overflow"))?;

        builder = builder
            .add_video_material(material)
            .add_clip_to_track("main_video", clip)?;
    }

    let style = build_simple_subtitle_style(&timeline.subtitle_style)?;
    let transform = build_simple_subtitle_transform(&timeline.subtitle_style)?;
    for (index, cue) in timeline.subtitles.iter().enumerate() {
        let start = seconds_to_micros(cue.start, &format!("subtitles[{index}].start"))?;
        let end = seconds_to_micros(cue.end, &format!("subtitles[{index}].end"))?;
        if end <= start {
            bail!("subtitles[{index}] end must be greater than start");
        }
        if end > cursor {
            bail!("subtitles[{index}] end exceeds stitched video duration");
        }

        let clip = make_text_clip(
            &cue.text,
            TimeRange::new(start, end - start),
            Some(style.clone()),
            Some(transform.clone()),
        );
        builder = builder.add_clip_to_track("subtitle", clip)?;
    }

    let mut project = builder.build();
    project.id = timeline
        .project
        .as_ref()
        .and_then(|project| project.id.clone())
        .or(bundle.project_id.clone())
        .unwrap_or(project.id);

    ensure_output_dir_ready(&options.output)?;
    write_draft(&project, &options.output)?;

    Ok(ImportBundleSummary {
        source: options.source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "simple_timeline_package".to_string(),
        timeline_file: Some(timeline_file),
        source_draft_dir: None,
        draft_dir: options.output.as_str().to_string(),
        project_id: project.id,
        name: project.name,
        duration: project.duration,
        track_count: project.tracks.len(),
        asset_count: timeline.videos.len(),
        video_material_count: project.video_materials.len(),
        audio_material_count: project.audio_materials.len(),
    })
}

fn import_pipeline_package(
    options: &ImportBundleOptions,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
) -> Result<ImportBundleSummary> {
    let pipeline = bundle
        .pipeline
        .as_ref()
        .ok_or_else(|| anyhow!("pipeline_package requires bundle.pipeline"))?;
    validate_simple_subtitle_style(&bundle.subtitle_style)?;
    validate_pipeline_audio_style(&bundle.audio_style)?;

    validate_pipeline_spec(pipeline)?;
    let concat_path = resolve_pipeline_asset_path(
        "pipeline.concat_file",
        &pipeline.concat_file,
        &prepared.bundle_root,
        bundle.assets_dir.as_deref(),
    )?;
    let subtitle_path = resolve_pipeline_asset_path(
        "pipeline.subtitle_file",
        &pipeline.subtitle_file,
        &prepared.bundle_root,
        bundle.assets_dir.as_deref(),
    )?;

    let video_files = parse_concat_file(&concat_path)
        .with_context(|| format!("failed to parse concat file: {concat_path}"))?;
    let subtitles = parse_srt_file(&subtitle_path)
        .with_context(|| format!("failed to parse subtitle file: {subtitle_path}"))?;
    validate_pipeline_narration_count(&pipeline.narration_files, &subtitles)?;

    let project_name = options
        .name_override
        .clone()
        .or_else(|| bundle.project_name.clone())
        .unwrap_or_else(|| "imported_bundle".to_string());

    let mut builder = ProjectBuilder::new(&project_name, Canvas::default())
        .maintrack_adsorb(true)
        .add_track(TrackKind::Video, "main_video", 0)?
        .add_track(TrackKind::Audio, "audio", 0)?
        .add_track(TrackKind::Text, "subtitle", 0)?;

    let mut cursor = 0_u64;
    for (index, video_file) in video_files.iter().enumerate() {
        let material_path = resolve_pipeline_asset_path(
            &format!("concat file[{index}]"),
            video_file,
            &prepared.bundle_root,
            bundle.assets_dir.as_deref(),
        )
        .with_context(|| format!("invalid concat file[{index}] path"))?;
        let material = create_video_material(&material_path, None)
            .with_context(|| format!("failed to load concat video[{index}]: {video_file}"))?;
        if material.kind != MaterialKind::Video {
            bail!("concat video[{index}] is not a video material: {video_file}");
        }
        if material.duration == 0 {
            bail!("concat video[{index}] duration is zero: {video_file}");
        }

        let target = TimeRange::new(cursor, material.duration);
        let clip = make_video_clip(
            &material,
            target,
            None,
            None,
            bundle.audio_style.video_volume,
            None,
        )
        .with_context(|| format!("failed to build concat video[{index}] clip"))?;
        cursor = cursor
            .checked_add(material.duration)
            .ok_or_else(|| anyhow!("pipeline_package duration overflow"))?;

        builder = builder
            .add_video_material(material)
            .add_clip_to_track("main_video", clip)?;
    }

    let text_style = build_simple_subtitle_style(&bundle.subtitle_style)?;
    let text_transform = build_simple_subtitle_transform(&bundle.subtitle_style)?;
    for (index, cue) in subtitles.iter().enumerate() {
        let start = seconds_to_micros(cue.start, &format!("subtitle.srt[{index}].start"))?;
        let end = seconds_to_micros(cue.end, &format!("subtitle.srt[{index}].end"))?;
        if end > cursor {
            bail!("subtitle.srt[{index}] end exceeds stitched video duration");
        }
        let narration_file = &pipeline.narration_files[index];
        let narration_path = resolve_pipeline_asset_path(
            &format!("pipeline.narration_files[{index}]"),
            narration_file,
            &prepared.bundle_root,
            bundle.assets_dir.as_deref(),
        )
        .with_context(|| format!("invalid pipeline.narration_files[{index}] path"))?;
        let audio_material = create_audio_material(&narration_path, None).with_context(|| {
            format!("failed to load pipeline narration[{index}]: {narration_file}")
        })?;
        let cue_duration = end - start;
        let audio_duration = audio_material.duration.min(cue_duration);
        if audio_duration == 0 {
            bail!("pipeline narration[{index}] duration is zero");
        }
        let audio_clip = make_audio_clip(
            &audio_material,
            TimeRange::new(start, audio_duration),
            Some(TimeRange::new(0, audio_duration)),
            None,
            bundle.audio_style.narration_volume,
        )?;
        builder = builder
            .add_audio_material(audio_material)
            .add_clip_to_track("audio", audio_clip)?;

        let clip = make_text_clip(
            &cue.text,
            TimeRange::new(start, end - start),
            Some(text_style.clone()),
            Some(text_transform.clone()),
        );
        builder = builder.add_clip_to_track("subtitle", clip)?;
    }

    let mut project = builder.build();
    project.id = bundle.project_id.clone().unwrap_or(project.id);

    ensure_output_dir_ready(&options.output)?;
    write_draft(&project, &options.output)?;

    Ok(ImportBundleSummary {
        source: options.source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "pipeline_package".to_string(),
        timeline_file: None,
        source_draft_dir: None,
        draft_dir: options.output.as_str().to_string(),
        project_id: project.id,
        name: project.name,
        duration: project.duration,
        track_count: project.tracks.len(),
        asset_count: video_files.len() + pipeline.narration_files.len(),
        video_material_count: project.video_materials.len(),
        audio_material_count: project.audio_materials.len(),
    })
}

fn inspect_timeline_package(
    source: &Utf8Path,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
) -> Result<BundleInspection> {
    let timeline_file = bundle
        .timeline_file
        .as_deref()
        .unwrap_or("timeline.json")
        .to_string();
    let timeline = read_json::<TimelineManifest>(&prepared.bundle_root.join(&timeline_file))?;

    Ok(BundleInspection {
        source: source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "timeline_package".to_string(),
        timeline_file: Some(timeline_file),
        source_draft_dir: None,
        project_id: timeline.project.id.or(bundle.project_id.clone()),
        project_name: timeline.project.name.or(bundle.project_name.clone()),
        asset_count: timeline.assets.len(),
        track_count: timeline.tracks.len(),
        asset_kinds: timeline
            .assets
            .iter()
            .map(|asset| asset.kind.as_str().to_string())
            .collect(),
    })
}

fn inspect_simple_timeline_package(
    source: &Utf8Path,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
) -> Result<BundleInspection> {
    let timeline_file = bundle
        .timeline_file
        .as_deref()
        .unwrap_or("timeline.json")
        .to_string();
    let timeline = read_json::<SimpleTimelineManifest>(&prepared.bundle_root.join(&timeline_file))?;
    validate_simple_timeline(&timeline)?;
    for (index, video) in timeline.videos.iter().enumerate() {
        resolve_simple_asset_path(
            &video.path,
            &prepared.bundle_root,
            bundle.assets_dir.as_deref(),
        )
        .with_context(|| format!("invalid video[{index}] path"))?;
    }

    Ok(BundleInspection {
        source: source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "simple_timeline_package".to_string(),
        timeline_file: Some(timeline_file),
        source_draft_dir: None,
        project_id: timeline
            .project
            .as_ref()
            .and_then(|project| project.id.clone())
            .or(bundle.project_id.clone()),
        project_name: timeline
            .project
            .as_ref()
            .and_then(|project| project.name.clone())
            .or(bundle.project_name.clone()),
        asset_count: timeline.videos.len(),
        track_count: 2,
        asset_kinds: timeline
            .videos
            .iter()
            .map(|_| "video".to_string())
            .collect(),
    })
}

fn inspect_pipeline_package(
    source: &Utf8Path,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
) -> Result<BundleInspection> {
    let pipeline = bundle
        .pipeline
        .as_ref()
        .ok_or_else(|| anyhow!("pipeline_package requires bundle.pipeline"))?;
    validate_simple_subtitle_style(&bundle.subtitle_style)?;
    validate_pipeline_audio_style(&bundle.audio_style)?;
    validate_pipeline_spec(pipeline)?;

    let concat_path = resolve_pipeline_asset_path(
        "pipeline.concat_file",
        &pipeline.concat_file,
        &prepared.bundle_root,
        bundle.assets_dir.as_deref(),
    )?;
    let subtitle_path = resolve_pipeline_asset_path(
        "pipeline.subtitle_file",
        &pipeline.subtitle_file,
        &prepared.bundle_root,
        bundle.assets_dir.as_deref(),
    )?;

    let video_files = parse_concat_file(&concat_path)
        .with_context(|| format!("failed to parse concat file: {concat_path}"))?;
    let subtitles = parse_srt_file(&subtitle_path)
        .with_context(|| format!("failed to parse subtitle file: {subtitle_path}"))?;
    validate_pipeline_narration_count(&pipeline.narration_files, &subtitles)?;
    for (index, narration_file) in pipeline.narration_files.iter().enumerate() {
        resolve_pipeline_asset_path(
            &format!("pipeline.narration_files[{index}]"),
            narration_file,
            &prepared.bundle_root,
            bundle.assets_dir.as_deref(),
        )
        .with_context(|| format!("invalid pipeline.narration_files[{index}] path"))?;
    }

    let mut asset_kinds = video_files
        .iter()
        .map(|_| "video".to_string())
        .collect::<Vec<_>>();
    asset_kinds.extend(pipeline.narration_files.iter().map(|_| "audio".to_string()));

    Ok(BundleInspection {
        source: source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "pipeline_package".to_string(),
        timeline_file: None,
        source_draft_dir: None,
        project_id: bundle.project_id.clone(),
        project_name: bundle.project_name.clone(),
        asset_count: video_files.len() + pipeline.narration_files.len(),
        track_count: 3,
        asset_kinds,
    })
}

fn inspect_draft_package(
    source: &Utf8Path,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
) -> Result<BundleInspection> {
    let source_draft_dir = prepared
        .bundle_root
        .join(bundle.draft_dir.as_deref().unwrap_or("draft"));
    let draft_json: Value = serde_json::from_str(&std::fs::read_to_string(
        source_draft_dir.join("draft_content.json"),
    )?)?;
    let track_count = draft_json
        .get("tracks")
        .and_then(Value::as_array)
        .map(Vec::len)
        .unwrap_or(0);

    Ok(BundleInspection {
        source: source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "draft_package".to_string(),
        timeline_file: None,
        source_draft_dir: Some(source_draft_dir.as_str().to_string()),
        project_id: bundle.project_id.clone(),
        project_name: bundle.project_name.clone(),
        asset_count: bundle.assets.len(),
        track_count,
        asset_kinds: bundle
            .assets
            .iter()
            .map(|asset| asset.kind.as_str().to_string())
            .collect(),
    })
}

impl PreparedSource {
    fn from_source(source: &Utf8Path) -> Result<Self> {
        if source.is_dir() {
            let temp_dir = TempDir::new().context("failed to create temporary import workspace")?;
            return Ok(Self {
                bundle_root: find_bundle_root(source)?,
                temp_dir,
            });
        }

        if source.is_file() && source.file_name() == Some("bundle.json") {
            let temp_dir = TempDir::new().context("failed to create temporary import workspace")?;
            let bundle_root = source
                .parent()
                .ok_or_else(|| anyhow!("bundle.json must have a parent directory"))?
                .to_path_buf();
            return Ok(Self {
                bundle_root,
                temp_dir,
            });
        }

        if source.is_file() {
            let temp_dir =
                TempDir::new().context("failed to create temporary extraction directory")?;
            extract_zip_archive(source, temp_dir.path())?;
            let temp_root = utf8_path_buf(temp_dir.path().to_path_buf())?;
            return Ok(Self {
                bundle_root: find_bundle_root(&temp_root)?,
                temp_dir,
            });
        }

        bail!("bundle source does not exist: {source}");
    }
}

impl From<ImportTrackKind> for TrackKind {
    fn from(value: ImportTrackKind) -> Self {
        match value {
            ImportTrackKind::Video => TrackKind::Video,
            ImportTrackKind::Audio => TrackKind::Audio,
            ImportTrackKind::Text => TrackKind::Text,
        }
    }
}

impl TimeValue {
    fn as_micros(&self) -> Result<u64> {
        match self {
            TimeValue::Integer(value) => Ok(*value),
            TimeValue::Float(value) => {
                if *value < 0.0 {
                    bail!("time value must not be negative");
                }
                Ok((value * SEC as f64) as u64)
            }
            TimeValue::String(value) => {
                if let Ok(raw) = value.parse::<u64>() {
                    Ok(raw)
                } else {
                    parse_time_str(value)
                        .map_err(|error| anyhow!("invalid time value '{value}': {error}"))
                }
            }
        }
    }
}

fn read_json<T: for<'de> Deserialize<'de>>(path: &Utf8Path) -> Result<T> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read json file: {path}"))?;
    serde_json::from_str(&content).with_context(|| format!("failed to parse json file: {path}"))
}

fn find_bundle_root(start: &Utf8Path) -> Result<Utf8PathBuf> {
    if start.join("bundle.json").exists() {
        return Ok(start.to_path_buf());
    }

    let mut stack = vec![start.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir)
            .with_context(|| format!("failed to inspect directory: {dir}"))?
        {
            let entry = entry?;
            let path = entry.path();
            if entry.file_type()?.is_dir() {
                let utf8_dir = utf8_path_buf(path)?;
                if utf8_dir.join("bundle.json").exists() {
                    return Ok(utf8_dir);
                }
                stack.push(utf8_dir);
            }
        }
    }

    bail!("bundle.json not found under source: {start}");
}

fn extract_zip_archive(source: &Utf8Path, destination: &Path) -> Result<()> {
    let file =
        File::open(source).with_context(|| format!("failed to open bundle archive: {source}"))?;
    let mut archive =
        ZipArchive::new(file).with_context(|| format!("failed to read zip archive: {source}"))?;

    for index in 0..archive.len() {
        let mut entry = archive.by_index(index)?;
        let Some(relative) = entry.enclosed_name().map(PathBuf::from) else {
            continue;
        };
        let output_path = destination.join(relative);

        if entry.is_dir() {
            std::fs::create_dir_all(&output_path)?;
            continue;
        }

        if let Some(parent) = output_path.parent() {
            std::fs::create_dir_all(parent)?;
        }

        let mut output_file = File::create(&output_path)?;
        io::copy(&mut entry, &mut output_file)?;
        output_file.flush()?;
    }

    Ok(())
}

fn resolve_asset<F>(
    asset: &AssetSpec,
    bundle_root: &Utf8Path,
    assets_dir: Option<&str>,
    cache_root: &Utf8Path,
    progress: &mut F,
) -> Result<ImportedMaterial>
where
    F: FnMut(ImportBundleProgress),
{
    let resolved_path = resolve_asset_source(
        &asset.id,
        &asset.source,
        bundle_root,
        assets_dir,
        cache_root,
        asset.kind,
        progress,
    )?;

    match asset.kind {
        AssetKind::Video | AssetKind::Image => Ok(ImportedMaterial::Video(
            create_video_material(&resolved_path, asset.name.as_deref()).with_context(|| {
                format!(
                    "failed to create video/image material for asset '{}'",
                    asset.id
                )
            })?,
        )),
        AssetKind::Audio => Ok(ImportedMaterial::Audio(
            create_audio_material(&resolved_path, asset.name.as_deref()).with_context(|| {
                format!("failed to create audio material for asset '{}'", asset.id)
            })?,
        )),
    }
}

fn validate_simple_timeline(timeline: &SimpleTimelineManifest) -> Result<()> {
    if timeline.videos.is_empty() {
        bail!("simple_timeline_package requires at least one video");
    }
    validate_simple_subtitle_style(&timeline.subtitle_style)?;

    for (index, cue) in timeline.subtitles.iter().enumerate() {
        let start = seconds_to_micros(cue.start, &format!("subtitles[{index}].start"))?;
        let end = seconds_to_micros(cue.end, &format!("subtitles[{index}].end"))?;
        if end <= start {
            bail!("subtitles[{index}] end must be greater than start");
        }
    }

    Ok(())
}

fn validate_simple_subtitle_style(style: &SimpleSubtitleStyle) -> Result<()> {
    if !style.font_size.is_finite() || style.font_size <= 0.0 {
        bail!("subtitle_style.font_size must be greater than 0");
    }
    if !is_normalized_position(style.x) {
        bail!("subtitle_style.x must be between 0.0 and 1.0");
    }
    if !is_normalized_position(style.y) {
        bail!("subtitle_style.y must be between 0.0 and 1.0");
    }
    Ok(())
}

fn validate_pipeline_audio_style(style: &PipelineAudioStyle) -> Result<()> {
    if !style.video_volume.is_finite() || style.video_volume < 0.0 {
        bail!("audio_style.video_volume must be non-negative");
    }
    if style.audio_volume.is_some() {
        bail!("audio_style.audio_volume is no longer supported; use audio_style.narration_volume");
    }
    if !style.narration_volume.is_finite() || style.narration_volume < 0.0 {
        bail!("audio_style.narration_volume must be non-negative");
    }
    Ok(())
}

fn validate_pipeline_spec(pipeline: &PipelineSpec) -> Result<()> {
    if pipeline.audio_file.is_some() {
        bail!("pipeline.audio_file is no longer supported; use pipeline.narration_files");
    }
    if pipeline.narration_files.is_empty() {
        bail!("pipeline.narration_files must contain at least one narration file");
    }
    Ok(())
}

fn validate_pipeline_narration_count(
    narration_files: &[String],
    subtitles: &[SimpleSubtitleCue],
) -> Result<()> {
    if narration_files.len() != subtitles.len() {
        bail!(
            "pipeline.narration_files length ({}) must match subtitle cue count ({})",
            narration_files.len(),
            subtitles.len()
        );
    }
    Ok(())
}

fn is_normalized_position(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn build_simple_subtitle_style(style: &SimpleSubtitleStyle) -> Result<TextStyle> {
    validate_simple_subtitle_style(style)?;
    Ok(TextStyle {
        size: style.font_size,
        align: TextAlign::Center,
        auto_wrapping: true,
        ..Default::default()
    })
}

fn build_simple_subtitle_transform(style: &SimpleSubtitleStyle) -> Result<Transform> {
    validate_simple_subtitle_style(style)?;
    Ok(Transform {
        x: style.x,
        y: style.y,
        ..Default::default()
    })
}

fn seconds_to_micros(value: f64, label: &str) -> Result<u64> {
    if !value.is_finite() || value < 0.0 {
        bail!("{label} must be non-negative seconds");
    }
    let micros = value * SEC as f64;
    if micros > u64::MAX as f64 {
        bail!("{label} is too large");
    }
    Ok(micros.round() as u64)
}

fn resolve_simple_asset_path(
    relative: &str,
    bundle_root: &Utf8Path,
    assets_dir: Option<&str>,
) -> Result<Utf8PathBuf> {
    validate_simple_relative_path("asset path", relative)?;
    let assets_dir = assets_dir.unwrap_or("assets");
    validate_simple_relative_path("assets_dir", assets_dir)?;

    let path = bundle_root.join(assets_dir).join(relative);
    if !path.is_file() {
        bail!("simple_timeline_package asset not found: {relative}");
    }
    Ok(path)
}

fn resolve_pipeline_asset_path(
    label: &str,
    relative: &str,
    bundle_root: &Utf8Path,
    assets_dir: Option<&str>,
) -> Result<Utf8PathBuf> {
    validate_simple_relative_path(label, relative)?;
    let assets_dir = assets_dir.unwrap_or("assets");
    validate_simple_relative_path("assets_dir", assets_dir)?;

    let path = bundle_root.join(assets_dir).join(relative);
    if !path.is_file() {
        bail!("{label} not found: {relative}");
    }
    Ok(path)
}

fn validate_simple_relative_path(label: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{label} must not be empty");
    }
    if value.contains('\\') {
        bail!("{label} must use '/' as path separator");
    }
    let path = Utf8Path::new(value);
    if path.is_absolute() {
        bail!("{label} must be relative");
    }
    if value
        .split('/')
        .any(|segment| segment.is_empty() || segment == "." || segment == "..")
    {
        bail!("{label} contains an invalid path segment");
    }
    Ok(())
}

fn parse_concat_file(path: &Utf8Path) -> Result<Vec<String>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read concat file: {path}"))?;
    let mut files = Vec::new();
    for (line_index, raw_line) in content.lines().enumerate() {
        let line_no = line_index + 1;
        let line = raw_line.trim().trim_start_matches('\u{feff}');
        if line.is_empty() {
            continue;
        }
        let Some(rest) = line.strip_prefix("file ") else {
            bail!("concat line {line_no} only supports file entries");
        };
        let relative = parse_concat_file_value(rest)
            .with_context(|| format!("invalid concat line {line_no}"))?;
        validate_simple_relative_path(&format!("concat line {line_no} path"), &relative)?;
        files.push(relative);
    }

    if files.is_empty() {
        bail!("concat file must contain at least one file entry");
    }
    Ok(files)
}

fn parse_concat_file_value(value: &str) -> Result<String> {
    let value = value.trim();
    if value.starts_with('\'') && value.ends_with('\'') && value.len() >= 2 {
        return Ok(value[1..value.len() - 1].to_string());
    }
    bail!("concat file entry must use single quotes");
}

fn parse_srt_file(path: &Utf8Path) -> Result<Vec<SimpleSubtitleCue>> {
    let content = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read subtitle file: {path}"))?;
    parse_srt_content(&content)
}

fn parse_srt_content(content: &str) -> Result<Vec<SimpleSubtitleCue>> {
    let normalized = content
        .trim_start_matches('\u{feff}')
        .replace("\r\n", "\n")
        .replace('\r', "\n");
    let mut cues = Vec::new();

    for (block_index, block) in normalized.split("\n\n").enumerate() {
        let lines = block
            .lines()
            .map(str::trim_end)
            .filter(|line| !line.trim().is_empty())
            .collect::<Vec<_>>();
        if lines.is_empty() {
            continue;
        }

        let time_line_index = if lines[0].contains("-->") { 0 } else { 1 };
        if time_line_index >= lines.len() {
            bail!("srt block {} is missing time range", block_index + 1);
        }
        let text_start_index = time_line_index + 1;
        if text_start_index >= lines.len() {
            bail!("srt block {} is missing text", block_index + 1);
        }

        let (start, end) = parse_srt_time_range(lines[time_line_index])
            .with_context(|| format!("invalid srt block {} time range", block_index + 1))?;
        if end <= start {
            bail!(
                "srt block {} end must be greater than start",
                block_index + 1
            );
        }

        let text = lines[text_start_index..].join("\n");
        cues.push(SimpleSubtitleCue { start, end, text });
    }

    if cues.is_empty() {
        bail!("srt file must contain at least one subtitle cue");
    }
    Ok(cues)
}

fn parse_srt_time_range(line: &str) -> Result<(f64, f64)> {
    let Some((start, end)) = line.split_once("-->") else {
        bail!("srt time range must contain -->");
    };
    Ok((parse_srt_time(start.trim())?, parse_srt_time(end.trim())?))
}

fn parse_srt_time(value: &str) -> Result<f64> {
    let Some((hour_text, rest)) = value.split_once(':') else {
        bail!("invalid srt time: {value}");
    };
    let Some((minute_text, rest)) = rest.split_once(':') else {
        bail!("invalid srt time: {value}");
    };
    let Some((second_text, millis_text)) = rest.split_once(',').or_else(|| rest.split_once('.'))
    else {
        bail!("invalid srt time: {value}");
    };

    let hours = hour_text
        .parse::<u64>()
        .with_context(|| format!("invalid srt hour: {value}"))?;
    let minutes = minute_text
        .parse::<u64>()
        .with_context(|| format!("invalid srt minute: {value}"))?;
    let seconds = second_text
        .parse::<u64>()
        .with_context(|| format!("invalid srt second: {value}"))?;
    let millis = millis_text
        .parse::<u64>()
        .with_context(|| format!("invalid srt millis: {value}"))?;
    if minutes >= 60 || seconds >= 60 || millis >= 1000 {
        bail!("invalid srt time component: {value}");
    }

    Ok((hours * 3600 + minutes * 60 + seconds) as f64 + millis as f64 / 1000.0)
}

fn resolve_draft_binding<F>(
    asset: &DraftAssetBinding,
    bundle_root: &Utf8Path,
    assets_dir: Option<&str>,
    cache_root: &Utf8Path,
    output_draft_dir: &Utf8Path,
    material_index: usize,
    progress: &mut F,
) -> Result<DraftMaterialReplacement>
where
    F: FnMut(ImportBundleProgress),
{
    let source = AssetSourceSpec {
        source_type: AssetSourceType::BundlePath,
        path: Some(asset.relative_path.clone()),
        url: None,
    };
    let resolved_path = resolve_asset_source(
        &asset.match_value,
        &source,
        bundle_root,
        assets_dir,
        cache_root,
        asset.kind,
        progress,
    )?;
    let localized_path =
        localize_draft_package_asset(&resolved_path, output_draft_dir, asset.kind, material_index)?;

    Ok(DraftMaterialReplacement {
        kind: asset.kind,
        name: asset.match_value.clone(),
        path: normalize_path_for_draft(&localized_path),
    })
}

#[derive(Debug, Clone)]
struct DraftMaterialReplacement {
    kind: AssetKind,
    name: String,
    path: String,
}

fn localize_draft_package_asset(
    source: &Utf8Path,
    output_draft_dir: &Utf8Path,
    kind: AssetKind,
    index: usize,
) -> Result<Utf8PathBuf> {
    if !source.exists() || !source.is_file() || source.starts_with(output_draft_dir) {
        return Ok(source.to_path_buf());
    }

    let (category, prefix) = match kind {
        AssetKind::Video | AssetKind::Image => ("video", "video"),
        AssetKind::Audio => ("audio", "audio"),
    };
    let file_name = source.file_name().unwrap_or("asset");
    let destination = output_draft_dir
        .join("_assets")
        .join(category)
        .join(format!("{prefix}_{index:04}_{file_name}"));

    if let Some(parent) = destination.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::copy(source, &destination).with_context(|| {
        format!("failed to copy draft package asset: {source} -> {destination}")
    })?;

    Ok(destination)
}

fn rewrite_draft_package_snapshots(
    draft_dir: &Utf8Path,
    replacements: &[DraftMaterialReplacement],
) -> Result<()> {
    let mut snapshot_files = Vec::new();
    collect_draft_snapshot_files(draft_dir, &mut snapshot_files)?;

    for snapshot_file in snapshot_files {
        rewrite_draft_snapshot(&snapshot_file, replacements)
            .with_context(|| format!("failed to rewrite draft snapshot: {snapshot_file}"))?;
    }

    Ok(())
}

fn collect_draft_snapshot_files(
    current_dir: &Utf8Path,
    snapshot_files: &mut Vec<Utf8PathBuf>,
) -> Result<()> {
    for entry in std::fs::read_dir(current_dir)? {
        let entry = entry?;
        let path = utf8_path_buf(entry.path())?;
        if entry.file_type()?.is_dir() {
            collect_draft_snapshot_files(&path, snapshot_files)?;
            continue;
        }

        if is_draft_snapshot_file(&path) {
            snapshot_files.push(path);
        }
    }

    Ok(())
}

fn is_draft_snapshot_file(path: &Utf8Path) -> bool {
    matches!(
        path.file_name(),
        Some("draft_content.json" | "draft_info.json" | "draft_info.json.bak" | "template-2.tmp")
    )
}

fn rewrite_draft_snapshot(
    snapshot_file: &Utf8Path,
    replacements: &[DraftMaterialReplacement],
) -> Result<()> {
    let content = std::fs::read_to_string(snapshot_file)?;
    let mut draft: Value = match serde_json::from_str(&content) {
        Ok(draft) => draft,
        Err(_) => return Ok(()),
    };

    let mut changed = false;
    for replacement in replacements {
        changed |= rewrite_material_path_by_name(&mut draft, replacement);
    }

    if changed {
        std::fs::write(snapshot_file, serde_json::to_string_pretty(&draft)?)?;
    }

    Ok(())
}

fn rewrite_material_path_by_name(
    draft: &mut Value,
    replacement: &DraftMaterialReplacement,
) -> bool {
    let Some(materials) = draft.get_mut("materials").and_then(Value::as_object_mut) else {
        return false;
    };

    let (list_key, name_key) = match replacement.kind {
        AssetKind::Video | AssetKind::Image => ("videos", "material_name"),
        AssetKind::Audio => ("audios", "name"),
    };

    let Some(items) = materials.get_mut(list_key).and_then(Value::as_array_mut) else {
        return false;
    };

    let mut changed = false;
    for item in items {
        if item.get(name_key).and_then(Value::as_str) == Some(replacement.name.as_str()) {
            item["path"] = json!(replacement.path);
            changed = true;
        }
    }

    changed
}

fn resolve_asset_source<F>(
    asset_id: &str,
    source: &AssetSourceSpec,
    bundle_root: &Utf8Path,
    assets_dir: Option<&str>,
    cache_root: &Utf8Path,
    asset_kind: AssetKind,
    progress: &mut F,
) -> Result<Utf8PathBuf>
where
    F: FnMut(ImportBundleProgress),
{
    match source.source_type {
        AssetSourceType::BundlePath => {
            let relative = source
                .path
                .as_deref()
                .ok_or_else(|| anyhow!("asset '{}' is missing source.path", asset_id))?;
            Ok(resolve_bundle_relative_path(
                relative,
                bundle_root,
                assets_dir,
            ))
        }
        AssetSourceType::LocalPath => {
            let raw_path = source
                .path
                .as_deref()
                .ok_or_else(|| anyhow!("asset '{}' is missing source.path", asset_id))?;
            let path = Utf8PathBuf::from(raw_path);
            Ok(if path.is_absolute() {
                path
            } else {
                bundle_root.join(path)
            })
        }
        AssetSourceType::Url => {
            let url = source
                .url
                .as_deref()
                .ok_or_else(|| anyhow!("asset '{}' is missing source.url", asset_id))?;
            download_asset(url, asset_id, cache_root, asset_kind, progress)
        }
    }
}

fn resolve_bundle_relative_path(
    relative: &str,
    bundle_root: &Utf8Path,
    assets_dir: Option<&str>,
) -> Utf8PathBuf {
    let base = assets_dir
        .map(|dir| bundle_root.join(dir))
        .unwrap_or_else(|| bundle_root.to_path_buf());
    let preferred = base.join(relative);
    let fallback = bundle_root.join(relative);
    if preferred.exists() {
        preferred
    } else if fallback.exists() {
        fallback
    } else {
        preferred
    }
}

fn download_asset<F>(
    url: &str,
    asset_id: &str,
    cache_root: &Utf8Path,
    asset_kind: AssetKind,
    progress: &mut F,
) -> Result<Utf8PathBuf>
where
    F: FnMut(ImportBundleProgress),
{
    progress(ImportBundleProgress {
        stage: "download_asset".to_string(),
        message: format!("Downloading asset {} from {}", asset_id, url),
        data: json!({
            "asset_id": asset_id,
            "kind": asset_kind.as_str(),
            "url": url,
        }),
    });

    let downloads_dir = cache_root.join("downloads");
    std::fs::create_dir_all(&downloads_dir)?;

    let file_name = download_file_name(url, asset_id);
    let destination = downloads_dir.join(file_name);

    let mut response = reqwest::blocking::get(url)
        .with_context(|| format!("failed to request asset url: {url}"))?
        .error_for_status()
        .with_context(|| format!("asset download returned non-success status: {url}"))?;
    let mut file = File::create(&destination)
        .with_context(|| format!("failed to create downloaded asset file: {destination}"))?;
    io::copy(&mut response, &mut file)
        .with_context(|| format!("failed to save downloaded asset: {destination}"))?;
    file.flush()?;

    Ok(destination)
}

fn copy_dir_all(source: &Utf8Path, target: &Utf8Path) -> Result<()> {
    std::fs::create_dir_all(target)?;
    for entry in std::fs::read_dir(source)? {
        let entry = entry?;
        let source_path = utf8_path_buf(entry.path())?;
        let target_path = target.join(entry.file_name().to_string_lossy().as_ref());
        if entry.file_type()?.is_dir() {
            copy_dir_all(&source_path, &target_path)?;
        } else {
            if let Some(parent) = target_path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            std::fs::copy(&source_path, &target_path)?;
        }
    }
    Ok(())
}

fn rewrite_meta_info(draft_dir: &Utf8Path, final_name: &str) -> Result<()> {
    let meta_path = draft_dir.join("draft_meta_info.json");
    if !meta_path.exists() {
        return Ok(());
    }

    let mut meta: Value = serde_json::from_str(&std::fs::read_to_string(&meta_path)?)?;
    let output_dir_str = normalize_path_for_draft(draft_dir);
    meta["draft_name"] = json!(final_name);
    meta["draft_root_path"] = json!(output_dir_str);
    meta["draft_fold_path"] = json!(normalize_path_for_draft(draft_dir));
    meta["tm_draft_modified"] = json!(chrono::Utc::now().timestamp());
    meta["tm_draft_cloud_modified"] = json!(0);
    meta["draft_id"] = json!(format!(
        "{{{}}}",
        Uuid::new_v4().as_hyphenated().to_string().to_uppercase()
    ));
    std::fs::write(meta_path, serde_json::to_string_pretty(&meta)?)?;
    Ok(())
}

fn normalize_path_for_draft(path: &Utf8Path) -> String {
    if cfg!(windows) {
        path.as_str().replace('\\', "/")
    } else {
        path.as_str().to_string()
    }
}

fn download_file_name(url: &str, asset_id: &str) -> String {
    Url::parse(url)
        .ok()
        .and_then(|parsed| {
            parsed
                .path_segments()
                .and_then(|mut segments| segments.next_back().map(ToString::to_string))
        })
        .filter(|name| !name.is_empty())
        .map(|name| sanitize_file_name(&name))
        .unwrap_or_else(|| format!("{asset_id}.bin"))
}

fn sanitize_file_name(name: &str) -> String {
    let sanitized = name
        .chars()
        .map(|ch| match ch {
            'a'..='z' | 'A'..='Z' | '0'..='9' | '.' | '_' | '-' => ch,
            _ => '_',
        })
        .collect::<String>();
    if sanitized.is_empty() {
        "asset.bin".to_string()
    } else {
        sanitized
    }
}

fn build_clip(clip: &ClipSpec, assets: &HashMap<String, ImportedMaterial>) -> Result<Clip> {
    let target = build_time_range(&clip.timeline_in, &clip.timeline_out)
        .context("invalid clip target timerange")?;
    let source = match (&clip.source_in, &clip.source_out) {
        (Some(start), Some(end)) => {
            Some(build_time_range(start, end).context("invalid clip source timerange")?)
        }
        (None, None) => None,
        _ => bail!("source_in and source_out must be provided together"),
    };

    let mut built = match clip.clip_type {
        ImportClipType::Video => {
            let material = lookup_video_material(clip.asset_id.as_deref(), assets)?;
            make_video_clip(
                material,
                target,
                source,
                clip.speed,
                clip.volume.unwrap_or(1.0),
                clip.transform.clone(),
            )?
        }
        ImportClipType::Image => {
            let material = lookup_video_material(clip.asset_id.as_deref(), assets)?;
            make_image_clip(material, target, clip.transform.clone())
        }
        ImportClipType::Audio => {
            let material = lookup_audio_material(clip.asset_id.as_deref(), assets)?;
            make_audio_clip(
                material,
                target,
                source,
                clip.speed,
                clip.volume.unwrap_or(1.0),
            )?
        }
        ImportClipType::Text => {
            let text = clip
                .text
                .as_deref()
                .ok_or_else(|| anyhow!("text clip is missing text content"))?;
            make_text_clip(text, target, clip.style.clone(), clip.transform.clone())
        }
    };

    if let Some(id) = &clip.id {
        assign_clip_id(&mut built, id);
    }

    if let Clip::Text(text_clip) = &mut built {
        text_clip.border = clip.border.clone();
        text_clip.background = clip.background.clone();
        text_clip.shadow = clip.shadow.clone();
    }

    Ok(built)
}

fn build_time_range(start: &TimeValue, end: &TimeValue) -> Result<TimeRange> {
    let start_us = start.as_micros()?;
    let end_us = end.as_micros()?;
    if end_us <= start_us {
        bail!("time range end must be greater than start");
    }
    Ok(TimeRange::new(start_us, end_us - start_us))
}

fn lookup_video_material<'a>(
    asset_id: Option<&str>,
    assets: &'a HashMap<String, ImportedMaterial>,
) -> Result<&'a jy_schema::VideoMaterialRef> {
    let asset_id = asset_id.ok_or_else(|| anyhow!("clip is missing asset_id"))?;
    match assets.get(asset_id) {
        Some(ImportedMaterial::Video(material)) => Ok(material),
        Some(ImportedMaterial::Audio(_)) => bail!("asset '{asset_id}' is audio, not video/image"),
        None => bail!("asset '{asset_id}' not found"),
    }
}

fn lookup_audio_material<'a>(
    asset_id: Option<&str>,
    assets: &'a HashMap<String, ImportedMaterial>,
) -> Result<&'a jy_schema::AudioMaterialRef> {
    let asset_id = asset_id.ok_or_else(|| anyhow!("clip is missing asset_id"))?;
    match assets.get(asset_id) {
        Some(ImportedMaterial::Audio(material)) => Ok(material),
        Some(ImportedMaterial::Video(_)) => bail!("asset '{asset_id}' is video/image, not audio"),
        None => bail!("asset '{asset_id}' not found"),
    }
}

fn assign_clip_id(clip: &mut Clip, id: &str) {
    match clip {
        Clip::Video(video) => video.id = id.to_string(),
        Clip::Audio(audio) => audio.id = id.to_string(),
        Clip::Text(text) => text.id = id.to_string(),
        Clip::Image(image) => image.id = id.to_string(),
    }
}

fn track_render_index_offset(kind: ImportTrackKind, render_index: Option<i32>) -> i32 {
    let default_render = TrackKind::from(kind).default_render_index();
    render_index.unwrap_or(default_render) - default_render
}

fn ensure_output_dir_ready(output: &Utf8Path) -> Result<()> {
    if output.exists() {
        let mut entries = std::fs::read_dir(output)?;
        if entries.next().transpose()?.is_some() {
            bail!("output directory is not empty: {output}");
        }
    } else {
        std::fs::create_dir_all(output)?;
    }
    Ok(())
}

fn utf8_path_buf(path: PathBuf) -> Result<Utf8PathBuf> {
    Utf8PathBuf::from_path_buf(path).map_err(|path| anyhow!("non-utf8 path: {}", path.display()))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use std::process::Command;

    use jy_schema::{Canvas, TimeRange, TrackKind};
    use tempfile::tempdir;
    use zip::write::FileOptions;

    #[test]
    fn import_bundle_from_directory_generates_draft() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("bundle")).unwrap();
        fs::create_dir_all(bundle_dir.join("assets").join("image"))?;
        write_test_png(&bundle_dir.join("assets").join("image").join("poster.png"))?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "project_id": "proj_dir",
                "project_name": "Directory Bundle",
                "timeline_file": "timeline.json",
                "assets_dir": "assets"
            }))?,
        )?;
        fs::write(
            bundle_dir.join("timeline.json"),
            serde_json::to_string_pretty(&json!({
                "project": { "id": "proj_dir", "name": "Directory Bundle" },
                "canvas": { "width": 1280, "height": 720, "fps": 30 },
                "assets": [
                    {
                        "id": "poster",
                        "kind": "image",
                        "source": { "type": "bundle_path", "path": "image/poster.png" }
                    }
                ],
                "tracks": [
                    {
                        "kind": "video",
                        "name": "visual",
                        "clips": [
                            {
                                "id": "clip_image",
                                "type": "image",
                                "asset_id": "poster",
                                "timeline_in": 0,
                                "timeline_out": 2000000
                            }
                        ]
                    },
                    {
                        "kind": "text",
                        "name": "caption",
                        "clips": [
                            {
                                "type": "text",
                                "timeline_in": "0s",
                                "timeline_out": "2s",
                                "text": "bundle import works"
                            }
                        ]
                    }
                ]
            }))?,
        )?;

        let summary = import_bundle(&ImportBundleOptions {
            source: bundle_dir.clone(),
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft_dir")).unwrap(),
            name_override: Some("Imported Dir Draft".to_string()),
        })?;

        let output_dir = Utf8PathBuf::from(summary.draft_dir.as_str());
        assert!(output_dir.join("draft_content.json").exists());
        assert!(output_dir.join("draft_info.json").exists());
        assert!(output_dir.join("_assets").join("video").exists());

        let content = fs::read_to_string(output_dir.join("draft_content.json"))?;
        let meta = fs::read_to_string(output_dir.join("draft_meta_info.json"))?;
        assert!(meta.contains("Imported Dir Draft"));
        assert!(content.contains("bundle import works"));

        Ok(())
    }

    #[test]
    fn import_bundle_from_zip_generates_draft() -> Result<()> {
        let temp = tempdir()?;
        let zip_path = temp.path().join("bundle.zip");
        let png_bytes = test_png_bytes();
        let bundle_json = serde_json::to_vec_pretty(&json!({
            "bundle_version": 1,
            "project_id": "proj_zip",
            "project_name": "Zip Bundle",
            "timeline_file": "timeline.json",
            "assets_dir": "assets"
        }))?;
        let timeline_json = serde_json::to_vec_pretty(&json!({
            "project": { "id": "proj_zip", "name": "Zip Bundle" },
            "canvas": { "width": 1080, "height": 1920, "fps": 30 },
            "assets": [
                {
                    "id": "poster",
                    "kind": "image",
                    "source": { "type": "bundle_path", "path": "image/poster.png" }
                }
            ],
            "tracks": [
                {
                    "kind": "video",
                    "name": "visual",
                    "clips": [
                        {
                            "type": "image",
                            "asset_id": "poster",
                            "timeline_in": 0,
                            "timeline_out": 1500000
                        }
                    ]
                }
            ]
        }))?;

        {
            let file = File::create(&zip_path)?;
            let mut writer = zip::ZipWriter::new(file);
            let options = FileOptions::default();
            writer.add_directory("sample_bundle/", options)?;
            writer.start_file("sample_bundle/bundle.json", options)?;
            writer.write_all(&bundle_json)?;
            writer.start_file("sample_bundle/timeline.json", options)?;
            writer.write_all(&timeline_json)?;
            writer.add_directory("sample_bundle/assets/image/", options)?;
            writer.start_file("sample_bundle/assets/image/poster.png", options)?;
            writer.write_all(&png_bytes)?;
            writer.finish()?;
        }

        let summary = import_bundle(&ImportBundleOptions {
            source: Utf8PathBuf::from_path_buf(zip_path).unwrap(),
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft_zip")).unwrap(),
            name_override: None,
        })?;
        let output_dir = Utf8PathBuf::from(summary.draft_dir.as_str());

        assert!(output_dir.join("draft_content.json").exists());
        let meta = fs::read_to_string(output_dir.join("draft_meta_info.json"))?;
        assert!(meta.contains("Zip Bundle"));

        Ok(())
    }

    #[test]
    fn inspect_bundle_reports_project_metadata() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("bundle")).unwrap();
        fs::create_dir_all(&bundle_dir)?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "project_id": "proj_inspect",
                "project_name": "Inspect Bundle",
                "timeline_file": "timeline.json"
            }))?,
        )?;
        fs::write(
            bundle_dir.join("timeline.json"),
            serde_json::to_string_pretty(&json!({
                "project": { "id": "proj_inspect", "name": "Inspect Bundle" },
                "assets": [],
                "tracks": []
            }))?,
        )?;

        let inspection = inspect_bundle_source(&bundle_dir)?;
        assert_eq!(inspection.project_name.as_deref(), Some("Inspect Bundle"));
        assert_eq!(inspection.project_id.as_deref(), Some("proj_inspect"));
        assert_eq!(inspection.asset_count, 0);
        assert_eq!(inspection.track_count, 0);

        Ok(())
    }

    #[test]
    fn import_simple_timeline_package_generates_draft() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("simple_bundle")).unwrap();
        let assets_dir = bundle_dir.join("assets");
        fs::create_dir_all(&assets_dir)?;

        let first_video = assets_dir.join("video_001.mp4");
        let second_video = assets_dir.join("video_002.mp4");
        if !write_test_mp4(&first_video)? || !write_test_mp4(&second_video)? {
            return Ok(());
        }

        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "simple_timeline_package",
                "project_id": "proj_simple",
                "project_name": "Simple Timeline Bundle",
                "timeline_file": "timeline.json",
                "assets_dir": "assets"
            }))?,
        )?;
        fs::write(
            bundle_dir.join("timeline.json"),
            serde_json::to_string_pretty(&json!({
                "canvas": { "width": 1920, "height": 1080, "fps": 30 },
                "videos": [
                    { "path": "video_001.mp4" },
                    { "path": "video_002.mp4" }
                ],
                "subtitle_style": { "font_size": 9.0, "x": 0.5, "y": 0.82 },
                "subtitles": [
                    { "start": 0.0, "end": 0.4, "text": "第一句字幕" },
                    { "start": 1.0, "end": 1.4, "text": "第二句字幕" }
                ]
            }))?,
        )?;

        let summary = import_bundle(&ImportBundleOptions {
            source: bundle_dir,
            output: Utf8PathBuf::from_path_buf(temp.path().join("simple_draft")).unwrap(),
            name_override: None,
        })?;

        assert_eq!(summary.bundle_type, "simple_timeline_package");
        assert_eq!(summary.asset_count, 2);
        assert_eq!(summary.track_count, 2);
        assert_eq!(summary.video_material_count, 2);
        assert_eq!(summary.audio_material_count, 0);

        let output_dir = Utf8PathBuf::from(summary.draft_dir.as_str());
        assert!(output_dir.join("draft_content.json").exists());
        assert!(output_dir.join("_assets").join("video").exists());

        let content = fs::read_to_string(output_dir.join("draft_content.json"))?;
        assert!(content.contains("main_video"));
        assert!(content.contains("subtitle"));
        assert!(content.contains("第一句字幕"));
        assert!(content.contains("第二句字幕"));
        let draft: Value = serde_json::from_str(&content)?;
        let text_content = draft["materials"]["texts"][0]["content"]
            .as_str()
            .unwrap_or_default();
        let text_material: Value = serde_json::from_str(text_content)?;
        assert_eq!(text_material["styles"][0]["size"].as_f64(), Some(9.0));
        assert_eq!(
            draft["materials"]["texts"][0]["type"].as_str(),
            Some("subtitle")
        );
        let subtitle_segment = draft["tracks"]
            .as_array()
            .and_then(|tracks| {
                tracks.iter().find_map(|track| {
                    if track["name"].as_str() == Some("subtitle") {
                        track["segments"][0]["clip"]["transform"]["y"].as_f64()
                    } else {
                        None
                    }
                })
            })
            .unwrap_or_default();
        assert!((subtitle_segment - 0.64).abs() < 0.000001);

        Ok(())
    }

    #[test]
    fn simple_timeline_package_rejects_empty_videos() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("simple_bundle")).unwrap();
        fs::create_dir_all(&bundle_dir)?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "simple_timeline_package",
                "project_name": "Invalid Simple Bundle",
                "timeline_file": "timeline.json",
                "assets_dir": "assets"
            }))?,
        )?;
        fs::write(
            bundle_dir.join("timeline.json"),
            serde_json::to_string_pretty(&json!({
                "videos": [],
                "subtitles": []
            }))?,
        )?;

        let error = import_bundle(&ImportBundleOptions {
            source: bundle_dir,
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft")).unwrap(),
            name_override: None,
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("requires at least one video"));

        Ok(())
    }

    #[test]
    fn simple_timeline_package_rejects_missing_asset() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("simple_bundle")).unwrap();
        fs::create_dir_all(bundle_dir.join("assets"))?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "simple_timeline_package",
                "project_name": "Invalid Simple Bundle",
                "timeline_file": "timeline.json",
                "assets_dir": "assets"
            }))?,
        )?;
        fs::write(
            bundle_dir.join("timeline.json"),
            serde_json::to_string_pretty(&json!({
                "videos": [
                    { "path": "missing.mp4" }
                ],
                "subtitles": []
            }))?,
        )?;

        let error = import_bundle(&ImportBundleOptions {
            source: bundle_dir,
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft")).unwrap(),
            name_override: None,
        })
        .unwrap_err();
        let error_text = format!("{error:#}");
        assert!(error_text.contains("invalid video[0] path"));
        assert!(error_text.contains("asset not found"));

        Ok(())
    }

    #[test]
    fn simple_timeline_package_rejects_invalid_subtitle_time() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("simple_bundle")).unwrap();
        fs::create_dir_all(bundle_dir.join("assets"))?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "simple_timeline_package",
                "project_name": "Invalid Simple Bundle",
                "timeline_file": "timeline.json",
                "assets_dir": "assets"
            }))?,
        )?;
        fs::write(
            bundle_dir.join("timeline.json"),
            serde_json::to_string_pretty(&json!({
                "videos": [
                    { "path": "missing.mp4" }
                ],
                "subtitles": [
                    { "start": 1.0, "end": 1.0, "text": "bad" }
                ]
            }))?,
        )?;

        let error = import_bundle(&ImportBundleOptions {
            source: bundle_dir,
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft")).unwrap(),
            name_override: None,
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("subtitles[0] end must be greater than start"));

        Ok(())
    }

    #[test]
    fn simple_timeline_subtitle_style_uses_field_defaults() -> Result<()> {
        let timeline: SimpleTimelineManifest = serde_json::from_value(json!({
            "videos": [
                { "path": "video_001.mp4" }
            ],
            "subtitle_style": {
                "font_size": 6.5
            },
            "subtitles": []
        }))?;

        assert_eq!(timeline.subtitle_style.font_size, 6.5);
        assert_eq!(timeline.subtitle_style.x, 0.5);
        assert_eq!(timeline.subtitle_style.y, 0.82);

        Ok(())
    }

    #[test]
    fn import_pipeline_package_generates_draft() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("pipeline_bundle")).unwrap();
        let assets_dir = bundle_dir.join("assets");
        fs::create_dir_all(&assets_dir)?;

        let first_video = assets_dir.join("video_001.mp4");
        let second_video = assets_dir.join("video_002.mp4");
        let narration_dir = assets_dir.join("narration");
        fs::create_dir_all(&narration_dir)?;
        let first_narration = narration_dir.join("001.wav");
        let second_narration = narration_dir.join("002.wav");
        if !write_test_mp4(&first_video)?
            || !write_test_mp4(&second_video)?
            || !write_test_wav(&first_narration, 1)?
            || !write_test_wav(&second_narration, 1)?
        {
            return Ok(());
        }

        fs::write(
            assets_dir.join("concat.txt"),
            "file 'video_001.mp4'\nfile 'video_002.mp4'\n",
        )?;
        fs::write(
            assets_dir.join("subtitle.srt"),
            "1\n00:00:00,000 --> 00:00:00,400\n第一句字幕\n\n2\n00:00:00,500 --> 00:00:02,000\n第二句字幕\n",
        )?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "pipeline_package",
                "project_id": "proj_pipeline",
                "project_name": "Pipeline Bundle",
                "assets_dir": "assets",
                "pipeline": {
                    "concat_file": "concat.txt",
                    "subtitle_file": "subtitle.srt",
                    "narration_files": [
                        "narration/001.wav",
                        "narration/002.wav"
                    ]
                },
                "subtitle_style": { "font_size": 9.0, "x": 0.5, "y": 0.82 },
                "audio_style": { "video_volume": 1.0, "narration_volume": 0.8 }
            }))?,
        )?;

        let summary = import_bundle(&ImportBundleOptions {
            source: bundle_dir,
            output: Utf8PathBuf::from_path_buf(temp.path().join("pipeline_draft")).unwrap(),
            name_override: None,
        })?;

        assert_eq!(summary.bundle_type, "pipeline_package");
        assert_eq!(summary.asset_count, 4);
        assert_eq!(summary.track_count, 3);
        assert_eq!(summary.video_material_count, 2);
        assert_eq!(summary.audio_material_count, 2);

        let draft: Value = serde_json::from_str(&fs::read_to_string(
            Utf8PathBuf::from(summary.draft_dir.as_str()).join("draft_content.json"),
        )?)?;
        assert_eq!(draft["materials"]["audios"].as_array().unwrap().len(), 2);
        assert_eq!(draft["materials"]["texts"].as_array().unwrap().len(), 2);

        let tracks = draft["tracks"].as_array().unwrap();
        assert!(tracks
            .iter()
            .any(|track| track["name"].as_str() == Some("main_video")));
        let audio_track = tracks
            .iter()
            .find(|track| track["name"].as_str() == Some("audio"))
            .expect("audio track exists");
        let audio_segments = audio_track["segments"].as_array().unwrap();
        assert_eq!(audio_segments.len(), 2);
        assert_eq!(audio_segments[0]["volume"].as_f64(), Some(0.8));
        assert_eq!(
            audio_segments[0]["target_timerange"]["start"].as_u64(),
            Some(0)
        );
        assert_eq!(
            audio_segments[0]["target_timerange"]["duration"].as_u64(),
            Some(400_000)
        );
        assert_eq!(
            audio_segments[1]["target_timerange"]["start"].as_u64(),
            Some(500_000)
        );
        assert_eq!(
            audio_segments[1]["target_timerange"]["duration"].as_u64(),
            Some(1_000_000)
        );
        assert!(tracks
            .iter()
            .any(|track| track["name"].as_str() == Some("subtitle")));

        Ok(())
    }

    #[test]
    fn parse_concat_file_rejects_unsupported_lines() -> Result<()> {
        let temp = tempdir()?;
        let concat = Utf8PathBuf::from_path_buf(temp.path().join("concat.txt")).unwrap();

        fs::write(&concat, "")?;
        assert!(format!("{:#}", parse_concat_file(&concat).unwrap_err())
            .contains("at least one file entry"));

        fs::write(&concat, "duration 1\n")?;
        assert!(format!("{:#}", parse_concat_file(&concat).unwrap_err())
            .contains("only supports file entries"));

        fs::write(&concat, "file 'C:/assets/video.mp4'\n")?;
        assert!(
            format!("{:#}", parse_concat_file(&concat).unwrap_err()).contains("must be relative")
        );

        fs::write(&concat, "file '../video.mp4'\n")?;
        assert!(format!("{:#}", parse_concat_file(&concat).unwrap_err())
            .contains("invalid path segment"));

        fs::write(&concat, "file 'dir\\video.mp4'\n")?;
        assert!(format!("{:#}", parse_concat_file(&concat).unwrap_err()).contains("path separator"));

        Ok(())
    }

    #[test]
    fn parse_srt_content_validates_time_ranges() -> Result<()> {
        let cues = parse_srt_content("1\n00:00:00,000 --> 00:00:01,200\n第一行\n第二行\n")?;
        assert_eq!(cues.len(), 1);
        assert_eq!(cues[0].text, "第一行\n第二行");
        assert_eq!(cues[0].start, 0.0);
        assert_eq!(cues[0].end, 1.2);

        let invalid_time = parse_srt_content("1\nbad --> 00:00:01,000\nbad\n").unwrap_err();
        assert!(format!("{invalid_time:#}").contains("invalid srt"));

        let invalid_range =
            parse_srt_content("1\n00:00:01,000 --> 00:00:01,000\nbad\n").unwrap_err();
        assert!(format!("{invalid_range:#}").contains("end must be greater than start"));

        Ok(())
    }

    #[test]
    fn pipeline_package_rejects_subtitle_overflow() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("pipeline_bundle")).unwrap();
        let assets_dir = bundle_dir.join("assets");
        fs::create_dir_all(&assets_dir)?;

        let video = assets_dir.join("video_001.mp4");
        let narration = assets_dir.join("001.wav");
        if !write_test_mp4(&video)? || !write_test_wav(&narration, 1)? {
            return Ok(());
        }

        fs::write(assets_dir.join("concat.txt"), "file 'video_001.mp4'\n")?;
        fs::write(
            assets_dir.join("subtitle.srt"),
            "1\n00:00:00,000 --> 00:00:02,000\n超出视频\n",
        )?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "pipeline_package",
                "project_name": "Overflow Pipeline Bundle",
                "assets_dir": "assets",
                "pipeline": {
                    "concat_file": "concat.txt",
                    "subtitle_file": "subtitle.srt",
                    "narration_files": ["001.wav"]
                }
            }))?,
        )?;

        let error = import_bundle(&ImportBundleOptions {
            source: bundle_dir,
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft")).unwrap(),
            name_override: None,
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("end exceeds stitched video duration"));

        Ok(())
    }

    #[test]
    fn pipeline_package_rejects_non_audio_file() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("pipeline_bundle")).unwrap();
        let assets_dir = bundle_dir.join("assets");
        fs::create_dir_all(&assets_dir)?;

        let video = assets_dir.join("video_001.mp4");
        if !write_test_mp4(&video)? {
            return Ok(());
        }

        fs::write(assets_dir.join("concat.txt"), "file 'video_001.mp4'\n")?;
        fs::write(
            assets_dir.join("subtitle.srt"),
            "1\n00:00:00,000 --> 00:00:00,400\n字幕\n",
        )?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "pipeline_package",
                "project_name": "Invalid Audio Pipeline Bundle",
                "assets_dir": "assets",
                "pipeline": {
                    "concat_file": "concat.txt",
                    "subtitle_file": "subtitle.srt",
                    "narration_files": ["video_001.mp4"]
                }
            }))?,
        )?;

        let error = import_bundle(&ImportBundleOptions {
            source: bundle_dir,
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft")).unwrap(),
            name_override: None,
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("failed to load pipeline narration[0]"));

        Ok(())
    }

    #[test]
    fn pipeline_package_rejects_deprecated_audio_file() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("pipeline_bundle")).unwrap();
        fs::create_dir_all(&bundle_dir)?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "pipeline_package",
                "project_name": "Deprecated Audio File",
                "assets_dir": "assets",
                "pipeline": {
                    "concat_file": "concat.txt",
                    "audio_file": "audio.wav",
                    "subtitle_file": "subtitle.srt"
                }
            }))?,
        )?;

        let error = import_bundle(&ImportBundleOptions {
            source: bundle_dir,
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft")).unwrap(),
            name_override: None,
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("pipeline.audio_file is no longer supported"));

        Ok(())
    }

    #[test]
    fn pipeline_package_rejects_missing_narration_files() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("pipeline_bundle")).unwrap();
        fs::create_dir_all(&bundle_dir)?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "pipeline_package",
                "project_name": "Missing Narration Files",
                "assets_dir": "assets",
                "pipeline": {
                    "concat_file": "concat.txt",
                    "subtitle_file": "subtitle.srt"
                }
            }))?,
        )?;

        let error = import_bundle(&ImportBundleOptions {
            source: bundle_dir,
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft")).unwrap(),
            name_override: None,
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("narration_files must contain at least one"));

        Ok(())
    }

    #[test]
    fn pipeline_package_rejects_deprecated_audio_volume() -> Result<()> {
        let manifest: BundleManifest = serde_json::from_value(json!({
            "bundle_version": 1,
            "bundle_type": "pipeline_package",
            "project_name": "Deprecated Audio Volume",
            "pipeline": {
                "concat_file": "concat.txt",
                "subtitle_file": "subtitle.srt",
                "narration_files": ["001.wav"]
            },
            "audio_style": {
                "audio_volume": 0.8
            }
        }))?;

        let error = validate_pipeline_audio_style(&manifest.audio_style).unwrap_err();
        assert!(format!("{error:#}").contains("audio_style.audio_volume is no longer supported"));

        Ok(())
    }

    #[test]
    fn pipeline_package_rejects_narration_count_mismatch() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("pipeline_bundle")).unwrap();
        let assets_dir = bundle_dir.join("assets");
        fs::create_dir_all(&assets_dir)?;

        let video = assets_dir.join("video_001.mp4");
        let narration = assets_dir.join("001.wav");
        if !write_test_mp4(&video)? || !write_test_wav(&narration, 1)? {
            return Ok(());
        }

        fs::write(assets_dir.join("concat.txt"), "file 'video_001.mp4'\n")?;
        fs::write(
            assets_dir.join("subtitle.srt"),
            "1\n00:00:00,000 --> 00:00:00,400\n第一句\n\n2\n00:00:00,500 --> 00:00:00,900\n第二句\n",
        )?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "pipeline_package",
                "project_name": "Mismatch Pipeline Bundle",
                "assets_dir": "assets",
                "pipeline": {
                    "concat_file": "concat.txt",
                    "subtitle_file": "subtitle.srt",
                    "narration_files": ["001.wav"]
                }
            }))?,
        )?;

        let error = import_bundle(&ImportBundleOptions {
            source: bundle_dir,
            output: Utf8PathBuf::from_path_buf(temp.path().join("draft")).unwrap(),
            name_override: None,
        })
        .unwrap_err();
        assert!(format!("{error:#}").contains("must match subtitle cue count"));

        Ok(())
    }

    #[test]
    fn import_draft_package_rewrites_material_paths() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("bundle")).unwrap();
        let source_draft_dir = bundle_dir.join("draft");
        fs::create_dir_all(bundle_dir.join("assets"))?;
        fs::create_dir_all(&source_draft_dir)?;

        let original_asset = bundle_dir.join("original.png");
        let replacement_asset = bundle_dir.join("assets").join("replacement.png");
        write_test_png(&original_asset)?;
        write_test_png(&replacement_asset)?;

        let material = create_video_material(&original_asset, Some("poster.png"))?;
        let clip = make_image_clip(&material, TimeRange::new(0, 2 * SEC), None);
        let project = ProjectBuilder::new("Source Draft", Canvas::new(1280, 720, 30))
            .add_track(TrackKind::Video, "visual", 0)?
            .add_video_material(material)
            .add_clip_to_track("visual", clip)?
            .build();
        write_draft(&project, &source_draft_dir)?;

        let source_content = fs::read_to_string(source_draft_dir.join("draft_content.json"))?;
        fs::write(source_draft_dir.join("template-2.tmp"), &source_content)?;
        let timeline_dir = source_draft_dir.join("Timelines").join("{TIMELINE-1}");
        fs::create_dir_all(&timeline_dir)?;
        fs::write(timeline_dir.join("draft_info.json"), &source_content)?;
        fs::write(timeline_dir.join("template-2.tmp"), &source_content)?;

        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "draft_package",
                "project_id": "proj_draft",
                "project_name": "Draft Package",
                "draft_dir": "draft",
                "assets_dir": "assets",
                "match_key": "name",
                "assets": [
                    {
                        "kind": "image",
                        "match_value": "poster.png",
                        "relative_path": "replacement.png"
                    }
                ]
            }))?,
        )?;

        let summary = import_bundle(&ImportBundleOptions {
            source: bundle_dir.clone(),
            output: Utf8PathBuf::from_path_buf(temp.path().join("imported_draft")).unwrap(),
            name_override: Some("Imported Draft Package".to_string()),
        })?;

        assert_eq!(summary.bundle_type, "draft_package");
        let output_dir = Utf8PathBuf::from(summary.draft_dir.as_str());
        let content: Value =
            serde_json::from_str(&fs::read_to_string(output_dir.join("draft_content.json"))?)?;
        let replaced_path = content["materials"]["videos"][0]["path"]
            .as_str()
            .unwrap_or_default();
        assert!(replaced_path.contains("/_assets/video/video_0000_replacement.png"));
        assert!(!replaced_path.contains("/bundle/assets/replacement.png"));

        let info = fs::read_to_string(output_dir.join("draft_info.json"))?;
        assert!(info.contains("replacement.png"));
        assert!(info.contains("/_assets/video/"));
        let template = fs::read_to_string(output_dir.join("template-2.tmp"))?;
        assert!(template.contains("/_assets/video/video_0000_replacement.png"));
        let timeline_info =
            fs::read_to_string(output_dir.join("Timelines/{TIMELINE-1}/draft_info.json"))?;
        assert!(timeline_info.contains("/_assets/video/video_0000_replacement.png"));
        let timeline_template =
            fs::read_to_string(output_dir.join("Timelines/{TIMELINE-1}/template-2.tmp"))?;
        assert!(timeline_template.contains("/_assets/video/video_0000_replacement.png"));
        let meta = fs::read_to_string(output_dir.join("draft_meta_info.json"))?;
        assert!(meta.contains("Imported Draft Package"));

        Ok(())
    }

    fn write_test_png(path: &Utf8Path) -> Result<()> {
        fs::write(path, test_png_bytes())?;
        Ok(())
    }

    fn write_test_mp4(path: &Utf8Path) -> Result<bool> {
        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "color=c=black:s=16x16:d=1",
                "-pix_fmt",
                "yuv420p",
                path.as_str(),
            ])
            .status();

        match status {
            Ok(status) => Ok(status.success()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn write_test_wav(path: &Utf8Path, duration_seconds: u64) -> Result<bool> {
        let duration = duration_seconds.to_string();
        let status = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=1000:sample_rate=44100",
                "-t",
                duration.as_str(),
                "-c:a",
                "pcm_s16le",
                path.as_str(),
            ])
            .status();

        match status {
            Ok(status) => Ok(status.success()),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(error.into()),
        }
    }

    fn test_png_bytes() -> Vec<u8> {
        vec![
            0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A, 0x00, 0x00, 0x00, 0x0D, b'I', b'H',
            b'D', b'R', 0x00, 0x00, 0x00, 0x01, 0x00, 0x00, 0x00, 0x01, 0x08, 0x06, 0x00, 0x00,
            0x00, 0x1F, 0x15, 0xC4, 0x89, 0x00, 0x00, 0x00, 0x0D, b'I', b'D', b'A', b'T', 0x78,
            0x9C, 0x63, 0xF8, 0xCF, 0xC0, 0xF0, 0x1F, 0x00, 0x05, 0x00, 0x01, 0xFF, 0x89, 0x99,
            0x3D, 0x1D, 0x00, 0x00, 0x00, 0x00, b'I', b'E', b'N', b'D', 0xAE, 0x42, 0x60, 0x82,
        ]
    }
}
