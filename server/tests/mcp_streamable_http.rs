//! MCP Streamable HTTP integration (#915, ADR-0001 D4/D6/D10).
//!
//! Drives the real axum `/mcp` router in-process (no live engine) and proves:
//! the streamable-HTTP session lifecycle, catalog-parity of `tools/list`, the
//! `tools/call` → catalog-method mapping, the `/rpc` auth gate applied to MCP,
//! the SSE-per-request path, and that no raw-step/enterprise escape hatch is
//! exposed.

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use async_trait::async_trait;
use axum::Router;
use axum::body::Body;
use axum::http::{HeaderMap, Request, StatusCode, header};
use axum::routing::post;
use serde_json::{Value, json};
use tower::ServiceExt;

use rigorix_mcp::host::error::HostError;
use rigorix_server::backend::MethodBackend;
use rigorix_server::catalog::{AuthLevel, CATALOG};
use rigorix_server::events::EventHub;
use rigorix_server::mcp_http;
use rigorix_server::state::ServerState;

/// Echoes the dispatched method + params; optionally models an IdP-gated host.
struct StubBackend {
    auth_configured: bool,
    authenticated: bool,
    dispatched: AtomicBool,
}

impl StubBackend {
    fn open() -> Self {
        Self {
            auth_configured: false,
            authenticated: false,
            dispatched: AtomicBool::new(false),
        }
    }

    fn gated_unattested() -> Self {
        Self {
            auth_configured: true,
            authenticated: false,
            dispatched: AtomicBool::new(false),
        }
    }
}

#[async_trait]
impl MethodBackend for StubBackend {
    async fn dispatch(&self, method: &str, params: Value) -> Result<Value, HostError> {
        self.dispatched.store(true, Ordering::SeqCst);
        Ok(json!({ "method": method, "params": params }))
    }

    async fn auth_configured(&self) -> bool {
        self.auth_configured
    }

    async fn is_authenticated(&self) -> bool {
        self.authenticated
    }
}

fn router(backend: Arc<dyn MethodBackend>) -> Router {
    let state = ServerState::new(backend, Arc::new(EventHub::new()));
    Router::new()
        .route(
            "/mcp",
            post(mcp_http::post_handler)
                .get(mcp_http::get_handler)
                .delete(mcp_http::delete_handler),
        )
        .with_state(state)
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, HeaderMap, Value) {
    let response = app.clone().oneshot(request).await.expect("router response");
    let status = response.status();
    let headers = response.headers().clone();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let value = if body.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&body).unwrap_or(Value::Null)
    };
    (status, headers, value)
}

fn post_json(body: Value) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri("/mcp")
        .header(header::CONTENT_TYPE, "application/json")
        .header(header::ACCEPT, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

async fn init_session(app: &Router) -> String {
    let (status, headers, body) = send(
        app,
        post_json(json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": { "protocolVersion": "2025-06-18", "clientInfo": { "name": "gui" } }
        })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["protocolVersion"], "2025-06-18");
    headers
        .get(mcp_http::SESSION_HEADER)
        .and_then(|value| value.to_str().ok())
        .expect("initialize issues a session id")
        .to_string()
}

fn with_session(mut request: Request<Body>, session: &str) -> Request<Body> {
    request.headers_mut().insert(
        mcp_http::SESSION_HEADER,
        session.parse().expect("session header"),
    );
    request
}

// ---------------------------------------------------------------------------
// Catalog parity / governance
// ---------------------------------------------------------------------------

#[test]
fn exposed_tools_are_exactly_the_catalog_governed_subset() {
    let tools = mcp_http::tools_list();
    let exposed: Vec<&str> = tools
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();

    // The governed subset: catalog entries with an MCP tool, non-admin.
    let governed: Vec<&str> = CATALOG
        .iter()
        .filter(|entry| entry.mcp_tool.is_some() && entry.auth != AuthLevel::Admin)
        .filter_map(|entry| entry.mcp_tool)
        .collect();

    assert_eq!(exposed, governed);
    // No admin / host-only / enterprise tool, and no raw-step surface.
    for forbidden in [
        "rigorix_policy_bundle",
        "rigorix_enterprise_call",
        "rigorix_enterprise_health",
        "rigorix_step",
    ] {
        assert!(!exposed.contains(&forbidden), "must not expose {forbidden}");
    }
    assert!(
        exposed.iter().all(|name| !name.contains("raw")),
        "no raw-step escape hatch"
    );
}

