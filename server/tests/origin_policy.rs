//! The HTTP front door's origin policy, asserted **per route on the production router** (#1650).
//!
//! # Why this file exists
//!
//! The origin guard was implemented once, in `mcp_http.rs`, and wired to `/mcp` only. `/rpc` had
//! none — so a page on any origin could drive a free-mode executor on loopback — and `/events` had
//! none either. Every existing test passed, because each test built the router it was testing: the
//! per-route suites were self-consistent and the *table* was never asserted.
//!
//! So this suite drives `rigorix_server::app::router` — the production table, the same one `main`
//! serves — and asks each mounted route the same question. A route that loses its guard fails here
//! even though its own handler still works.

#![allow(clippy::disallowed_types)] // the point is to speak HTTP to the real router

use async_trait::async_trait;
use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode, header};
use rigorix_mcp::host::error::HostError;
use rigorix_server::app;
use rigorix_server::backend::MethodBackend;
use rigorix_server::events::EventHub;
use rigorix_server::state::ServerState;
use serde_json::{Value, json};
use tower::ServiceExt;

struct EchoBackend;

#[async_trait]
impl MethodBackend for EchoBackend {
    async fn dispatch(&self, method: &str, params: Value) -> Result<Value, HostError> {
        Ok(json!({ "method": method, "params": params }))
    }
}

fn router() -> Router {
    let state = ServerState::new(Arc::new(EchoBackend), Arc::new(EventHub::new()));
    app::router(state)
}

use std::sync::Arc;

struct Sent {
    status: StatusCode,
    body: Value,
}

async fn send(request: Request<Body>) -> Sent {
    let response = router().oneshot(request).await.expect("router response");
    let status = response.status();
    let bytes = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let body = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    Sent { status, body }
}

fn post(
    path: &str,
    origin: Option<&str>,
    content_type: Option<&str>,
    body: Value,
) -> Request<Body> {
    let mut builder = Request::builder()
        .method("POST")
        .uri(path)
        .header(header::HOST, "127.0.0.1:3001");
    if let Some(origin) = origin {
        builder = builder.header(header::ORIGIN, origin);
    }
    if let Some(content_type) = content_type {
        builder = builder.header(header::CONTENT_TYPE, content_type);
    }
    builder.body(Body::from(body.to_string())).expect("request")
}

fn rpc_body() -> Value {
    json!({ "jsonrpc": "2.0", "id": 1, "method": "rigorix.system.version", "params": {} })
}

/// The audit's executed case, on the route that had no guard at all: a page on any origin, sending
/// the content type a browser can send **without a preflight**.
#[tokio::test]
async fn rpc_refuses_a_browser_simple_cross_origin_request() {
    let sent = send(post(
        "/rpc",
        Some("http://example.invalid"),
        Some("text/plain"),
        rpc_body(),
    ))
    .await;
    assert_eq!(
        sent.status,
        StatusCode::FORBIDDEN,
        "a cross-origin simple request must not reach the handler: {:?}",
        sent.body
    );
    assert_eq!(sent.body["error"], "origin_refused");
    let message = sent.body["message"].as_str().unwrap_or_default();
    assert!(
        message.contains("example.invalid"),
        "the refusal names the page's origin: {message}"
    );
}

/// The other half: a browser on loopback sending the same non-JSON shape is refused too, because the
/// clause is about the *shape* (no preflight) and not about which origin sent it.
#[tokio::test]
async fn rpc_refuses_a_browser_request_that_could_skip_the_preflight() {
    let sent = send(post(
        "/rpc",
        Some("http://localhost:5173"),
        Some("application/x-www-form-urlencoded"),
        rpc_body(),
    ))
    .await;
    assert_eq!(sent.status, StatusCode::FORBIDDEN, "{:?}", sent.body);
    assert_eq!(sent.body["error"], "origin_refused");
}

/// Naming the boundary the guard must not cross: **free mode is the documented quickstart**, and a
/// non-browser client sends no `Origin`. If this fails, the fix for #1650 broke the product.
#[tokio::test]
async fn rpc_still_serves_a_non_browser_client() {
    let sent = send(post("/rpc", None, Some("application/json"), rpc_body())).await;
    assert_eq!(
        sent.status,
        StatusCode::OK,
        "a non-browser client must still execute: {:?}",
        sent.body
    );
    assert_eq!(
        sent.body["result"]["name"], "rigorix-server",
        "the request must have reached the handler: {:?}",
        sent.body
    );
}

