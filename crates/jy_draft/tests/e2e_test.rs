use jy_draft::converter::project_to_draft;
use jy_schema::{
    AudioClip, AudioMaterialRef, Canvas, Clip, CropSettings, MaterialKind, Project, Speed,
    TextClip, TimeRange, Track, TrackKind, Transform, VideoClip, VideoMaterialRef,
};
use serde_json::Value;

fn new_id() -> String {
    uuid::Uuid::new_v4().as_simple().to_string()
}

fn make_video_material(name: &str, duration_us: u64) -> VideoMaterialRef {
    VideoMaterialRef {
        id: new_id(),
        path: camino::Utf8PathBuf::from(format!("C:/test/{}.mp4", name)),
        duration: duration_us,
        width: 1920,
        height: 1080,
        kind: MaterialKind::Video,
        crop: CropSettings::default(),
        name: name.to_string(),
    }
}

fn make_audio_material(name: &str, duration_us: u64) -> AudioMaterialRef {
    AudioMaterialRef {
        id: new_id(),
        path: camino::Utf8PathBuf::from(format!("C:/test/{}.mp3", name)),
        duration: duration_us,
        name: name.to_string(),
    }
}

fn make_video_clip(mat: &VideoMaterialRef, start_us: u64, duration_us: u64) -> Clip {
    Clip::Video(VideoClip {
        id: new_id(),
        material_id: mat.id.clone(),
        target_timerange: TimeRange::new(start_us, duration_us),
        source_timerange: Some(TimeRange::new(0, duration_us)),
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
    })
}

fn make_audio_clip(mat: &AudioMaterialRef, start_us: u64, duration_us: u64) -> Clip {
    Clip::Audio(AudioClip {
        id: new_id(),
        material_id: mat.id.clone(),
        target_timerange: TimeRange::new(start_us, duration_us),
        source_timerange: Some(TimeRange::new(0, duration_us)),
        speed: Speed {
            id: new_id(),
            speed: 1.0,
        },
        volume: 0.8,
        change_pitch: false,
        keyframes: Vec::new(),
        fade: None,
        effects: Vec::new(),
    })
}

fn make_text_clip(text: &str, start_us: u64, duration_us: u64) -> Clip {
    Clip::Text(TextClip {
        id: new_id(),
        material_id: new_id(),
        target_timerange: TimeRange::new(start_us, duration_us),
        text: text.to_string(),
        font: None,
        style: jy_schema::TextStyle::default(),
        transform: Transform::default(),
        keyframes: Vec::new(),
        border: None,
        background: None,
        shadow: None,
        animations: None,
        bubble: None,
        effect: None,
    })
}

