use thiserror::Error;

#[derive(Debug, Error)]
pub enum Error {
    #[error("item not found: {0}")]
    NotFound(String),

    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    #[error("json error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("invalid filename: {0}")]
    InvalidFilename(String),

    #[error("cache error: {0}")]
    Cache(#[from] rusqlite::Error),
}
