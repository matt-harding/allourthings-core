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

    #[error("invalid input: {0}")]
    Validation(String),

    #[error("an item named \"{name}\" already exists (id {existing_id})")]
    DuplicateName { name: String, existing_id: String },

    #[error("an item with id {0} already exists")]
    DuplicateId(String),

    #[error("\"{query}\" matches more than one item ({}); use an id instead", ids.join(", "))]
    Ambiguous { query: String, ids: Vec<String> },

    #[error("attachment not found: {filename} on item {item_id}")]
    AttachmentNotFound { item_id: String, filename: String },
}
