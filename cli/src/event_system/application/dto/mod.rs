use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeInput {
    pub event_type: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeOutput {
    pub subscribed: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishInput {
    pub event_type: String,
    pub payload: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishOutput {
    pub published: bool,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListEventsOutput {
    pub event_types: Vec<String>,
    pub total: u32,
}
