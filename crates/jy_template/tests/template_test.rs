use camino::Utf8PathBuf;
use jy_draft::converter::project_to_draft;
use jy_schema::{
    AudioClip, AudioMaterialRef, Canvas, Clip, CropSettings, MaterialKind, Project, Speed,
    TextClip, TextStyle, TimeRange, Track, TrackKind, Transform, VideoClip, VideoMaterialRef,
};
use jy_template::{ExtendMode, ReplacementMaterial, ShrinkMode, TemplateDraft, TrackSelector};
use tempfile::tempdir;

fn new_id() -> String {
    uuid::Uuid::new_v4().as_simple().to_string()
}

fn make_video_material(name: &str, duration_us: u64, path: &str) -> VideoMaterialRef {
    VideoMaterialRef {
        id: new_id(),
        path: Utf8PathBuf::from(path),
        duration: duration_us,
        width: 1920,
        height: 1080,
        kind: MaterialKind::Video,
        crop: CropSettings::default(),
        name: name.to_string(),
    }
}

fn make_audio_material(name: &str, duration_us: u64, path: &str) -> AudioMaterialRef {
    AudioMaterialRef {
        id: new_id(),
        path: Utf8PathBuf::from(path),
        duration: duration_us,
        name: name.to_string(),
    }
}

fn sample_project() -> Project {
    let video_mat = make_video_material("main_video.mp4", 8_000_000, "C:/test/main_video.mp4");
    let audio_mat = make_audio_material("voice.mp3", 8_000_000, "C:/test/voice.mp3");

    let video_track = Track {
        id: new_id(),
        kind: TrackKind::Video,
        name: "video_main".into(),
        render_index: 0,
        mute: false,
        clips: vec![Clip::Video(VideoClip {
            id: new_id(),
            material_id: video_mat.id.clone(),
            target_timerange: TimeRange::new(0, 5_000_000),
            source_timerange: Some(TimeRange::new(0, 5_000_000)),
            speed: Speed {
                id: new_id(),
                speed: 1.0,
            },
            volume: 1.0,
            change_pitch: false,
            transform: Transform::default(),
            keyframes: Vec::new(),
            fade: None,
            effects: Vec::new(),
            filters: Vec::new(),
            mask: None,
            transition: None,
            background_filling: None,
            animations: None,
            mix_mode: None,
        })],
    };

    let audio_track = Track {
        id: new_id(),
        kind: TrackKind::Audio,
        name: "audio_main".into(),
        render_index: 0,
        mute: false,
        clips: vec![Clip::Audio(AudioClip {
            id: new_id(),
            material_id: audio_mat.id.clone(),
            target_timerange: TimeRange::new(0, 5_000_000),
            source_timerange: Some(TimeRange::new(0, 5_000_000)),
            speed: Speed {
                id: new_id(),
                speed: 1.0,
            },
            volume: 1.0,
            change_pitch: false,
            keyframes: Vec::new(),
            fade: None,
            effects: Vec::new(),
        })],
    };

    let text_track = Track {
        id: new_id(),
        kind: TrackKind::Text,
        name: "subtitle".into(),
        render_index: 15000,
        mute: false,
        clips: vec![Clip::Text(TextClip {
            id: new_id(),
            material_id: new_id(),
            target_timerange: TimeRange::new(0, 5_000_000),
            text: "你好世界".into(),
            font: None,
            style: TextStyle::default(),
            transform: Transform::default(),
            keyframes: Vec::new(),
            border: None,
            background: None,
            shadow: None,
            animations: None,
            bubble: None,
            effect: None,
        })],
    };

    Project {
        id: new_id(),
        name: "template_project".into(),
        canvas: Canvas::default(),
        maintrack_adsorb: true,
        tracks: vec![video_track, audio_track, text_track],
        video_materials: vec![video_mat],
        audio_materials: vec![audio_mat],
        duration: 5_000_000,
    }
}

#[test]
fn replace_material_by_name_updates_video_material() {
    let project = sample_project();
    let value = project_to_draft(&project).unwrap();
    let mut draft = TemplateDraft::from_value(value);
    let replacement = ReplacementMaterial::Video(make_video_material(
        "replacement.mp4",
        12_000_000,
        "C:/replacement/replacement.mp4",
    ));

    draft
        .replace_material_by_name("main_video.mp4", &replacement, true)
        .unwrap();

    let videos = draft
        .content()
        .get("materials")
        .and_then(|m| m.get("videos"))
        .and_then(|v| v.as_array())
        .unwrap();
    let video = &videos[0];
    assert_eq!(video["material_name"], "replacement.mp4");
    assert_eq!(video["path"], "C:/replacement/replacement.mp4");
    assert_eq!(video["duration"], 12_000_000);
}

#[test]
fn replace_text_uses_character_count_for_style_ranges() {
    let project = sample_project();
    let value = project_to_draft(&project).unwrap();
    let mut draft = TemplateDraft::from_value(value);

    draft
        .replace_text(
            &TrackSelector {
                name: Some("subtitle".into()),
                index: None,
            },
            0,
            "你好，剪映",
            true,
        )
        .unwrap();

    let text_material = draft
        .content()
        .get("materials")
        .and_then(|m| m.get("texts"))
        .and_then(|v| v.as_array())
        .unwrap()[0]
        .clone();
    let content: serde_json::Value =
        serde_json::from_str(text_material["content"].as_str().unwrap()).unwrap();

    assert_eq!(content["text"], "你好，剪映");
    assert_eq!(content["styles"][0]["range"][1], 5);
}

