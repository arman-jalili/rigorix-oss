use crate::state_persistence::application::dto::{
    DeleteStateOutput, ListStatesOutput, LoadStateOutput,
};
use serde::{Deserialize, Serialize};

pub const API_BASE_PATH: &str = "/api/v1/cli/state";
pub const LIST_PATH: &str = "/api/v1/cli/state";
pub const LIST_METHOD: &str = "GET";
pub const LOAD_PATH: &str = "/api/v1/cli/state/{id}";
pub const LOAD_METHOD: &str = "GET";
pub const DELETE_PATH: &str = "/api/v1/cli/state/{id}";
pub const DELETE_METHOD: &str = "DELETE";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListStatesApiResponse {
    pub sessions: Vec<String>,
    pub total: u32,
}
impl From<ListStatesOutput> for ListStatesApiResponse {
    fn from(o: ListStatesOutput) -> Self {
        Self {
            sessions: o.sessions,
            total: o.total,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadStateApiResponse {
    pub session_id: String,
    pub data: serde_json::Value,
}
impl From<LoadStateOutput> for LoadStateApiResponse {
    fn from(o: LoadStateOutput) -> Self {
        Self {
            session_id: o.session_id,
            data: o.data,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteStateApiResponse {
    pub deleted: bool,
}
impl From<DeleteStateOutput> for DeleteStateApiResponse {
    fn from(o: DeleteStateOutput) -> Self {
        Self { deleted: o.deleted }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliApiErrorResponse {
    pub status: u16,
    pub code: String,
    pub message: String,
    pub details: Option<serde_json::Value>,
    pub request_id: Option<String>,
}
pub mod error_codes {
    pub const NOT_FOUND: &str = "STATE_NOT_FOUND";
    pub const LOAD_FAILED: &str = "STATE_LOAD_FAILED";
    pub const INTERNAL_ERROR: &str = "STATE_INTERNAL_ERROR";
}
pub mod status_codes {
    pub const NOT_FOUND: u16 = 404;
    pub const LOAD_FAILED: u16 = 500;
    pub const INTERNAL_ERROR: u16 = 500;
}
