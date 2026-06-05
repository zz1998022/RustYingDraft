use serde::Deserialize;

use crate::asset::AssetKind;
use crate::pipeline::spec::{PipelineAudioStyle, PipelineSpec};
use crate::subtitle_style::SimpleSubtitleStyle;

#[derive(Debug, Deserialize)]
pub(crate) struct BundleManifest {
    #[serde(default)]
    #[serde(rename = "bundle_version")]
    pub(crate) _bundle_version: u32,
    #[serde(default)]
    pub(crate) bundle_type: BundleType,
    pub(crate) project_id: Option<String>,
    pub(crate) project_name: Option<String>,
    pub(crate) timeline_file: Option<String>,
    pub(crate) assets_dir: Option<String>,
    pub(crate) draft_dir: Option<String>,
    #[serde(default)]
    pub(crate) match_key: DraftMatchKey,
    #[serde(default)]
    pub(crate) assets: Vec<DraftAssetBinding>,
    pub(crate) pipeline: Option<PipelineSpec>,
    #[serde(default)]
    pub(crate) subtitle_style: SimpleSubtitleStyle,
    #[serde(default)]
    pub(crate) audio_style: PipelineAudioStyle,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum BundleType {
    #[default]
    TimelinePackage,
    DraftPackage,
    SimpleTimelinePackage,
    PipelinePackage,
}

#[derive(Debug, Clone, Copy, Deserialize, PartialEq, Eq, Default)]
#[serde(rename_all = "snake_case")]
pub(crate) enum DraftMatchKey {
    #[default]
    Name,
}

#[derive(Debug, Deserialize)]
pub(crate) struct DraftAssetBinding {
    pub(crate) kind: AssetKind,
    pub(crate) match_value: String,
    pub(crate) relative_path: String,
    #[serde(rename = "name")]
    pub(crate) _name: Option<String>,
}