/// `OPTIONS` on a body-carrying route answers an **explicit refusal**, never the 405 this route used
/// to answer — a 405 leaves the preflight-free path open while looking like a shape error.
#[tokio::test]
async fn preflight_on_every_body_route_is_a_documented_refusal_not_a_405() {
    for path in ["/rpc", "/mcp"] {
        let request = Request::builder()
            .method("OPTIONS")
            .uri(path)
            .header(header::HOST, "127.0.0.1:3001")
            .header(header::ORIGIN, "http://example.invalid")
            .header(header::ACCESS_CONTROL_REQUEST_METHOD, "POST")
            .body(Body::empty())
            .expect("request");
        let sent = send(request).await;
        assert_eq!(
            sent.status,
            StatusCode::FORBIDDEN,
            "OPTIONS {path} must be a refusal, not 405: {:?}",
            sent.body
        );
        assert_eq!(sent.body["error"], "cors_not_granted", "on {path}");
        assert!(
            sent.body["message"]
                .as_str()
                .unwrap_or_default()
                .contains("preflight"),
            "the answer explains itself: {:?}",
            sent.body
        );
    }
}

/// The same question on `/mcp`, which had the guard before this change — kept so the move to one
/// decision is not a place where the guard could have been dropped.
#[tokio::test]
async fn mcp_refuses_a_cross_origin_request() {
    let sent = send(post(
        "/mcp",
        Some("http://example.invalid"),
        Some("application/json"),
        json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize", "params": {} }),
    ))
    .await;
    assert_eq!(sent.status, StatusCode::FORBIDDEN, "{:?}", sent.body);
    assert_eq!(sent.body["error"], "origin_refused");
}

/// `/events` had no origin check at all. Its unauthenticated **content** in free mode stays as
/// designed — what changes is that a page on another origin can no longer subscribe.
#[tokio::test]
async fn events_refuses_a_cross_origin_subscriber() {
    let request = Request::builder()
        .method("GET")
        .uri("/events")
        .header(header::HOST, "127.0.0.1:3001")
        .header(header::ORIGIN, "http://example.invalid")
        .body(Body::empty())
        .expect("request");
    let sent = send(request).await;
    assert_eq!(
        sent.status,
        StatusCode::FORBIDDEN,
        "a cross-origin page must not subscribe to the event stream: {:?}",
        sent.body
    );
    assert_eq!(sent.body["error"], "origin_refused");
}

/// Every mounted route is enumerated **from the router's own source**, so a route added later
/// without a decision fails this test rather than joining the table unguarded. This is the shape the
/// issue asks for; #1659 turns it into a table with declared policies.
#[test]
fn every_mounted_route_is_covered_by_this_suite() {
    let source = include_str!("../src/app.rs");
    let table = source
        .split("pub fn router(")
        .nth(1)
        .expect("the route table")
        .split(".with_state(")
        .next()
        .expect("the table body");
    // `.route(` may span lines — a line-based scan silently missed `/mcp`, which is the shape of a
    // test that reports "every route" while reading three of them. The first string literal after
    // each `.route(` is the path, wherever the newline falls.
    let mut routes: Vec<String> = Vec::new();
    let mut rest = table;
    while let Some(index) = rest.find(".route(") {
        rest = &rest[index + ".route(".len()..];
        let Some(quote) = rest.find('"') else { break };
        let after = &rest[quote + 1..];
        let Some(end) = after.find('"') else { break };
        routes.push(after[..end].to_string());
    }
    routes.sort();
    routes.dedup();
    assert_eq!(
        routes,
        vec![
            "/events".to_string(),
            "/health".to_string(),
            "/mcp".to_string(),
            "/rpc".to_string()
        ],
        "the route table changed — every new route needs the origin decision and a test here \
         (this suite covers: /rpc, /mcp, /events; /health is a constant with nothing to drive)"
    );
}
