use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum EventSystemCliEvent {
    Subscribed {
        event_type: String,
    },
    EventReceived {
        event_type: String,
        payload: serde_json::Value,
    },
    PublishSucceeded {
        event_type: String,
    },
    PublishFailed {
        event_type: String,
        error: String,
    },
}
