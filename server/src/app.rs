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
use crate::route_policy;
use crate::rpc;
use crate::state::ServerState;

/// Build the server's route table **from [`route_policy::ROUTES`]** (#1659).
///
/// The table is the router's input, not a description of it: mounting a route without a row is not
/// expressible, because there is no second place to add one. `server/tests/route_policy.rs` asserts
/// each row's declared policy against the behaviour, and prints the row when it fails.
pub fn router(state: ServerState) -> Router {
    let mut router = Router::new();
    for route in route_policy::ROUTES {
        router = match route.path {
            "/health" => router.route(route.path, get(|| async { "ok" })),
            "/rpc" => router.route(route.path, post(rpc_handler).options(rpc_preflight)),
            "/events" => router.route(route.path, get(events_handler)),
            "/mcp" => router.route(
                route.path,
                post(crate::mcp_http::post_handler)
                    .get(crate::mcp_http::get_handler)
                    .delete(crate::mcp_http::delete_handler)
                    .options(mcp_preflight),
            ),
            // A row with no handler is a declaration without an implementation — a route that exists
            // in the table and 404s, which no test would describe as missing. It is answered, not
            // panicked: a panic in a router takes down a task (or, with the default panic strategy,
            // a connection) for a defect that is a *build* property, and the table test already
            // fails on it loudly. The 501 names the route and says what is missing (#1659).
            other => router.route(
                other,
                axum::routing::any(move || async move {
                    (
                        StatusCode::NOT_IMPLEMENTED,
                        format!(
                            "{other} is declared in route_policy::ROUTES and app::router has no                              handler for it — a route must be mounted *and* given a policy (#1659)"
                        ),
                    )
                }),
            ),
        };
    }
    router.with_state(state)
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
    // The policy comes from the row this handler was mounted by, so a route cannot declare one thing
    // and enforce another (#1659).
    if let Some(refusal) = route_policy::policy_for("/rpc").and_then(|p| p.enforce_origin(&headers))
    {
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
