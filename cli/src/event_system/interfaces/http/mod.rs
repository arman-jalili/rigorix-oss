use crate::event_system::application::dto::{ListEventsOutput, PublishOutput, SubscribeOutput};
use serde::{Deserialize, Serialize};

pub const API_BASE_PATH: &str = "/api/v1/cli/events";
pub const LIST_PATH: &str = "/api/v1/cli/events";
pub const LIST_METHOD: &str = "GET";
pub const SUBSCRIBE_PATH: &str = "/api/v1/cli/events/subscribe";
pub const SUBSCRIBE_METHOD: &str = "POST";
pub const PUBLISH_PATH: &str = "/api/v1/cli/events/publish";
pub const PUBLISH_METHOD: &str = "POST";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeApiRequest {
    pub event_type: String,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubscribeApiResponse {
    pub subscribed: bool,
}
impl From<SubscribeOutput> for SubscribeApiResponse {
    fn from(o: SubscribeOutput) -> Self {
        Self {
            subscribed: o.subscribed,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishApiRequest {
    pub event_type: String,
    pub payload: serde_json::Value,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PublishApiResponse {
    pub published: bool,
}
impl From<PublishOutput> for PublishApiResponse {
    fn from(o: PublishOutput) -> Self {
        Self {
            published: o.published,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListEventsApiResponse {
    pub event_types: Vec<String>,
    pub total: u32,
}
impl From<ListEventsOutput> for ListEventsApiResponse {
    fn from(o: ListEventsOutput) -> Self {
        Self {
            event_types: o.event_types,
            total: o.total,
        }
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
    pub const SUBSCRIBE_FAILED: &str = "EVENT_SUBSCRIBE_FAILED";
    pub const PUBLISH_FAILED: &str = "EVENT_PUBLISH_FAILED";
    pub const NOT_FOUND: &str = "EVENT_NOT_FOUND";
    pub const INTERNAL_ERROR: &str = "EVENT_INTERNAL_ERROR";
}
pub mod status_codes {
    pub const SUBSCRIBE_FAILED: u16 = 422;
    pub const PUBLISH_FAILED: u16 = 500;
    pub const NOT_FOUND: u16 = 404;
    pub const INTERNAL_ERROR: u16 = 500;
}