#[test]
fn replace_material_by_seg_extends_clip_and_swaps_material_id() {
    let project = sample_project();
    let value = project_to_draft(&project).unwrap();
    let mut draft = TemplateDraft::from_value(value);
    let replacement_material =
        make_video_material("longer.mp4", 10_000_000, "C:/replacement/longer.mp4");
    let expected_id = replacement_material.id.clone();
    let replacement = ReplacementMaterial::Video(replacement_material);

    draft
        .replace_material_by_seg(
            TrackKind::Video,
            &TrackSelector {
                name: Some("video_main".into()),
                index: None,
            },
            0,
            &replacement,
            Some(TimeRange::new(0, 7_000_000)),
            ShrinkMode::CutTail,
            &[ExtendMode::PushTail],
        )
        .unwrap();

    let track = draft
        .content()
        .get("tracks")
        .and_then(|v| v.as_array())
        .unwrap()
        .iter()
        .find(|track| track["name"] == "video_main")
        .unwrap();
    let segment = &track["segments"][0];
    assert_eq!(segment["material_id"], expected_id);
    assert_eq!(segment["source_timerange"]["duration"], 7_000_000);
    assert_eq!(segment["target_timerange"]["duration"], 7_000_000);
}

#[test]
fn replace_texts_updates_text_template_parts() {
    let project = sample_project();
    let mut value = project_to_draft(&project).unwrap();

    let text_material_id = value["materials"]["texts"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let extra_text_material_id = new_id();

    value["materials"]["texts"]
        .as_array_mut()
        .unwrap()
        .push(serde_json::json!({
            "id": extra_text_material_id,
            "content": serde_json::to_string(&serde_json::json!({
                "styles": [{
                    "range": [0, 2],
                    "size": 8.0,
                    "bold": false,
                    "italic": false,
                    "underline": false,
                    "strokes": [],
                    "fill": {
                        "alpha": 1.0,
                        "content": {
                            "render_type": "solid",
                            "solid": { "alpha": 1.0, "color": [1.0, 1.0, 1.0] }
                        }
                    }
                }],
                "text": "世界"
            })).unwrap(),
            "typesetting": 0,
            "alignment": 0,
            "letter_spacing": 0.0,
            "line_spacing": 0.02,
            "line_feed": 1,
            "line_max_width": 0.82,
            "force_apply_line_max_width": false,
            "check_flag": 7,
            "type": "text",
            "global_alpha": 1.0
        }));

    let template_material_id = new_id();
    value["materials"]["text_templates"] = serde_json::json!([{
        "id": template_material_id,
        "name": "double-text",
        "text_info_resources": [
            { "text_material_id": text_material_id },
            { "text_material_id": extra_text_material_id }
        ]
    }]);
    value["tracks"][2]["segments"][0]["material_id"] = serde_json::json!(template_material_id);

    let mut draft = TemplateDraft::from_value(value);
    draft
        .replace_texts(
            &TrackSelector {
                name: Some("subtitle".into()),
                index: None,
            },
            0,
            &["你好".to_string(), "Rust".to_string()],
            true,
        )
        .unwrap();

    let texts = draft.content()["materials"]["texts"].as_array().unwrap();
    let first: serde_json::Value = serde_json::from_str(
        texts
            .iter()
            .find(|item| item["id"] == text_material_id)
            .unwrap()["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let second: serde_json::Value = serde_json::from_str(
        texts
            .iter()
            .find(|item| item["id"] == extra_text_material_id)
            .unwrap()["content"]
            .as_str()
            .unwrap(),
    )
    .unwrap();

    assert_eq!(first["text"], "你好");
    assert_eq!(second["text"], "Rust");
}

#[test]
fn duplicate_draft_dir_copies_files_and_loads_new_content() {
    let dir = tempdir().unwrap();
    let template_dir = Utf8PathBuf::from_path_buf(dir.path().join("template")).unwrap();
    let new_dir = Utf8PathBuf::from_path_buf(dir.path().join("copy")).unwrap();
    std::fs::create_dir_all(&template_dir).unwrap();

    let project = sample_project();
    let draft_content = project_to_draft(&project).unwrap();
    std::fs::write(
        template_dir.join("draft_content.json"),
        serde_json::to_string_pretty(&draft_content).unwrap(),
    )
    .unwrap();
    std::fs::write(
        template_dir.join("draft_meta_info.json"),
        "{\"draft_name\":\"template\"}",
    )
    .unwrap();

    let duplicated = TemplateDraft::duplicate_draft_dir(&template_dir, &new_dir, false).unwrap();
    assert!(new_dir.join("draft_content.json").exists());
    assert!(new_dir.join("draft_meta_info.json").exists());
    assert_eq!(duplicated.content()["tracks"][2]["name"], "subtitle");
}
