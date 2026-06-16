use thiserror::Error;

#[derive(Debug, Error)]
pub enum StatePersistenceCliError {
    #[error("Failed to load state: {detail}")]
    LoadFailed { detail: String },
    #[error("Failed to save state: {detail}")]
    SaveFailed { detail: String },
    #[error("State not found: {id}")]
    NotFound { id: String },
    #[error("Internal error: {detail}")]
    Internal { detail: String },
}
