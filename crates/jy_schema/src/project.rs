use crate::canvas::Canvas;
use crate::material::{AudioMaterialRef, VideoMaterialRef};
use crate::track::Track;

/// 顶层工程对象。
///
/// 这是 `YingDraft` 内部最核心的数据结构，几乎所有命令最终都会先组装成
/// 一个 `Project`，再交给 `jy_draft` 转成剪映草稿 JSON。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub canvas: Canvas,
    pub maintrack_adsorb: bool,
    pub tracks: Vec<Track>,
    pub video_materials: Vec<VideoMaterialRef>,
    pub audio_materials: Vec<AudioMaterialRef>,
    /// 工程总时长，单位为微秒。
    ///
    /// 一般由 `ProjectBuilder` 根据所有片段的结束时间自动维护。
    pub duration: u64,
}
