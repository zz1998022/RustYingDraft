use jy_schema::TrackKind;
use serde::Deserialize;

use crate::subtitle_style::SimpleSubtitleStyle;

#[derive(Debug, Deserialize)]
pub(crate) struct PipelineSpec {
    pub(crate) concat_file: Option<String>,
    pub(crate) subtitle_file: Option<String>,
    #[serde(default)]
    pub(crate) narration_files: Vec<String>,
    pub(crate) tracks: Option<Vec<PipelineTrackSpec>>,
    pub(crate) audio_file: Option<String>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PipelineTrackSpec {
    pub(crate) kind: PipelineTrackKind,
    pub(crate) name: String,
    #[serde(default)]
    pub(crate) clips: Vec<PipelineClipSpec>,
    pub(crate) subtitle_file: Option<String>,
    pub(crate) style: Option<SimpleSubtitleStyle>,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "snake_case")]
pub(crate) enum PipelineTrackKind {
    Video,
    Audio,
    Text,
}

impl PipelineTrackKind {
    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Video => "video",
            Self::Audio => "audio",
            Self::Text => "text",
        }
    }
}

impl From<PipelineTrackKind> for TrackKind {
    fn from(value: PipelineTrackKind) -> Self {
        match value {
            PipelineTrackKind::Video => TrackKind::Video,
            PipelineTrackKind::Audio => TrackKind::Audio,
            PipelineTrackKind::Text => TrackKind::Text,
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct PipelineClipSpec {
    pub(crate) path: String,
    pub(crate) start: f64,
    pub(crate) end: Option<f64>,
    pub(crate) volume: Option<f64>,
}

#[derive(Debug, Deserialize)]
pub(crate) struct PipelineAudioStyle {
    #[serde(default = "default_pipeline_volume")]
    pub(crate) video_volume: f64,
    #[serde(default = "default_pipeline_volume")]
    pub(crate) narration_volume: f64,
    pub(crate) audio_volume: Option<f64>,
}

impl Default for PipelineAudioStyle {
    fn default() -> Self {
        Self {
            video_volume: default_pipeline_volume(),
            narration_volume: default_pipeline_volume(),
            audio_volume: None,
        }
    }
}

pub(crate) fn default_pipeline_volume() -> f64 {
    1.0
}
