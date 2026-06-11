use anyhow::{anyhow, bail, Context, Result};
use camino::Utf8Path;
use jy_draft::writer::write_draft;
use jy_schema::{Canvas, MaterialKind, TimeRange, TrackKind};
use jy_timeline::builder::ProjectBuilder;
use jy_timeline::clip::{make_audio_clip, make_text_clip, make_video_clip};
use serde_json::json;

use crate::api::{
    BundleInspection, ImportBundleOptions, ImportBundleProgress, ImportBundleSummary,
};
use crate::fs_util::{ensure_output_dir_ready, resolve_pipeline_asset_path, seconds_to_micros};
use crate::manifest::BundleManifest;
use crate::media_cache::{MediaMaterialCache, MediaProbeRequest};
use crate::pipeline::concat::parse_concat_file;
use crate::pipeline::spec::PipelineSpec;
use crate::pipeline::srt::parse_srt_file;
use crate::pipeline::{
    emit_pipeline_progress, ensure_not_cancelled, progress_count_data,
    validate_pipeline_narration_count,
};
use crate::source::PreparedSource;
use crate::subtitle_style::{build_simple_subtitle_style, build_simple_subtitle_transform};

pub(crate) fn import_pipeline_legacy_package(
    options: &ImportBundleOptions,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
    pipeline: &PipelineSpec,
    progress: &mut impl FnMut(ImportBundleProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<ImportBundleSummary> {
    ensure_not_cancelled(is_cancelled)?;
    let concat_file = pipeline
        .concat_file
        .as_deref()
        .ok_or_else(|| anyhow!("pipeline.concat_file is required"))?;
    let subtitle_file = pipeline
        .subtitle_file
        .as_deref()
        .ok_or_else(|| anyhow!("pipeline.subtitle_file is required"))?;
    let concat_path = resolve_pipeline_asset_path(
        "pipeline.concat_file",
        concat_file,
        &prepared.bundle_root,
        bundle.assets_dir.as_deref(),
    )?;
    let subtitle_path = resolve_pipeline_asset_path(
        "pipeline.subtitle_file",
        subtitle_file,
        &prepared.bundle_root,
        bundle.assets_dir.as_deref(),
    )?;

    let video_files = parse_concat_file(&concat_path)
        .with_context(|| format!("failed to parse concat file: {concat_path}"))?;
    let subtitles = parse_srt_file(&subtitle_path)
        .with_context(|| format!("failed to parse subtitle file: {subtitle_path}"))?;
    validate_pipeline_narration_count(&pipeline.narration_files, &subtitles)?;
    emit_pipeline_progress(
        progress,
        "pipeline_prepare",
        "已读取 pipeline_package",
        json!({
            "video_count": video_files.len(),
            "audio_count": pipeline.narration_files.len(),
            "subtitle_count": subtitles.len(),
        }),
    );

    let mut probe_requests = Vec::with_capacity(video_files.len() + pipeline.narration_files.len());
    for (index, video_file) in video_files.iter().enumerate() {
        let material_path = resolve_pipeline_asset_path(
            &format!("concat file[{index}]"),
            video_file,
            &prepared.bundle_root,
            bundle.assets_dir.as_deref(),
        )
        .with_context(|| format!("invalid concat file[{index}] path"))?;
        probe_requests.push(MediaProbeRequest {
            path: material_path,
            label: format!("failed to load concat video[{index}]: {video_file}"),
        });
    }
    for (index, narration_file) in pipeline.narration_files.iter().enumerate() {
        let narration_path = resolve_pipeline_asset_path(
            &format!("pipeline.narration_files[{index}]"),
            narration_file,
            &prepared.bundle_root,
            bundle.assets_dir.as_deref(),
        )
        .with_context(|| format!("invalid pipeline.narration_files[{index}] path"))?;
        probe_requests.push(MediaProbeRequest {
            path: narration_path,
            label: format!("failed to load pipeline narration[{index}]: {narration_file}"),
        });
    }

    let media_cache = MediaMaterialCache::preload(
        probe_requests,
        |current, total, request| {
            emit_pipeline_progress(
                progress,
                "pipeline_probe",
                format!("探测素材 {current}/{total}"),
                progress_count_data(current, total, Some(request.path.as_str())),
            );
        },
        is_cancelled,
    )?;
    ensure_not_cancelled(is_cancelled)?;

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
        let material = media_cache
            .create_video_material(&material_path, None)
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
        let audio_material = media_cache
            .create_audio_material(&narration_path, None)
            .with_context(|| {
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

    ensure_not_cancelled(is_cancelled)?;
    emit_pipeline_progress(
        progress,
        "pipeline_write",
        "写入剪映草稿",
        json!({ "output": options.output.as_str() }),
    );
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

pub(crate) fn inspect_pipeline_legacy_package(
    source: &Utf8Path,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
    pipeline: &PipelineSpec,
) -> Result<BundleInspection> {
    let concat_file = pipeline
        .concat_file
        .as_deref()
        .ok_or_else(|| anyhow!("pipeline.concat_file is required"))?;
    let subtitle_file = pipeline
        .subtitle_file
        .as_deref()
        .ok_or_else(|| anyhow!("pipeline.subtitle_file is required"))?;
    let concat_path = resolve_pipeline_asset_path(
        "pipeline.concat_file",
        concat_file,
        &prepared.bundle_root,
        bundle.assets_dir.as_deref(),
    )?;
    let subtitle_path = resolve_pipeline_asset_path(
        "pipeline.subtitle_file",
        subtitle_file,
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
