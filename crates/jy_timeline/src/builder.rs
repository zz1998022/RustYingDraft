use crate::error::TimelineError;
use jy_schema::{AudioMaterialRef, Canvas, Clip, Project, Track, TrackKind, VideoMaterialRef};
use uuid::Uuid;

/// `Project` 构建器。
///
/// 这层的职责是把“零散的轨道、素材、片段”组装成一个合法的 `Project`，
/// 并在过程中做一些最基础的约束校验：
///
/// - 轨道名不能重复
/// - 片段类型必须和轨道类型匹配
/// - 同一轨道上的片段不能重叠
/// - 自动维护工程总时长
pub struct ProjectBuilder {
    project: Project,
}

impl ProjectBuilder {
    /// 创建一个新的空工程。
    pub fn new(name: &str, canvas: Canvas) -> Self {
        Self {
            project: Project {
                id: Uuid::new_v4().as_simple().to_string(),
                name: name.to_string(),
                canvas,
                maintrack_adsorb: true,
                tracks: Vec::new(),
                video_materials: Vec::new(),
                audio_materials: Vec::new(),
                duration: 0,
            },
        }
    }

    /// 设置是否启用主轨吸附。
    pub fn maintrack_adsorb(mut self, adsorb: bool) -> Self {
        self.project.maintrack_adsorb = adsorb;
        self
    }

    /// 添加一条轨道。
    ///
    /// `render_index_offset` 用于在同类型轨道内控制层级，
    /// 最终会叠加到 `TrackKind` 自带的默认层级上。
    pub fn add_track(
        mut self,
        kind: TrackKind,
        name: &str,
        render_index_offset: i32,
    ) -> Result<Self, TimelineError> {
        if self.project.tracks.iter().any(|t| t.name == name) {
            return Err(TimelineError::DuplicateTrack {
                name: name.to_string(),
            });
        }
        let render_index = kind.default_render_index() + render_index_offset;
        let track = Track::new(
            Uuid::new_v4().as_simple().to_string(),
            kind,
            name.to_string(),
            render_index,
        );
        self.project.tracks.push(track);
        Ok(self)
    }

    /// 添加视频/图片素材。
    ///
    /// 当前按素材 ID 去重，不按路径去重。
    pub fn add_video_material(mut self, mat: VideoMaterialRef) -> Self {
        if !self.project.video_materials.iter().any(|m| m.id == mat.id) {
            self.project.video_materials.push(mat);
        }
        self
    }

    /// 添加音频素材。
    pub fn add_audio_material(mut self, mat: AudioMaterialRef) -> Self {
        if !self.project.audio_materials.iter().any(|m| m.id == mat.id) {
            self.project.audio_materials.push(mat);
        }
        self
    }

    /// 向指定轨道添加一个片段，并做合法性校验。
    pub fn add_clip_to_track(
        mut self,
        track_name: &str,
        clip: Clip,
    ) -> Result<Self, TimelineError> {
        let track = self
            .project
            .tracks
            .iter_mut()
            .find(|t| t.name == track_name)
            .ok_or_else(|| jy_schema::SchemaError::TrackNotFound {
                name: track_name.to_string(),
            })?;

        // 先检查片段类型是否能放进目标轨道。
        if !track.kind.accepts_clip(&clip) {
            return Err(jy_schema::SchemaError::ClipTypeMismatch {
                clip_type: match &clip {
                    Clip::Video(_) => "video",
                    Clip::Audio(_) => "audio",
                    Clip::Text(_) => "text",
                    Clip::Image(_) => "image",
                }
                .to_string(),
                track_kind: track.kind.to_str().to_string(),
            }
            .into());
        }

        // 再检查同一轨道内是否有时间重叠。
        let new_range = clip.target_timerange();
        for existing in &track.clips {
            if existing.target_timerange().overlaps(new_range) {
                return Err(jy_schema::SchemaError::SegmentOverlap {
                    start: new_range.start,
                    end: new_range.end(),
                }
                .into());
            }
        }

        // 最后顺带更新工程总时长。
        self.project.duration = self.project.duration.max(new_range.end());

        track.clips.push(clip);
        Ok(self)
    }

    /// 消费构建器并产出最终 `Project`。
    pub fn build(self) -> Project {
        self.project
    }
}
