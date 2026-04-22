use thiserror::Error;

#[derive(Debug, Error)]
pub enum DraftError {
    #[error("template JSON parse error: {0}")]
    TemplateParse(#[from] serde_json::Error),

    #[error("IO error: {0}")]
    IoError(#[from] std::io::Error),

    #[error("schema error: {0}")]
    Schema(#[from] jy_schema::SchemaError),
}
