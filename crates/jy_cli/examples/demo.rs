use jy_draft::writer::write_draft;
use jy_schema::{
    AudioFade, BackgroundFillType, BackgroundFillingRef, Canvas, Clip, TextStyle, TimeRange,
    TrackKind, Transform, SEC,
};
use jy_timeline::builder::ProjectBuilder;
use jy_timeline::clip::{make_audio_clip, make_text_clip, make_video_clip};
use uuid::Uuid;

fn new_id() -> String {
    Uuid::new_v4().as_simple().to_string()
}

fn main() -> anyhow::Result<()> {
    // Tutorial asset directory
    let tutorial_dir =
        camino::Utf8PathBuf::from(std::env::var("TUTORIAL_DIR").unwrap_or_else(|_| {
            let demo_dir = std::env::current_dir().unwrap();
            format!(
                "{}\\readme_assets\\tutorial",
                demo_dir.parent().unwrap().display()
            )
        }));

    println!("Tutorial assets: {}", tutorial_dir);
    assert!(tutorial_dir.exists(), "Tutorial asset directory not found");

    // Output directory - user must set DRAFT_FOLDER env var
    let draft_folder = std::env::var("DRAFT_FOLDER").expect(
        "Please set DRAFT_FOLDER to your JianYing drafts folder (e.g. .../JianyingPro Drafts)",
    );
    let output_dir = camino::Utf8PathBuf::from(&draft_folder).join("demo");

    // Probe media files
    let video_path = tutorial_dir.join("video.mp4");
    let audio_path = tutorial_dir.join("audio.mp3");
    let gif_path = tutorial_dir.join("sticker.gif");

    println!("Probing video...");
    let video_mat = jy_media::material::create_video_material(&video_path, None)?;
    println!(
        "  duration: {:.3}s, {}x{}",
        video_mat.duration as f64 / SEC as f64,
        video_mat.width,
        video_mat.height
    );

    println!("Probing audio...");
    let audio_mat = jy_media::material::create_audio_material(&audio_path, None)?;
    println!("  duration: {:.3}s", audio_mat.duration as f64 / SEC as f64);

    println!("Probing gif...");
    let gif_mat = jy_media::material::create_video_material(&gif_path, None)?;
    println!(
        "  duration: {:.3}s, {}x{}",
        gif_mat.duration as f64 / SEC as f64,
        gif_mat.width,
        gif_mat.height
    );

    // Create audio segment (0-5s, volume 60%, 1s fade in)
    let audio_clip = make_audio_clip(&audio_mat, TimeRange::from_secs(0.0, 5.0), None, None, 0.6)?;
    let audio_clip = add_audio_fade(audio_clip, SEC, 0)?;

    // Create video segment (0-4.2s)
    let video_clip = make_video_clip(
        &video_mat,
        TimeRange::from_secs(0.0, 4.2),
        None,
        None,
        1.0,
        None,
    )?;

    // Create gif segment (starts right after video, duration = gif duration)
    let gif_start = (4.2 * SEC as f64) as u64;
    let gif_duration = gif_mat.duration;
    let gif_clip = make_video_clip(
        &gif_mat,
        TimeRange::new(gif_start, gif_duration),
        None,
        None,
        1.0,
        None,
    )?;
    // Add blur background filling to gif
    let gif_clip = add_background_blur(gif_clip, 0.0625)?;

    // Create text segment (matches video segment timing, positioned at bottom)
    let text_clip = make_text_clip(
        "据说pyJianYingDraft效果还不错?",
        TimeRange::from_secs(0.0, 4.2),
        Some(TextStyle {
            color: (1.0, 1.0, 0.0), // yellow
            ..Default::default()
        }),
        Some(Transform {
            y: 0.1, // transform_y = -0.8 → y = 0.5 + (-0.8)/2 = 0.1
            ..Default::default()
        }),
    );

    // Build project
    let project = ProjectBuilder::new("demo", Canvas::default())
        .maintrack_adsorb(true)
        .add_track(TrackKind::Audio, "audio1", 0)?
        .add_track(TrackKind::Video, "video1", 0)?
        .add_track(TrackKind::Text, "text1", 1)?
        .add_video_material(video_mat)
        .add_video_material(gif_mat)
        .add_audio_material(audio_mat)
        .add_clip_to_track("audio1", audio_clip)?
        .add_clip_to_track("video1", video_clip)?
        .add_clip_to_track("video1", gif_clip)?
        .add_clip_to_track("text1", text_clip)?
        .build();

    println!(
        "Project duration: {:.3}s",
        project.duration as f64 / SEC as f64
    );

    // Write draft
    write_draft(&project, &output_dir)?;
    println!("Draft written to: {}", output_dir);
    println!("Open JianYing and look for the 'demo' draft.");

    Ok(())
}

fn add_audio_fade(
    clip: Clip,
    in_duration: u64,
    out_duration: u64,
) -> Result<Clip, jy_schema::SchemaError> {
    match clip {
        Clip::Audio(mut ac) => {
            ac.fade = Some(AudioFade {
                id: new_id(),
                in_duration,
                out_duration,
            });
            Ok(Clip::Audio(ac))
        }
        Clip::Video(mut vc) => {
            vc.fade = Some(AudioFade {
                id: new_id(),
                in_duration,
                out_duration,
            });
            Ok(Clip::Video(vc))
        }
        _ => Ok(clip),
    }
}

fn add_background_blur(clip: Clip, blur: f64) -> Result<Clip, jy_schema::SchemaError> {
    match clip {
        Clip::Video(mut vc) => {
            vc.background_filling = Some(BackgroundFillingRef {
                id: new_id(),
                fill_type: BackgroundFillType::Blur,
                blur,
                color: String::new(),
            });
            Ok(Clip::Video(vc))
        }
        _ => Ok(clip),
    }
}
