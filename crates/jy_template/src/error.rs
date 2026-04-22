use thiserror::Error;

#[derive(Debug, Error)]
pub enum TemplateError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("track not found")]
    TrackNotFound,

    #[error("ambiguous track selection")]
    AmbiguousTrack,

    #[error("segment index {index} out of range")]
    SegmentIndexOutOfRange { index: usize },

    #[error("material not found: {name}")]
    MaterialNotFound { name: String },

    #[error("ambiguous material match: {name}")]
    AmbiguousMaterial { name: String },

    #[error("material type mismatch")]
    MaterialTypeMismatch,

    #[error("extension failed")]
    ExtensionFailed,

    #[error("invalid text replacement: expected {expected}, got {actual}")]
    InvalidTextReplacement { expected: usize, actual: usize },

    #[error("unsupported draft structure: {0}")]
    UnsupportedStructure(String),
}
