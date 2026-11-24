//! The HTTP application: routes shared by `main` and the transport tests.
//!
//! @canonical .pi/architecture/modules/rigorix-server.md#http-surface
//!
//! Keeping the router here (rather than in `main`) means the end-to-end test
//! binds the **production** route table, not a hand-assembled lookalike. The
//! binary only composes state and serves it.

use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};

use crate::events::events_handler;
use crate::origin;
use crate::rpc;
use crate::state::ServerState;

/// Build the server's route table.
pub fn router(state: ServerState) -> Router {
    Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/rpc", post(rpc_handler).options(rpc_preflight))
        .route("/events", get(events_handler))
        // #915 / ADR-0001 D4: the optional MCP Streamable HTTP adapter over the
        // frozen catalog. Same governed dispatch + auth as `/rpc`.
        .route(
            "/mcp",
            post(crate::mcp_http::post_handler)
                .get(crate::mcp_http::get_handler)
                .delete(crate::mcp_http::delete_handler)
                .options(mcp_preflight),
        )
        .with_state(state)
}

/// `POST /rpc` — JSON-RPC 2.0 (single, batch, notification).
///
/// Always HTTP 200 for a well-formed envelope; notifications yield an empty
/// body. Malformed JSON yields a `-32700` error object.
///
/// #1650: the origin decision runs **before** the body is looked at. This route had none, so a page
/// on any origin could drive a free-mode executor on loopback — and without a JSON content type
/// requirement a browser could send that request as a *simple* request, which never preflights. The
/// decision is `crate::origin::decide`, the same one `/mcp` and `/events` call.
async fn rpc_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Some(refusal) = origin::refusal(origin::decide(&headers, true), "/rpc") {
        return refusal;
    }
    let value: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return Json(rpc::parse_error()).into_response(),
    };
    match rpc::handle_value(value, state.backend.as_ref()).await {
        Some(response) => Json(response).into_response(),
        None => StatusCode::OK.into_response(),
    }
}

/// `OPTIONS /rpc` — a browser preflight, refused explicitly.
///
/// A 405 here (which is what this route answered) reads as "wrong shape, try another" and leaves the
/// preflight-free simple-request path open. The refusal is a value, not a silence.
async fn rpc_preflight() -> Response {
    origin::preflight_refusal("/rpc")
}

/// `OPTIONS /mcp` — the same answer, from the same function.
async fn mcp_preflight() -> Response {
    origin::preflight_refusal("/mcp")
}
