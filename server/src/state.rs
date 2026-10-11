//! Server state (#888 OSS-C2).
//!
//! Bundles the catalog [`MethodBackend`] and the SSE [`EventHub`] so the axum
//! handlers share one `Clone` state value.

use std::sync::Arc;

use crate::backend::MethodBackend;
use crate::events::EventHub;
use crate::mcp_http::McpSessions;

/// Shared axum state.
#[derive(Clone)]
pub struct ServerState {
    /// Catalog-method executor.
    pub backend: Arc<dyn MethodBackend>,
    /// In-process push hub backing `GET /events`.
    pub events: Arc<EventHub>,
    /// MCP Streamable HTTP session registry backing `POST /mcp`.
    pub mcp_sessions: Arc<McpSessions>,
}

impl ServerState {
    /// Create state from a backend and an event hub.
    pub fn new(backend: Arc<dyn MethodBackend>, events: Arc<EventHub>) -> Self {
        Self {
            backend,
            events,
            mcp_sessions: Arc::new(McpSessions::default()),
        }
    }
}
