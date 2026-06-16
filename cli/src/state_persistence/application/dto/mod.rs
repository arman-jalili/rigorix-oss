use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadStateInput {
    pub session_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadStateOutput {
    pub session_id: String,
    pub data: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListStatesOutput {
    pub sessions: Vec<String>,
    pub total: u32,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteStateInput {
    pub session_id: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeleteStateOutput {
    pub deleted: bool,
}
