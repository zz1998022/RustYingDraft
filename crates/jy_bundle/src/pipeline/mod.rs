pub(crate) mod concat;
pub(crate) mod legacy;
pub(crate) mod multitrack;
pub(crate) mod spec;
pub(crate) mod srt;

use std::collections::HashMap;

use anyhow::{anyhow, bail, Context, Result};
use camino::Utf8Path;
use jy_schema::TrackKind;
use serde_json::{json, Value};

use crate::api::{
    BundleInspection, ImportBundleOptions, ImportBundleProgress, ImportBundleSummary,
};
use crate::fs_util::{resolve_pipeline_asset_path, validate_simple_relative_path};
use crate::manifest::BundleManifest;
use crate::source::PreparedSource;
use crate::subtitle_style::validate_simple_subtitle_style;

use spec::{
    PipelineAudioStyle, PipelineClipSpec, PipelineSpec, PipelineTrackKind, PipelineTrackSpec,
};
use srt::SrtCue;

pub(crate) fn import_pipeline_package(
    options: &ImportBundleOptions,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
    progress: &mut impl FnMut(ImportBundleProgress),
    is_cancelled: &impl Fn() -> bool,
) -> Result<ImportBundleSummary> {
    let pipeline = bundle
        .pipeline
        .as_ref()
        .ok_or_else(|| anyhow!("pipeline_package requires bundle.pipeline"))?;
    validate_simple_subtitle_style(&bundle.subtitle_style)?;
    validate_pipeline_audio_style(&bundle.audio_style)?;
    validate_pipeline_spec(pipeline)?;
    if pipeline.tracks.is_some() {
        return multitrack::import_pipeline_multitrack_package(
            options,
            prepared,
            bundle,
            pipeline,
            progress,
            is_cancelled,
        );
    }
    legacy::import_pipeline_legacy_package(
        options,
        prepared,
        bundle,
        pipeline,
        progress,
        is_cancelled,
    )
}

pub(crate) fn inspect_pipeline_package(
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
    if let Some(tracks) = &pipeline.tracks {
        return inspect_pipeline_multitrack_package(source, prepared, bundle, tracks);
    }
    legacy::inspect_pipeline_legacy_package(source, prepared, bundle, pipeline)
}

