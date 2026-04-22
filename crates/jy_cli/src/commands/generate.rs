use anyhow::Result;
use camino::Utf8Path;
use jy_draft::writer::write_draft;
use jy_schema::{AudioMaterialRef, Canvas, Project, Track, VideoMaterialRef};
use serde::Deserialize;
use uuid::Uuid;

/// `generate` 命令读取的 manifest 结构。
///
/// 这是一份“接近最终 schema、但对 CLI 更友好”的输入结构：
/// - `id` 和 `duration` 都允许省略
/// - `tracks / materials` 默认空数组
#[derive(Debug, Deserialize)]
struct ProjectManifest {
    id: Option<String>,
    name: String,
    canvas: Canvas,
    #[serde(default = "default_maintrack_adsorb")]
    maintrack_adsorb: bool,
    #[serde(default)]
    tracks: Vec<Track>,
    #[serde(default)]
    video_materials: Vec<VideoMaterialRef>,
    #[serde(default)]
    audio_materials: Vec<AudioMaterialRef>,
    duration: Option<u64>,
}

fn default_maintrack_adsorb() -> bool {
    true
}

/// 根据 manifest 直接生成剪映草稿。
pub fn run(project: &Utf8Path, output: &Utf8Path) -> Result<()> {
    let content = std::fs::read_to_string(project)?;
    let manifest: ProjectManifest = serde_json::from_str(&content)?;

    // 如果外部没有显式给出工程总时长，则根据所有片段的结束时间自动推导。
    let inferred_duration = manifest
        .tracks
        .iter()
        .flat_map(|track| track.clips.iter())
        .map(|clip| clip.target_timerange().end())
        .max()
        .unwrap_or(0);

    // 将 CLI 输入结构收敛为统一的 `Project`，再交给 draft writer。
    let project = Project {
        id: manifest
            .id
            .unwrap_or_else(|| Uuid::new_v4().as_simple().to_string()),
        name: manifest.name,
        canvas: manifest.canvas,
        maintrack_adsorb: manifest.maintrack_adsorb,
        tracks: manifest.tracks,
        video_materials: manifest.video_materials,
        audio_materials: manifest.audio_materials,
        duration: manifest.duration.unwrap_or(inferred_duration),
    };

    write_draft(&project, output)?;
    println!("Generated draft: {}", output);
    Ok(())
}
