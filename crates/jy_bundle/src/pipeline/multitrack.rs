use std::collections::HashMap;

use anyhow::{anyhow, bail, Context, Result};
use jy_draft::writer::write_draft;
use jy_schema::{Canvas, MaterialKind, TimeRange, TrackKind};
use jy_timeline::builder::ProjectBuilder;
use jy_timeline::clip::{make_audio_clip, make_text_clip, make_video_clip};
use serde_json::json;

use crate::api::{ImportBundleOptions, ImportBundleProgress, ImportBundleSummary};
use crate::fs_util::{ensure_output_dir_ready, resolve_pipeline_asset_path, seconds_to_micros};
use crate::manifest::BundleManifest;
use crate::media_cache::{MediaMaterialCache, MediaProbeRequest};
use crate::pipeline::spec::{default_pipeline_volume, PipelineSpec, PipelineTrackKind};
use crate::pipeline::srt::parse_srt_file;
use crate::pipeline::{
    emit_pipeline_progress, ensure_not_cancelled, progress_count_data,
    validate_pipeline_multitrack_spec, validate_text_tracks_within_video_duration,
};
use crate::source::PreparedSource;
use crate::subtitle_style::{build_simple_subtitle_style, build_simple_subtitle_transform};

pub(crate) fn import_pipeline_multitrack_package(
    options: &ImportBundleOptions,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
    pipeline: &PipelineSpec,
    progress: &mut impl FnMut(ImportBundleProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<ImportBundleSummary> {
    ensure_not_cancelled(is_cancelled)?;
    let tracks = pipeline
        .tracks
        .as_deref()
        .ok_or_else(|| anyhow!("pipeline.tracks is required"))?;
    validate_pipeline_multitrack_spec(tracks)?;
    let media_clip_count = tracks
        .iter()
        .filter(|track| {
            matches!(
                track.kind,
                PipelineTrackKind::Video | PipelineTrackKind::Audio
            )
        })
        .map(|track| track.clips.len())
        .sum::<usize>();
    emit_pipeline_progress(
        progress,
        "pipeline_prepare",
        "已读取 pipeline.tracks",
        json!({
            "track_count": tracks.len(),
            "media_clip_count": media_clip_count,
        }),
    );

    let mut probe_requests = Vec::with_capacity(media_clip_count);
    for (track_index, track) in tracks.iter().enumerate() {
        if !matches!(
            track.kind,
            PipelineTrackKind::Video | PipelineTrackKind::Audio
        ) {
            continue;
        }
        for (clip_index, clip) in track.clips.iter().enumerate() {
            let material_path = resolve_pipeline_asset_path(
                &format!("pipeline.tracks[{track_index}].clips[{clip_index}].path"),
                &clip.path,
                &prepared.bundle_root,
                bundle.assets_dir.as_deref(),
            )
            .with_context(|| {
                format!("invalid pipeline.tracks[{track_index}].clips[{clip_index}].path")
            })?;
            probe_requests.push(MediaProbeRequest {
                path: material_path,
                label: format!(
                    "failed to load pipeline {} track '{}' clip[{clip_index}]: {}",
                    track.kind.as_str(),
                    track.name,
                    clip.path
                ),
            });
        }
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

    let mut builder = ProjectBuilder::new(&project_name, Canvas::default()).maintrack_adsorb(true);
    let mut render_offsets: HashMap<PipelineTrackKind, i32> = HashMap::new();
    for track in tracks {
        let offset = render_offsets.entry(track.kind).or_insert(0);
        builder = builder.add_track(track.kind.into(), &track.name, *offset)?;
        *offset += 1;
    }

    let mut asset_count = 0_usize;

    for (track_index, track) in tracks.iter().enumerate() {
        ensure_not_cancelled(is_cancelled)?;
        match track.kind {
            PipelineTrackKind::Video => {
                for (clip_index, clip) in track.clips.iter().enumerate() {
                    let material_path = resolve_pipeline_asset_path(
                        &format!("pipeline.tracks[{track_index}].clips[{clip_index}].path"),
                        &clip.path,
                        &prepared.bundle_root,
                        bundle.assets_dir.as_deref(),
                    )
                    .with_context(|| {
                        format!("invalid pipeline.tracks[{track_index}].clips[{clip_index}].path")
                    })?;
                    let material = media_cache
                        .create_video_material(&material_path, None)
                        .with_context(|| {
                            format!(
                                "failed to load pipeline video track '{}' clip[{clip_index}]: {}",
                                track.name, clip.path
                            )
                        })?;
                    if material.kind != MaterialKind::Video {
                        bail!(
                            "pipeline.tracks[{track_index}].clips[{clip_index}] is not a video material: {}",
                            clip.path
                        );
                    }
                    if material.duration == 0 {
                        bail!(
                            "pipeline.tracks[{track_index}].clips[{clip_index}] duration is zero: {}",
                            clip.path
                        );
                    }

                    let start = seconds_to_micros(
                        clip.start,
                        &format!("pipeline.tracks[{track_index}].clips[{clip_index}].start"),
                    )?;
                    let target = TimeRange::new(start, material.duration);
                    let video_clip = make_video_clip(
                        &material,
                        target,
                        None,
                        None,
                        clip.volume.unwrap_or_else(default_pipeline_volume),
                        None,
                    )
                    .with_context(|| {
                        format!(
                            "failed to build pipeline video track '{}' clip[{clip_index}]",
                            track.name
                        )
                    })?;
                    builder = builder
                        .add_video_material(material)
                        .add_clip_to_track(&track.name, video_clip)?;
                    asset_count += 1;
                }
            }
            PipelineTrackKind::Audio => {
                for (clip_index, clip) in track.clips.iter().enumerate() {
                    let end = clip.end.ok_or_else(|| {
                        anyhow!(
                            "pipeline.tracks[{track_index}].clips[{clip_index}].end is required for audio tracks"
                        )
                    })?;
                    if end <= clip.start {
                        bail!(
                            "pipeline.tracks[{track_index}].clips[{clip_index}].end must be greater than start"
                        );
                    }
                    let material_path = resolve_pipeline_asset_path(
                        &format!("pipeline.tracks[{track_index}].clips[{clip_index}].path"),
                        &clip.path,
                        &prepared.bundle_root,
                        bundle.assets_dir.as_deref(),
                    )
                    .with_context(|| {
                        format!("invalid pipeline.tracks[{track_index}].clips[{clip_index}].path")
                    })?;
                    let audio_material = media_cache
                        .create_audio_material(&material_path, None)
                        .with_context(|| {
                            format!(
                                "failed to load pipeline audio track '{}' clip[{clip_index}]: {}",
                                track.name, clip.path
                            )
                        })?;
                    let start_us = seconds_to_micros(
                        clip.start,
                        &format!("pipeline.tracks[{track_index}].clips[{clip_index}].start"),
                    )?;
                    let end_us = seconds_to_micros(
                        end,
                        &format!("pipeline.tracks[{track_index}].clips[{clip_index}].end"),
                    )?;
                    let requested_duration = end_us - start_us;
                    let audio_duration = audio_material.duration.min(requested_duration);
                    if audio_duration == 0 {
                        bail!(
                            "pipeline.tracks[{track_index}].clips[{clip_index}] duration is zero"
                        );
                    }
                    let audio_clip = make_audio_clip(
                        &audio_material,
                        TimeRange::new(start_us, audio_duration),
                        Some(TimeRange::new(0, audio_duration)),
                        None,
                        clip.volume.unwrap_or_else(default_pipeline_volume),
                    )?;
                    builder = builder
                        .add_audio_material(audio_material)
                        .add_clip_to_track(&track.name, audio_clip)?;
                    asset_count += 1;
                }
            }
            PipelineTrackKind::Text => {
                let subtitle_file = track.subtitle_file.as_deref().ok_or_else(|| {
                    anyhow!(
                        "pipeline.tracks[{track_index}].subtitle_file is required for text tracks"
                    )
                })?;
                let subtitle_path = resolve_pipeline_asset_path(
                    &format!("pipeline.tracks[{track_index}].subtitle_file"),
                    subtitle_file,
                    &prepared.bundle_root,
                    bundle.assets_dir.as_deref(),
                )?;
                let subtitles = parse_srt_file(&subtitle_path)
                    .with_context(|| format!("failed to parse subtitle file: {subtitle_path}"))?;
                let style_spec = track.style.as_ref().unwrap_or(&bundle.subtitle_style);
                let text_style = build_simple_subtitle_style(style_spec)?;
                let text_transform = build_simple_subtitle_transform(style_spec)?;
                for (cue_index, cue) in subtitles.iter().enumerate() {
                    let start = seconds_to_micros(
                        cue.start,
                        &format!("pipeline.tracks[{track_index}].subtitle_file[{cue_index}].start"),
                    )?;
                    let end = seconds_to_micros(
                        cue.end,
                        &format!("pipeline.tracks[{track_index}].subtitle_file[{cue_index}].end"),
                    )?;
                    let clip = make_text_clip(
                        &cue.text,
                        TimeRange::new(start, end - start),
                        Some(text_style.clone()),
                        Some(text_transform.clone()),
                    );
                    builder = builder.add_clip_to_track(&track.name, clip)?;
                }
            }
        }
    }

    let mut project = builder.build();
    if !project
        .tracks
        .iter()
        .any(|track| track.kind == TrackKind::Video && !track.clips.is_empty())
    {
        bail!("pipeline.tracks must contain at least one video clip");
    }
    validate_text_tracks_within_video_duration(&project)?;
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
        asset_count,
        video_material_count: project.video_materials.len(),
        audio_material_count: project.audio_materials.len(),
    })
}
