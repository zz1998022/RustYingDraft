use anyhow::Result;
use camino::Utf8PathBuf;
use jy_draft::writer::write_draft;
use jy_schema::{Canvas, TextStyle, TimeRange, TrackKind, Transform, VideoMaterialRef, SEC};
use jy_timeline::builder::ProjectBuilder;
use jy_timeline::clip::{make_image_clip, make_text_clip, make_video_clip};

fn main() -> Result<()> {
    let clips_dir = camino::Utf8PathBuf::from("D:/Outplayed/Outplayed/Valorant");
    let draft_folder =
        std::env::var("DRAFT_FOLDER").unwrap_or_else(|_| "D:/JianyingPro Drafts".into());
    let output_dir = camino::Utf8PathBuf::from(&draft_folder).join("valorant_montage");

    // Collect all mp4 files
    let mut video_paths: Vec<Utf8PathBuf> = Vec::new();
    for entry in std::fs::read_dir(&clips_dir)? {
        let entry = entry?;
        let sub_dir = entry.path();
        if sub_dir.is_dir() {
            for sub_entry in std::fs::read_dir(&sub_dir)? {
                let sub_entry = sub_entry?;
                let path = sub_entry.path();
                if let Some(ext) = path.extension() {
                    if ext.to_ascii_lowercase() == "mp4" {
                        video_paths.push(Utf8PathBuf::from_path_buf(path).unwrap());
                    }
                }
            }
        }
    }
    video_paths.sort();

    println!("Found {} clips", video_paths.len());

    // Probe all videos
    let mut materials: Vec<VideoMaterialRef> = Vec::new();
    for path in &video_paths {
        print!("  Probing {} ... ", path.file_name().unwrap_or(""));
        let mat = jy_media::material::create_video_material(path, None)?;
        println!(
            "{:.1}s ({}x{})",
            mat.duration as f64 / SEC as f64,
            mat.width,
            mat.height
        );
        materials.push(mat);
    }

    // Probe watermark image
    let watermark_path = camino::Utf8PathBuf::from(r"C:\Users\10740\Desktop\images.jpg");
    let watermark_mat = jy_media::material::create_video_material(&watermark_path, None)?;
    println!(
        "  Watermark: {}x{}",
        watermark_mat.width, watermark_mat.height
    );

    let clip_duration = 5 * SEC;
    let total_duration = (materials.len() as u64) * clip_duration;

    let mut builder = ProjectBuilder::new("valorant_montage", Canvas::new(1920, 1080, 60))
        .maintrack_adsorb(true)
        .add_track(TrackKind::Video, "main", 0)?
        .add_track(TrackKind::Video, "watermark", 1)?
        .add_track(TrackKind::Text, "subtitle", 2)?;

    // Register all materials
    for mat in &materials {
        builder = builder.add_video_material(mat.clone());
    }
    builder = builder.add_video_material(watermark_mat.clone());

    // Add video clips sequentially
    let subtitles = [
        "ACE!",
        "Nice shot!",
        "EZ Clap",
        "Headshot machine",
        "No scope no hope",
        "One tap wonder",
        "Clean ace",
        "Unreal flick",
        "GG WP",
    ];

    for (i, mat) in materials.iter().enumerate() {
        let start = i as u64 * clip_duration;
        let actual_duration = mat.duration.min(clip_duration);

        let clip = make_video_clip(
            mat,
            TimeRange::new(start, actual_duration),
            Some(TimeRange::new(0, actual_duration)),
            None,
            1.0,
            None,
        )?;
        builder = builder.add_clip_to_track("main", clip)?;

        let text = subtitles.get(i).unwrap_or(&"Nice!");
        let text_clip = make_text_clip(
            text,
            TimeRange::new(start, actual_duration),
            Some(TextStyle {
                size: 10.0,
                bold: true,
                color: (1.0, 1.0, 1.0),
                ..Default::default()
            }),
            Some(Transform {
                y: 0.15,
                opacity: 0.9,
                ..Default::default()
            }),
        );
        builder = builder.add_clip_to_track("subtitle", text_clip)?;
    }

    // Watermark: spans the entire video, positioned at top-right corner
    let watermark_clip = make_image_clip(
        &watermark_mat,
        TimeRange::new(0, total_duration),
        Some(Transform {
            x: 0.85,       // right side
            y: 0.15,       // top area
            scale_x: 0.25, // small size
            scale_y: 0.25,
            opacity: 0.7,
            uniform_scale: true,
            ..Default::default()
        }),
    );
    builder = builder.add_clip_to_track("watermark", watermark_clip)?;

    let project = builder.build();
    println!(
        "\nProject duration: {:.1}s ({} clips + watermark)",
        project.duration as f64 / SEC as f64,
        materials.len()
    );

    write_draft(&project, &output_dir)?;
    println!("Draft written to: {}", output_dir);

    Ok(())
}
