use anyhow::{anyhow, bail, Context, Result};
use camino::Utf8Path;
use jy_draft::writer::write_draft;
use jy_media::material::create_video_material;
use jy_schema::{Canvas, MaterialKind, TimeRange, TrackKind};
use jy_timeline::builder::ProjectBuilder;
use jy_timeline::clip::{make_text_clip, make_video_clip};
use serde::Deserialize;

use crate::api::{BundleInspection, ImportBundleOptions, ImportBundleSummary};
use crate::fs_util::{ensure_output_dir_ready, resolve_simple_asset_path, seconds_to_micros};
use crate::manifest::BundleManifest;
use crate::source::{read_json, PreparedSource};
use crate::subtitle_style::{
    build_simple_subtitle_style, build_simple_subtitle_transform, validate_simple_subtitle_style,
    SimpleSubtitleStyle,
};

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
struct SimpleSubtitleCue {
    start: f64,
    end: f64,
    text: String,
}

pub(crate) fn import_simple_timeline_package(
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

pub(crate) fn inspect_simple_timeline_package(
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::write_test_mp4;
    use crate::{import_bundle, ImportBundleOptions};
    use camino::Utf8PathBuf;
    use serde_json::{json, Value};
    use std::fs;
    use tempfile::tempdir;

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
                "videos": [{ "path": "video_001.mp4" }, { "path": "video_002.mp4" }],
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
        let content = fs::read_to_string(output_dir.join("draft_content.json"))?;
        assert!(content.contains("main_video"));
        assert!(content.contains("subtitle"));
        assert!(content.contains("第一句字幕"));
        let draft: Value = serde_json::from_str(&content)?;
        let text_content = draft["materials"]["texts"][0]["content"]
            .as_str()
            .unwrap_or_default();
        let text_material: Value = serde_json::from_str(text_content)?;
        assert_eq!(text_material["styles"][0]["size"].as_f64(), Some(9.0));

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
                "bundle_type": "simple_timeline_package",
                "timeline_file": "timeline.json",
                "assets_dir": "assets"
            }))?,
        )?;
        fs::write(
            bundle_dir.join("timeline.json"),
            serde_json::to_string_pretty(&json!({"videos": [], "subtitles": []}))?,
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
                "bundle_type": "simple_timeline_package",
                "timeline_file": "timeline.json",
                "assets_dir": "assets"
            }))?,
        )?;
        fs::write(
            bundle_dir.join("timeline.json"),
            serde_json::to_string_pretty(
                &json!({"videos": [{"path": "missing.mp4"}], "subtitles": []}),
            )?,
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
}
