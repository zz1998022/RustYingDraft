use crate::clip::Clip;

/// 轨道类型。
///
/// 每种轨道都有一个默认的 `render_index` 基准值，用来控制在剪映中的层级。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum TrackKind {
    Video,
    Audio,
    Text,
    Effect,
    Filter,
    Sticker,
}

impl TrackKind {
    /// 返回该轨道类型的默认层级基准值。
    pub fn default_render_index(&self) -> i32 {
        match self {
            Self::Video => 0,
            Self::Audio => 0,
            Self::Effect => 10000,
            Self::Filter => 11000,
            Self::Sticker => 14000,
            Self::Text => 15000,
        }
    }

    /// 返回剪映 JSON 中使用的轨道类型字符串。
    pub fn to_str(&self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Text => "text",
            Self::Effect => "effect",
            Self::Filter => "filter",
            Self::Sticker => "sticker",
        }
    }

    /// 判断当前轨道是否接收某种片段类型。
    ///
    /// 例如：
    /// - 视频轨可以放 `Video` 和 `Image`
    /// - 音频轨只能放 `Audio`
    /// - 文本轨只能放 `Text`
    pub fn accepts_clip(&self, clip: &Clip) -> bool {
        matches!(
            (self, clip),
            (Self::Video, Clip::Video(_) | Clip::Image(_))
                | (Self::Audio, Clip::Audio(_))
                | (Self::Text, Clip::Text(_))
                | (Self::Effect, Clip::Video(_))
                | (Self::Filter, Clip::Video(_))
        )
    }
}

/// 混合模式引用。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MixModeRef {
    pub id: String,
    pub effect_id: String,
    pub resource_id: String,
    pub name: String,
}

/// 一条轨道。
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Track {
    pub id: String,
    pub kind: TrackKind,
    pub name: String,
    pub render_index: i32,
    pub mute: bool,
    pub clips: Vec<Clip>,
}

impl Track {
    /// 创建一条空轨道。
    pub fn new(id: String, kind: TrackKind, name: String, render_index: i32) -> Self {
        Self {
            id,
            kind,
            name,
            render_index,
            mute: false,
            clips: Vec::new(),
        }
    }
}
