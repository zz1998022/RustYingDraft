use thiserror::Error;

#[derive(Debug, Error)]
pub enum MediaError {
    #[error("file not found: {path}")]
    FileNotFound { path: String },

    #[error("ffprobe not found on PATH")]
    FfprobeNotFound,

    #[error("ffprobe failed: {0}")]
    FfprobeFailed(String),

    #[error("unsupported media format: {path}")]
    UnsupportedFormat { path: String },

    #[error("no audio stream found in: {path}")]
    NoAudioStream { path: String },

    #[error("no video stream found in: {path}")]
    NoVideoStream { path: String },

    #[error("JSON parse error: {0}")]
    JsonError(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),
}
