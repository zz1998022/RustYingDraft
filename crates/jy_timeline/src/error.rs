use thiserror::Error;

#[derive(Debug, Error)]
pub enum TimelineError {
    #[error("schema error: {0}")]
    Schema(#[from] jy_schema::SchemaError),

    #[error("track '{name}' already exists")]
    DuplicateTrack { name: String },

    #[error("no tracks available for clip type '{clip_type}'")]
    NoMatchingTrack { clip_type: String },

    #[error("invalid source timerange: duration exceeds material")]
    InvalidSourceRange,
}