fn inspect_pipeline_multitrack_package(
    source: &Utf8Path,
    prepared: &PreparedSource,
    bundle: &BundleManifest,
    tracks: &[PipelineTrackSpec],
) -> Result<BundleInspection> {
    validate_pipeline_multitrack_spec(tracks)?;

    let mut asset_kinds = Vec::new();
    for (track_index, track) in tracks.iter().enumerate() {
        match track.kind {
            PipelineTrackKind::Video | PipelineTrackKind::Audio => {
                for (clip_index, clip) in track.clips.iter().enumerate() {
                    resolve_pipeline_asset_path(
                        &format!("pipeline.tracks[{track_index}].clips[{clip_index}].path"),
                        &clip.path,
                        &prepared.bundle_root,
                        bundle.assets_dir.as_deref(),
                    )
                    .with_context(|| {
                        format!("invalid pipeline.tracks[{track_index}].clips[{clip_index}].path")
                    })?;
                    asset_kinds.push(track.kind.as_str().to_string());
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
                srt::parse_srt_file(&subtitle_path)
                    .with_context(|| format!("failed to parse subtitle file: {subtitle_path}"))?;
            }
        }
    }

    Ok(BundleInspection {
        source: source.as_str().to_string(),
        bundle_root: prepared.bundle_root.as_str().to_string(),
        bundle_type: "pipeline_package".to_string(),
        timeline_file: None,
        source_draft_dir: None,
        project_id: bundle.project_id.clone(),
        project_name: bundle.project_name.clone(),
        asset_count: asset_kinds.len(),
        track_count: tracks.len(),
        asset_kinds,
    })
}

pub(crate) fn validate_pipeline_audio_style(style: &PipelineAudioStyle) -> Result<()> {
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
    if let Some(tracks) = &pipeline.tracks {
        if pipeline.concat_file.is_some()
            || pipeline.subtitle_file.is_some()
            || !pipeline.narration_files.is_empty()
        {
            bail!(
                "pipeline.tracks cannot be combined with concat_file, subtitle_file, or narration_files"
            );
        }
        validate_pipeline_multitrack_spec(tracks)?;
        return Ok(());
    }
    if pipeline.concat_file.is_none() {
        bail!("pipeline.concat_file is required");
    }
    if pipeline.subtitle_file.is_none() {
        bail!("pipeline.subtitle_file is required");
    }
    if pipeline.narration_files.is_empty() {
        bail!("pipeline.narration_files must contain at least one narration file");
    }
    Ok(())
}

pub(crate) fn validate_pipeline_multitrack_spec(tracks: &[PipelineTrackSpec]) -> Result<()> {
    if tracks.is_empty() {
        bail!("pipeline.tracks must contain at least one track");
    }

    let mut names = HashMap::new();
    let mut has_video_clip = false;
    for (track_index, track) in tracks.iter().enumerate() {
        if track.name.trim().is_empty() {
            bail!("pipeline.tracks[{track_index}].name must not be empty");
        }
        if names.insert(track.name.as_str(), track_index).is_some() {
            bail!("pipeline.tracks name duplicated: {}", track.name);
        }

        match track.kind {
            PipelineTrackKind::Video => {
                if track.subtitle_file.is_some() {
                    bail!("pipeline.tracks[{track_index}].subtitle_file is only allowed on text tracks");
                }
                if track.clips.is_empty() {
                    bail!(
                        "pipeline.tracks[{track_index}].clips must contain at least one video clip"
                    );
                }
                for (clip_index, clip) in track.clips.iter().enumerate() {
                    validate_pipeline_clip_common(track_index, clip_index, clip)?;
                    if clip.end.is_some() {
                        bail!(
                            "pipeline.tracks[{track_index}].clips[{clip_index}].end is not supported for video tracks"
                        );
                    }
                    has_video_clip = true;
                }
            }
            PipelineTrackKind::Audio => {
                if track.subtitle_file.is_some() {
                    bail!("pipeline.tracks[{track_index}].subtitle_file is only allowed on text tracks");
                }
                if track.clips.is_empty() {
                    bail!(
                        "pipeline.tracks[{track_index}].clips must contain at least one audio clip"
                    );
                }
                let mut ranges = Vec::new();
                for (clip_index, clip) in track.clips.iter().enumerate() {
                    validate_pipeline_clip_common(track_index, clip_index, clip)?;
                    let end = clip.end.ok_or_else(|| {
                        anyhow!(
                            "pipeline.tracks[{track_index}].clips[{clip_index}].end is required for audio tracks"
                        )
                    })?;
                    if !end.is_finite() || end <= clip.start {
                        bail!(
                            "pipeline.tracks[{track_index}].clips[{clip_index}].end must be greater than start"
                        );
                    }
                    ranges.push((clip.start, end, clip_index));
                }
                validate_no_overlap(track_index, &track.name, &mut ranges)?;
            }
            PipelineTrackKind::Text => {
                if !track.clips.is_empty() {
                    bail!("pipeline.tracks[{track_index}].clips is not supported for text tracks");
                }
                let subtitle_file = track.subtitle_file.as_deref().ok_or_else(|| {
                    anyhow!(
                        "pipeline.tracks[{track_index}].subtitle_file is required for text tracks"
                    )
                })?;
                validate_simple_relative_path(
                    &format!("pipeline.tracks[{track_index}].subtitle_file"),
                    subtitle_file,
                )?;
                if let Some(style) = &track.style {
                    validate_simple_subtitle_style(style)?;
                }
            }
        }
    }

    if !has_video_clip {
        bail!("pipeline.tracks must contain at least one video clip");
    }
    Ok(())
}

fn validate_pipeline_clip_common(
    track_index: usize,
    clip_index: usize,
    clip: &PipelineClipSpec,
) -> Result<()> {
    validate_simple_relative_path(
        &format!("pipeline.tracks[{track_index}].clips[{clip_index}].path"),
        &clip.path,
    )?;
    if !clip.start.is_finite() || clip.start < 0.0 {
        bail!(
            "pipeline.tracks[{track_index}].clips[{clip_index}].start must be non-negative seconds"
        );
    }
    if let Some(volume) = clip.volume {
        if !volume.is_finite() || volume < 0.0 {
            bail!("pipeline.tracks[{track_index}].clips[{clip_index}].volume must be non-negative");
        }
    }
    Ok(())
}

fn validate_no_overlap(
    track_index: usize,
    track_name: &str,
    ranges: &mut [(f64, f64, usize)],
) -> Result<()> {
    ranges.sort_by(|left, right| left.0.total_cmp(&right.0));
    for pair in ranges.windows(2) {
        let (prev_start, prev_end, prev_index) = pair[0];
        let (next_start, _next_end, next_index) = pair[1];
        if next_start < prev_end {
            bail!(
                "pipeline.tracks[{track_index}] '{}' clips overlap: clip[{prev_index}] ({prev_start}-{prev_end}) and clip[{next_index}]",
                track_name
            );
        }
    }
    Ok(())
}

pub(crate) fn validate_text_tracks_within_video_duration(
    project: &jy_schema::Project,
) -> Result<()> {
    let video_duration = project
        .tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Video)
        .flat_map(|track| track.clips.iter())
        .map(|clip| clip.target_timerange().end())
        .max()
        .unwrap_or(0);

    for track in project
        .tracks
        .iter()
        .filter(|track| track.kind == TrackKind::Text)
    {
        for (clip_index, clip) in track.clips.iter().enumerate() {
            let end = clip.target_timerange().end();
            if end > video_duration {
                bail!(
                    "pipeline text track '{}' clip[{clip_index}] end exceeds video duration",
                    track.name
                );
            }
        }
    }

    Ok(())
}

