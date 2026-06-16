use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum StatePersistenceCliEvent {
    StateLoaded { session_id: String },
    StateSaved { session_id: String },
    StateDeleted { session_id: String },
    LoadFailed { session_id: String, error: String },
}
