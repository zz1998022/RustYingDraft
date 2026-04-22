use crate::keyframe::KeyframeList;
use crate::text_style::{TextBackground, TextBorder, TextShadow, TextStyle};
use crate::time::TimeRange;
use crate::transform::Transform;

// ---------------------------------------------------------------------------
// Speed & Fade
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Speed {
    pub id: String,
    pub speed: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioFade {
    pub id: String,
    pub in_duration: u64,
    pub out_duration: u64,
}

// ---------------------------------------------------------------------------
// Opaque references for effects/filters/etc. (MVP)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct EffectRef {
    pub id: String,
    pub effect_id: String,
    pub resource_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FilterRef {
    pub id: String,
    pub effect_id: String,
    pub resource_id: String,
    pub intensity: f64,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct MaskRef {
    pub id: String,
    pub name: String,
    pub resource_type: String,
    pub resource_id: String,
    pub aspect_ratio: f64,
    pub center_x: f64,
    pub center_y: f64,
    pub width: f64,
    pub height: f64,
    pub rotation: f64,
    pub invert: bool,
    pub feather: f64,
    pub round_corner: f64,
    pub position_info: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TransitionRef {
    pub id: String,
    pub name: String,
    pub effect_id: String,
    pub resource_id: String,
    pub duration: u64,
    pub is_overlap: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum BackgroundFillType {
    Blur,
    Color,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct BackgroundFillingRef {
    pub id: String,
    pub fill_type: BackgroundFillType,
    pub blur: f64,
    pub color: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimationRef {
    pub id: String,
    pub animations: Vec<AnimationItem>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AnimationItem {
    pub name: String,
    pub effect_id: String,
    pub animation_type: String,
    pub resource_id: String,
    pub start: u64,
    pub duration: u64,
    pub is_video_animation: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct FontRef {
    pub resource_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioEffectRef {
    pub id: String,
    pub name: String,
    pub resource_id: String,
    pub category_id: String,
    pub category_name: String,
    pub category_index: u32,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextBubbleRef {
    pub id: String,
    pub effect_id: String,
    pub resource_id: String,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextEffectRef {
    pub id: String,
    pub effect_id: String,
    pub resource_id: String,
}

// ---------------------------------------------------------------------------
// Clip variants
// ---------------------------------------------------------------------------

/// Top-level clip enum. Each variant carries its own data.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub enum Clip {
    Video(VideoClip),
    Audio(AudioClip),
    Text(TextClip),
    Image(ImageClip),
}

impl Clip {
    pub fn id(&self) -> &str {
        match self {
            Clip::Video(c) => &c.id,
            Clip::Audio(c) => &c.id,
            Clip::Text(c) => &c.id,
            Clip::Image(c) => &c.id,
        }
    }

    pub fn material_id(&self) -> &str {
        match self {
            Clip::Video(c) => &c.material_id,
            Clip::Audio(c) => &c.material_id,
            Clip::Text(c) => &c.material_id,
            Clip::Image(c) => &c.material_id,
        }
    }

    pub fn target_timerange(&self) -> &TimeRange {
        match self {
            Clip::Video(c) => &c.target_timerange,
            Clip::Audio(c) => &c.target_timerange,
            Clip::Text(c) => &c.target_timerange,
            Clip::Image(c) => &c.target_timerange,
        }
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VideoClip {
    pub id: String,
    pub material_id: String,
    pub target_timerange: TimeRange,
    pub source_timerange: Option<TimeRange>,
    pub speed: Speed,
    pub volume: f64,
    pub change_pitch: bool,
    pub transform: Transform,
    pub keyframes: Vec<KeyframeList>,
    pub fade: Option<AudioFade>,
    pub effects: Vec<EffectRef>,
    pub filters: Vec<FilterRef>,
    pub mask: Option<MaskRef>,
    pub transition: Option<TransitionRef>,
    pub background_filling: Option<BackgroundFillingRef>,
    pub animations: Option<AnimationRef>,
    pub mix_mode: Option<crate::track::MixModeRef>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioClip {
    pub id: String,
    pub material_id: String,
    pub target_timerange: TimeRange,
    pub source_timerange: Option<TimeRange>,
    pub speed: Speed,
    pub volume: f64,
    pub change_pitch: bool,
    pub keyframes: Vec<KeyframeList>,
    pub fade: Option<AudioFade>,
    pub effects: Vec<AudioEffectRef>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct TextClip {
    pub id: String,
    pub material_id: String,
    pub target_timerange: TimeRange,
    pub text: String,
    pub font: Option<FontRef>,
    pub style: TextStyle,
    pub transform: Transform,
    pub keyframes: Vec<KeyframeList>,
    pub border: Option<TextBorder>,
    pub background: Option<TextBackground>,
    pub shadow: Option<TextShadow>,
    pub animations: Option<AnimationRef>,
    pub bubble: Option<TextBubbleRef>,
    pub effect: Option<TextEffectRef>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ImageClip {
    pub id: String,
    pub material_id: String,
    pub target_timerange: TimeRange,
    pub source_timerange: Option<TimeRange>,
    pub speed: Speed,
    pub transform: Transform,
    pub keyframes: Vec<KeyframeList>,
    pub background_filling: Option<BackgroundFillingRef>,
    pub animations: Option<AnimationRef>,
}