#[test]
fn test_generate_basic_draft() {
    let video_mat = make_video_material("main_video", 5_000_000);
    let audio_mat = make_audio_material("bgm", 10_000_000);

    let video_track = Track {
        id: new_id(),
        kind: TrackKind::Video,
        name: "video1".into(),
        render_index: 0,
        mute: false,
        clips: vec![make_video_clip(&video_mat, 0, 5_000_000)],
    };

    let audio_track = Track {
        id: new_id(),
        kind: TrackKind::Audio,
        name: "audio1".into(),
        render_index: 0,
        mute: false,
        clips: vec![make_audio_clip(&audio_mat, 0, 5_000_000)],
    };

    let text_track = Track {
        id: new_id(),
        kind: TrackKind::Text,
        name: "text1".into(),
        render_index: 15000,
        mute: false,
        clips: vec![make_text_clip("Hello JianYing!", 0, 5_000_000)],
    };

    let project = Project {
        id: new_id(),
        name: "test_draft".into(),
        canvas: Canvas::default(),
        maintrack_adsorb: true,
        tracks: vec![video_track, audio_track, text_track],
        video_materials: vec![video_mat],
        audio_materials: vec![audio_mat],
        duration: 5_000_000,
    };

    let draft = project_to_draft(&project).expect("draft generation should succeed");

    // Verify top-level fields
    assert_eq!(draft["duration"], 5_000_000);
    assert_eq!(draft["fps"], 30.0);
    assert_eq!(draft["canvas_config"]["width"], 1920);
    assert_eq!(draft["canvas_config"]["height"], 1080);

    // Verify materials
    let materials = &draft["materials"];
    assert_eq!(materials["videos"].as_array().unwrap().len(), 1);
    assert_eq!(materials["audios"].as_array().unwrap().len(), 1);
    assert_eq!(materials["texts"].as_array().unwrap().len(), 1);
    assert_eq!(materials["speeds"].as_array().unwrap().len(), 2); // video + audio

    // Verify tracks
    let tracks = draft["tracks"].as_array().unwrap();
    assert_eq!(tracks.len(), 3);

    // Verify track ordering (sorted by render_index: video=0, audio=0, text=15000)
    assert_eq!(tracks[2]["type"], "text");

    // Verify video segment structure
    let video_segs = tracks[0]["segments"].as_array().unwrap();
    assert_eq!(video_segs.len(), 1);
    let seg = &video_segs[0];
    assert_eq!(seg["speed"], 1.0);
    assert_eq!(seg["volume"], 1.0);
    assert!(seg["hdr_settings"].is_object());

    // Verify audio segment structure
    let audio_segs = tracks[1]["segments"].as_array().unwrap();
    assert_eq!(audio_segs.len(), 1);
    assert!(audio_segs[0]["clip"].is_null());
    assert!(audio_segs[0]["hdr_settings"].is_null());

    // Verify text segment structure
    let text_segs = tracks[2]["segments"].as_array().unwrap();
    assert_eq!(text_segs.len(), 1);
    assert!(text_segs[0]["clip"].is_object());

    // Verify text material has content field
    let text_mats = materials["texts"].as_array().unwrap();
    assert!(text_mats[0]["content"].is_string());
}

#[test]
fn test_write_draft_to_directory() {
    let dir = tempfile::tempdir().expect("create temp dir");
    let output = camino::Utf8Path::from_path(dir.path())
        .unwrap()
        .join("test_draft");

    let video_mat = make_video_material("clip", 3_000_000);

    let project = Project {
        id: new_id(),
        name: "test_output".into(),
        canvas: Canvas::default(),
        maintrack_adsorb: true,
        tracks: vec![Track {
            id: new_id(),
            kind: TrackKind::Video,
            name: "v1".into(),
            render_index: 0,
            mute: false,
            clips: vec![make_video_clip(&video_mat, 0, 3_000_000)],
        }],
        video_materials: vec![video_mat],
        audio_materials: vec![],
        duration: 3_000_000,
    };

    jy_draft::writer::write_draft(&project, &output).expect("write should succeed");

    // Verify files exist
    assert!(std::fs::metadata(output.join("draft_content.json")).is_ok());
    assert!(std::fs::metadata(output.join("draft_meta_info.json")).is_ok());

    // Verify draft_content.json is valid JSON
    let content = std::fs::read_to_string(output.join("draft_content.json")).unwrap();
    let json: Value = serde_json::from_str(&content).unwrap();
    assert_eq!(json["duration"], 3_000_000);

    // Verify draft_meta_info.json
    let meta = std::fs::read_to_string(output.join("draft_meta_info.json")).unwrap();
    let meta_json: Value = serde_json::from_str(&meta).unwrap();
    assert_eq!(meta_json["draft_name"], "test_output");
}

#[test]
fn test_transform_conversion() {
    let t = Transform {
        x: 0.5,
        y: 0.5,
        ..Default::default()
    };
    assert!((t.to_jy_transform_x() - 0.0).abs() < 1e-10);
    assert!((t.to_jy_transform_y() - 0.0).abs() < 1e-10);

    let t2 = Transform {
        x: 0.75,
        y: 0.25,
        ..Default::default()
    };
    assert!((t2.to_jy_transform_x() - 0.5).abs() < 1e-10);
    assert!((t2.to_jy_transform_y() - (-0.5)).abs() < 1e-10);
}
