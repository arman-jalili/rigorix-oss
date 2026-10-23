//! The HTTP application: routes shared by `main` and the transport tests.
//!
//! @canonical .pi/architecture/modules/rigorix-server.md#http-surface
//!
//! Keeping the router here (rather than in `main`) means the end-to-end test
//! binds the **production** route table, not a hand-assembled lookalike. The
//! binary only composes state and serves it.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::events::events_handler;
use crate::rpc;
use crate::state::ServerState;

/// Build the server's route table.
pub fn router(state: ServerState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/rpc", post(rpc_handler))
        .route("/events", get(events_handler))
        // #915 / ADR-0001 D4: the optional MCP Streamable HTTP adapter over the
        // frozen catalog. Same governed dispatch + auth as `/rpc`.
        .route(
            "/mcp",
            post(crate::mcp_http::post_handler)
                .get(crate::mcp_http::get_handler)
                .delete(crate::mcp_http::delete_handler),
        )
        .with_state(state)
}

/// `POST /rpc` — JSON-RPC 2.0 (single, batch, notification).
///
/// Always HTTP 200 for a well-formed envelope; notifications yield an empty
/// body. Malformed JSON yields a `-32700` error object.
async fn rpc_handler(State(state): State<ServerState>, body: Bytes) -> Response {
    let value: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return Json(rpc::parse_error()).into_response(),
    };
    match rpc::handle_value(value, state.backend.as_ref()).await {
        Some(response) => Json(response).into_response(),
        None => StatusCode::OK.into_response(),
    }
}