pub(crate) fn validate_pipeline_narration_count(
    narration_files: &[String],
    subtitles: &[SrtCue],
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

pub(crate) fn emit_pipeline_progress(
    progress: &mut impl FnMut(ImportBundleProgress),
    stage: &str,
    message: impl Into<String>,
    data: Value,
) {
    progress(ImportBundleProgress {
        stage: stage.to_string(),
        message: message.into(),
        data,
    });
}

pub(crate) fn ensure_not_cancelled(is_cancelled: &impl Fn() -> bool) -> Result<()> {
    if is_cancelled() {
        bail!("import cancelled");
    }
    Ok(())
}

pub(crate) fn progress_count_data(current: usize, total: usize, path: Option<&str>) -> Value {
    json!({
        "current": current,
        "total": total,
        "path": path,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::{write_test_mp4, write_test_wav};
    use crate::{import_bundle, import_bundle_with_progress, ImportBundleOptions};
    use camino::Utf8PathBuf;
    use serde_json::json;
    use std::fs;
    use tempfile::tempdir;

    #[test]
    fn import_pipeline_legacy_package_generates_draft() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("pipeline_bundle")).unwrap();
        let assets_dir = bundle_dir.join("assets");
        let narration_dir = assets_dir.join("narration");
        fs::create_dir_all(&narration_dir)?;

        if !write_test_mp4(&assets_dir.join("video_001.mp4"))?
            || !write_test_mp4(&assets_dir.join("video_002.mp4"))?
            || !write_test_wav(&narration_dir.join("001.wav"), 1)?
            || !write_test_wav(&narration_dir.join("002.wav"), 1)?
        {
            return Ok(());
        }

        fs::write(
            assets_dir.join("concat.txt"),
            "file 'video_001.mp4'\nfile 'video_002.mp4'\n",
        )?;
        fs::write(
            assets_dir.join("subtitle.srt"),
            "1\n00:00:00,000 --> 00:00:00,400\n第一句\n\n2\n00:00:01,000 --> 00:00:01,400\n第二句\n",
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
                    "narration_files": ["narration/001.wav", "narration/002.wav"]
                },
                "subtitle_style": { "font_size": 8.0, "x": 0.5, "y": 0.82 },
                "audio_style": { "video_volume": 1.0, "narration_volume": 1.0 }
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

        let content = fs::read_to_string(
            Utf8PathBuf::from(summary.draft_dir.as_str()).join("draft_content.json"),
        )?;
        assert!(content.contains("main_video"));
        assert!(content.contains("audio"));
        assert!(content.contains("subtitle"));
        assert!(content.contains("第一句"));

        Ok(())
    }

    #[test]
    fn import_pipeline_multitrack_package_generates_draft() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir =
            Utf8PathBuf::from_path_buf(temp.path().join("pipeline_multitrack")).unwrap();
        let assets_dir = bundle_dir.join("assets");
        fs::create_dir_all(assets_dir.join("video"))?;
        fs::create_dir_all(assets_dir.join("audio"))?;
        fs::create_dir_all(assets_dir.join("subtitle"))?;

        if !write_test_mp4(&assets_dir.join("video").join("main.mp4"))?
            || !write_test_mp4(&assets_dir.join("video").join("overlay.mp4"))?
            || !write_test_wav(&assets_dir.join("audio").join("narration.wav"), 1)?
            || !write_test_wav(&assets_dir.join("audio").join("bgm.wav"), 1)?
        {
            return Ok(());
        }

        fs::write(
            assets_dir.join("subtitle").join("cn.srt"),
            "1\n00:00:00,000 --> 00:00:00,400\n主字幕\n",
        )?;
        fs::write(
            assets_dir.join("subtitle").join("comment.srt"),
            "1\n00:00:00,200 --> 00:00:00,600\n评论字幕\n",
        )?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "pipeline_package",
                "project_id": "proj_pipeline_multi",
                "project_name": "Pipeline Multi",
                "assets_dir": "assets",
                "pipeline": {
                    "tracks": [
                        {"kind": "video", "name": "main_video", "clips": [{"path": "video/main.mp4", "start": 0.0}]},
                        {"kind": "video", "name": "overlay_video", "clips": [{"path": "video/overlay.mp4", "start": 0.2, "volume": 0.0}]},
                        {"kind": "audio", "name": "narration", "clips": [{"path": "audio/narration.wav", "start": 0.0, "end": 0.4}]},
                        {"kind": "audio", "name": "bgm", "clips": [{"path": "audio/bgm.wav", "start": 0.0, "end": 0.8, "volume": 0.35}]},
                        {"kind": "text", "name": "subtitle_cn", "subtitle_file": "subtitle/cn.srt", "style": {"font_size": 8.0, "x": 0.5, "y": 0.82}},
                        {"kind": "text", "name": "subtitle_comment", "subtitle_file": "subtitle/comment.srt", "style": {"font_size": 6.0, "x": 0.5, "y": 0.72}}
                    ]
                }
            }))?,
        )?;

        let mut progress_events = Vec::new();
        let summary = import_bundle_with_progress(
            &ImportBundleOptions {
                source: bundle_dir,
                output: Utf8PathBuf::from_path_buf(temp.path().join("pipeline_multi_draft"))
                    .unwrap(),
                name_override: None,
            },
            |event| progress_events.push(event),
        )?;

        assert_eq!(summary.bundle_type, "pipeline_package");
        assert_eq!(summary.asset_count, 4);
        assert_eq!(summary.track_count, 6);
        assert_eq!(summary.video_material_count, 2);
        assert_eq!(summary.audio_material_count, 2);

        let content = fs::read_to_string(
            Utf8PathBuf::from(summary.draft_dir.as_str()).join("draft_content.json"),
        )?;
        assert!(content.contains("main_video"));
        assert!(content.contains("overlay_video"));
        assert!(content.contains("subtitle_comment"));
        assert!(content.contains("评论字幕"));
        assert!(progress_events
            .iter()
            .any(|event| event.stage == "pipeline_probe"));
        assert!(progress_events
            .iter()
            .any(|event| event.stage == "pipeline_write"));

        Ok(())
    }

    #[test]
    fn pipeline_package_rejects_tracks_mixed_with_legacy_fields() -> Result<()> {
        let manifest: crate::manifest::BundleManifest = serde_json::from_value(json!({
            "bundle_type": "pipeline_package",
            "project_name": "Mixed Pipeline",
            "pipeline": {
                "concat_file": "concat.txt",
                "subtitle_file": "subtitle.srt",
                "narration_files": ["001.wav"],
                "tracks": [
                    {"kind": "video", "name": "main", "clips": [{"path": "video.mp4", "start": 0.0}]}
                ]
            }
        }))?;

        let error = validate_pipeline_spec(manifest.pipeline.as_ref().unwrap()).unwrap_err();
        assert!(format!("{error:#}").contains("pipeline.tracks cannot be combined"));
        Ok(())
    }

    #[test]
    fn pipeline_package_rejects_deprecated_audio_file() -> Result<()> {
        let manifest: crate::manifest::BundleManifest = serde_json::from_value(json!({
            "bundle_type": "pipeline_package",
            "project_name": "Deprecated Audio File",
            "pipeline": {
                "audio_file": "audio.wav",
                "concat_file": "concat.txt",
                "subtitle_file": "subtitle.srt"
            }
        }))?;

        let error = validate_pipeline_spec(manifest.pipeline.as_ref().unwrap()).unwrap_err();
        assert!(format!("{error:#}").contains("pipeline.audio_file is no longer supported"));
        Ok(())
    }

    #[test]
    fn pipeline_import_stops_before_progress_when_video_clip_has_end() -> Result<()> {
        let temp = tempdir()?;
        let bundle_dir = Utf8PathBuf::from_path_buf(temp.path().join("invalid_video_end")).unwrap();
        fs::create_dir_all(bundle_dir.join("assets").join("video"))?;
        fs::write(
            bundle_dir.join("bundle.json"),
            serde_json::to_string_pretty(&json!({
                "bundle_version": 1,
                "bundle_type": "pipeline_package",
                "project_name": "Invalid Video End",
                "assets_dir": "assets",
                "pipeline": {
                    "tracks": [
                        {
                            "kind": "video",
                            "name": "video",
                            "clips": [
                                { "path": "video/88.mp4", "start": 0.0, "end": 1.0 }
                            ]
                        }
                    ]
                }
            }))?,
        )?;

        let mut progress_events = Vec::new();
        let error = import_bundle_with_progress(
            &ImportBundleOptions {
                source: bundle_dir,
                output: Utf8PathBuf::from_path_buf(temp.path().join("draft")).unwrap(),
                name_override: None,
            },
            |event| progress_events.push(event),
        )
        .unwrap_err();

        assert!(format!("{error:#}")
            .contains("pipeline.tracks[0].clips[0].end is not supported for video tracks"));
        assert!(progress_events.is_empty());
        Ok(())
    }
}
