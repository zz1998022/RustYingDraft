use camino::Utf8PathBuf;

/// The type of media material.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum MaterialKind {
    Video,
    Photo,
    Audio,
}

/// Crop settings defining the four corners of the visible area.
/// All values are in 0.0~1.0 range. Default = no crop.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct CropSettings {
    pub upper_left_x: f64,
    pub upper_left_y: f64,
    pub upper_right_x: f64,
    pub upper_right_y: f64,
    pub lower_left_x: f64,
    pub lower_left_y: f64,
    pub lower_right_x: f64,
    pub lower_right_y: f64,
}

impl Default for CropSettings {
    fn default() -> Self {
        Self {
            upper_left_x: 0.0,
            upper_left_y: 0.0,
            upper_right_x: 1.0,
            upper_right_y: 0.0,
            lower_left_x: 0.0,
            lower_left_y: 1.0,
            lower_right_x: 1.0,
            lower_right_y: 1.0,
        }
    }
}

/// Reference to a video or image material on disk.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct VideoMaterialRef {
    pub id: String,
    pub path: Utf8PathBuf,
    pub duration: u64,
    pub width: u32,
    pub height: u32,
    pub kind: MaterialKind,
    pub crop: CropSettings,
    pub name: String,
}

/// Reference to an audio material on disk.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct AudioMaterialRef {
    pub id: String,
    pub path: Utf8PathBuf,
    pub duration: u64,
    pub name: String,
}
