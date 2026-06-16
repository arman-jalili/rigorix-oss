use thiserror::Error;

#[derive(Debug, Error)]
pub enum EventSystemCliError {
    #[error("Failed to subscribe: {detail}")]
    SubscribeFailed { detail: String },
    #[error("Failed to publish event: {detail}")]
    PublishFailed { detail: String },
    #[error("Event not found: {event_type}")]
    NotFound { event_type: String },
    #[error("Internal error: {detail}")]
    Internal { detail: String },
}
