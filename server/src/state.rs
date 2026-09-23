//! Server state (#888 OSS-C2).
//!
//! Bundles the catalog [`MethodBackend`] and the SSE [`EventHub`] so the axum
//! handlers share one `Clone` state value.

use std::sync::Arc;

use crate::backend::MethodBackend;
use crate::events::EventHub;

/// Shared axum state.
#[derive(Clone)]
pub struct ServerState {
    /// Catalog-method executor.
    pub backend: Arc<dyn MethodBackend>,
    /// In-process push hub backing `GET /events`.
    pub events: Arc<EventHub>,
}

impl ServerState {
    /// Create state from a backend and an event hub.
    pub fn new(backend: Arc<dyn MethodBackend>, events: Arc<EventHub>) -> Self {
        Self { backend, events }
    }
}