#[tokio::test]
async fn tools_list_over_http_matches_the_governed_subset() {
    let app = router(Arc::new(StubBackend::open()));
    let session = init_session(&app).await;
    let (status, _, body) = send(
        &app,
        with_session(
            post_json(json!({ "jsonrpc": "2.0", "id": 2, "method": "tools/list" })),
            &session,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let names: Vec<&str> = body["result"]["tools"]
        .as_array()
        .expect("tools array")
        .iter()
        .filter_map(|tool| tool["name"].as_str())
        .collect();
    assert_eq!(names.len(), 17);
    assert!(names.contains(&"rigorix_plan"));
    assert!(!names.contains(&"rigorix_enterprise_call"));
}

// ---------------------------------------------------------------------------
// tools/call mapping
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tools_call_maps_to_its_catalog_method() {
    let app = router(Arc::new(StubBackend::open()));
    let session = init_session(&app).await;
    let (status, _, body) = send(
        &app,
        with_session(
            post_json(json!({
                "jsonrpc": "2.0",
                "id": 3,
                "method": "tools/call",
                "params": { "name": "rigorix_plan", "arguments": { "intent": "ship" } }
            })),
            &session,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["isError"], false);
    let text = body["result"]["content"][0]["text"]
        .as_str()
        .expect("text content");
    let inner: Value = serde_json::from_str(text).expect("inner json");
    assert_eq!(inner["method"], "rigorix.plan");
    assert_eq!(inner["params"]["intent"], "ship");
}

#[tokio::test]
async fn unknown_or_enterprise_tool_never_dispatches() {
    let backend = Arc::new(StubBackend::open());
    let app = router(backend.clone());
    let session = init_session(&app).await;
    let (status, _, body) = send(
        &app,
        with_session(
            post_json(json!({
                "jsonrpc": "2.0",
                "id": 4,
                "method": "tools/call",
                "params": { "name": "rigorix_enterprise_call", "arguments": {} }
            })),
            &session,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["isError"], true);
    assert!(
        !backend.dispatched.load(Ordering::SeqCst),
        "an ungoverned tool must never reach dispatch"
    );
}

// ---------------------------------------------------------------------------
// Auth parity with POST /rpc
// ---------------------------------------------------------------------------

#[tokio::test]
async fn tools_call_applies_the_same_auth_gate_as_rpc() {
    let backend = Arc::new(StubBackend::gated_unattested());
    let app = router(backend.clone());
    let session = init_session(&app).await;

    // A `session` method is refused when an IdP is configured and the caller is
    // unattested — exactly like `POST /rpc` (see tests/auth_levels.rs).
    let (status, _, body) = send(
        &app,
        with_session(
            post_json(json!({
                "jsonrpc": "2.0",
                "id": 5,
                "method": "tools/call",
                "params": { "name": "rigorix_run", "arguments": {} }
            })),
            &session,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["result"]["isError"], true);
    assert!(
        body["result"]["content"][0]["text"]
            .as_str()
            .expect("text")
            .contains("session is required"),
        "the refusal names the same cause as /rpc"
    );
    assert!(
        !backend.dispatched.load(Ordering::SeqCst),
        "auth is checked before dispatch"
    );

    // A `public` method is still allowed unattested.
    let (_, _, body) = send(
        &app,
        with_session(
            post_json(json!({
                "jsonrpc": "2.0",
                "id": 6,
                "method": "tools/call",
                "params": { "name": "rigorix_plan", "arguments": {} }
            })),
            &session,
        ),
    )
    .await;
    assert_eq!(body["result"]["isError"], false);
}

// ---------------------------------------------------------------------------
// Transport / session lifecycle
// ---------------------------------------------------------------------------

#[tokio::test]
async fn requests_without_a_session_are_404() {
    let app = router(Arc::new(StubBackend::open()));
    let (status, _, _) = send(
        &app,
        post_json(json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" })),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn sse_per_request_is_returned_when_the_client_prefers_it() {
    let app = router(Arc::new(StubBackend::open()));
    let session = init_session(&app).await;
    let mut request = post_json(json!({ "jsonrpc": "2.0", "id": 7, "method": "tools/list" }));
    request
        .headers_mut()
        .insert(header::ACCEPT, "text/event-stream".parse().expect("accept"));
    let response = app
        .clone()
        .oneshot(with_session(request, &session))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);
    assert!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok())
            .unwrap_or("")
            .starts_with("text/event-stream")
    );
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("body");
    let text = String::from_utf8_lossy(&body);
    assert!(text.contains("event: message"));
    assert!(text.contains("\"tools\""));
}

#[tokio::test]
async fn notifications_are_accepted_with_202_and_get_returns_405() {
    let app = router(Arc::new(StubBackend::open()));
    let session = init_session(&app).await;
    let (status, _, _) = send(
        &app,
        with_session(
            post_json(json!({ "jsonrpc": "2.0", "method": "notifications/initialized" })),
            &session,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::ACCEPTED);

    let (status, headers, _) = send(
        &app,
        Request::builder()
            .method("GET")
            .uri("/mcp")
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    assert!(headers.get(header::ALLOW).is_some());
}

#[tokio::test]
async fn delete_terminates_the_session() {
    let app = router(Arc::new(StubBackend::open()));
    let session = init_session(&app).await;
    let (status, _, _) = send(
        &app,
        Request::builder()
            .method("DELETE")
            .uri("/mcp")
            .header(mcp_http::SESSION_HEADER, &session)
            .body(Body::empty())
            .expect("request"),
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT);

    // The id is now unknown → 404.
    let (status, _, _) = send(
        &app,
        with_session(
            post_json(json!({ "jsonrpc": "2.0", "id": 8, "method": "tools/list" })),
            &session,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn unknown_mcp_method_is_method_not_found() {
    let app = router(Arc::new(StubBackend::open()));
    let session = init_session(&app).await;
    let (status, _, body) = send(
        &app,
        with_session(
            post_json(json!({ "jsonrpc": "2.0", "id": 9, "method": "resources/list" })),
            &session,
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["error"]["code"], -32601);
}
