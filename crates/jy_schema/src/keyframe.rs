/// Properties that can be animated via keyframes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, serde::Serialize, serde::Deserialize)]
pub enum KeyframeProperty {
    PositionX,
    PositionY,
    Rotation,
    ScaleX,
    ScaleY,
    UniformScale,
    Alpha,
    Saturation,
    Contrast,
    Brightness,
    Volume,
}

impl KeyframeProperty {
    /// Convert to JianYing's internal property ID string.
    pub fn to_jianying_id(&self) -> &'static str {
        match self {
            Self::PositionX => "KFTypePositionX",
            Self::PositionY => "KFTypePositionY",
            Self::Rotation => "KFTypeRotation",
            Self::ScaleX => "KFTypeScaleX",
            Self::ScaleY => "KFTypeScaleY",
            Self::UniformScale => "UNIFORM_SCALE",
            Self::Alpha => "KFTypeAlpha",
            Self::Saturation => "KFTypeSaturation",
            Self::Contrast => "KFTypeContrast",
            Self::Brightness => "KFTypeBrightness",
            Self::Volume => "KFTypeVolume",
        }
    }
}

/// A single keyframe point.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Keyframe {
    pub id: String,
    /// Time offset from segment start, in microseconds.
    pub time_offset: u64,
    pub value: f64,
}

/// A list of keyframes for a single property, sorted by time_offset.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct KeyframeList {
    pub id: String,
    pub property: KeyframeProperty,
    pub keyframes: Vec<Keyframe>,
}

impl KeyframeList {
    /// Add a keyframe, keeping the list sorted by time_offset.
    pub fn add(&mut self, time_offset: u64, value: f64, id: String) {
        let kf = Keyframe {
            id,
            time_offset,
            value,
        };
        let pos = self
            .keyframes
            .iter()
            .position(|k| k.time_offset > time_offset)
            .unwrap_or(self.keyframes.len());
        self.keyframes.insert(pos, kf);
    }
}
