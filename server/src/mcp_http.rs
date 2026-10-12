//! MCP Streamable HTTP endpoint — an optional adapter over the frozen
//! `rigorix.*` catalog (#915, ADR-0001 D4).
//!
//! ADR-0001 D4 keeps a **MCP Streamable HTTP** endpoint as an *optional*
//! adapter over the frozen catalog, gated on GUI-agent adoption. This module is
//! that adapter: it adds **no** new `rigorix.*` method and no raw-step escape
//! hatch. Every `tools/call` is translated to its catalog method and routed
//! through [`crate::rpc::invoke`] — the SAME governed path as `POST /rpc` — so
//! the auth level (D4) and dispatch behavior are identical by construction.
//!
//! # Governance invariants
//!
//! - **Catalog parity.** `tools/list` exposes exactly the catalog's governed
//!   subset: methods with an MCP tool mapping that are not `admin`. Enterprise
//!   proxy tools, `rigorix.policy.bundle` (admin, no tool), and
//!   `rigorix.system.version` (host-only, no tool) are never exposed.
//! - **No raw-step execution** (D6/D10). A `tools/call` name is only accepted
//!   if it maps to a catalog entry; the arguments become the catalog params.
//! - **Same auth as `/rpc`.** `tools/call` calls [`crate::rpc::invoke`], so a
//!   `session` method is refused when an IdP is configured and the caller is
//!   unattested — identical to `POST /rpc` (and the stdio host).
//!
//! # Transport (MCP Streamable HTTP)
//!
//! - `POST /mcp` — a single JSON-RPC message. A request yields a JSON
//!   (`application/json`) or SSE (`text/event-stream`) response; a
//!   notification/response yields `202 Accepted`.
//! - `GET /mcp` — the optional server-initiated stream; this host answers
//!   `405 Method Not Allowed` (the spec permits it — the catalog is
//!   request/response; live run events are on `GET /events`).
//! - `DELETE /mcp` — terminates the session (`204 No Content`).
//! - Sessions: `initialize` issues an `Mcp-Session-Id`; subsequent requests MUST
//!   present it (else `404`). `MCP-Protocol-Version` is echoed but not required.
//! - `Origin` is validated (DNS-rebinding protection): missing/`null` is allowed
//!   (non-browser clients), otherwise the host must be loopback or match `Host`.
//!
//! The legacy MCP SSE transport is **not** resurrected (GAP-A-10).

use std::collections::BTreeMap;
use std::collections::HashMap;
use std::convert::Infallible;
use std::sync::Mutex;
use std::time::Duration;

use axum::Json;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use serde_json::{Value, json};

use rigorix_mcp::host::all_tool_descriptors;
use rigorix_mcp::host::error::{ErrorType, HostError};

use crate::backend::MethodBackend;
use crate::catalog::{AuthLevel, CATALOG, CatalogEntry};
use crate::rpc;
use crate::state::ServerState;

/// HTTP header carrying the MCP session id.
pub const SESSION_HEADER: &str = "Mcp-Session-Id";
/// HTTP header carrying the negotiated MCP protocol version.
#[allow(dead_code)]
pub const PROTOCOL_VERSION_HEADER: &str = "MCP-Protocol-Version";
/// The protocol version assumed when the client omits one.
pub const DEFAULT_PROTOCOL_VERSION: &str = "2025-03-26";

// ---------------------------------------------------------------------------
// Session store
// ---------------------------------------------------------------------------

/// One MCP streamable-HTTP session.
#[derive(Debug, Clone)]
struct Session {
    protocol_version: String,
    initialized: bool,
}

/// In-memory MCP session registry.
///
/// Sessions are per-process and intentionally ephemeral: they carry no engine
/// authority (identity still comes from the governed host), so losing them on
/// restart only forces a re-`initialize`.
#[derive(Debug, Default)]
pub struct McpSessions {
    // PANIC-AUDIT (high, docs/panic-audit-2026-10-12.md, ISSUE-PRODUCTION-PANIC-SURFACE):
    // every access below is `.lock().expect("session lock")`. A mutex is POISONED by any
    // panic while held, and `expect` then panics on every later request — turning a
    // one-off fault into a permanent one for the whole /mcp surface. The guarded data is
    // a plain map, so recover with `unwrap_or_else(|e| e.into_inner())` (or map to a typed
    // error) rather than propagating the poison. Every site is listed in
    // .pi/panic-baseline.tsv.
    inner: Mutex<HashMap<String, Session>>,
}

impl McpSessions {
    /// Create a session and return its unguessable id.
    pub fn create(&self, protocol_version: impl Into<String>) -> String {
        let id = uuid::Uuid::new_v4().to_string();
        self.inner.lock().expect("session lock").insert(
            id.clone(),
            Session {
                protocol_version: protocol_version.into(),
                initialized: false,
            },
        );
        id
    }

    /// Whether the id names a live session.
    pub fn contains(&self, id: &str) -> bool {
        self.inner.lock().expect("session lock").contains_key(id)
    }

    /// Mark a session fully initialized (after `notifications/initialized`).
    pub fn mark_initialized(&self, id: &str) -> bool {
        match self.inner.lock().expect("session lock").get_mut(id) {
            Some(session) => {
                session.initialized = true;
                true
            }
            None => false,
        }
    }

    /// Terminate a session. Returns whether it existed.
    pub fn terminate(&self, id: &str) -> bool {
        self.inner
            .lock()
            .expect("session lock")
            .remove(id)
            .is_some()
    }

    /// The protocol version negotiated for a live session, if any.
    pub fn protocol_version(&self, id: &str) -> Option<String> {
        self.inner
            .lock()
            .expect("session lock")
            .get(id)
            .map(|session| session.protocol_version.clone())
    }

    /// Number of live sessions.
    pub fn len(&self) -> usize {
        self.inner.lock().expect("session lock").len()
    }

    /// Whether there are no live sessions.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }
}

// ---------------------------------------------------------------------------
// Catalog governance
// ---------------------------------------------------------------------------

/// The catalog's **governed subset** for MCP: every method with an MCP tool
/// mapping that is not enterprise/admin (ADR-0001 D3/D9/D10).
pub fn governed_tools() -> Vec<&'static CatalogEntry> {
    CATALOG
        .iter()
        .filter(|entry| entry.mcp_tool.is_some() && entry.auth != AuthLevel::Admin)
        .collect()
}

/// Resolve an MCP tool name back to its frozen catalog entry, if governed.
pub fn method_for_tool(tool: &str) -> Option<&'static CatalogEntry> {
    CATALOG
        .iter()
        .find(|entry| entry.mcp_tool == Some(tool) && entry.auth != AuthLevel::Admin)
}

/// Every governed tool descriptor, in catalog order.
///
/// Descriptors come from the shared stdio host (`all_tool_descriptors`) plus
/// the auth descriptors, so the MCP surface is the same one the stdio binary
/// advertises — filtered to the catalog's governed subset. Any governed tool
/// lacking a descriptor gets a minimal object (never silently omitted).
pub fn tools_list() -> Vec<Value> {
    let descriptors = descriptor_map();
    governed_tools()
        .iter()
        .filter_map(|entry| {
            let name = entry.mcp_tool?;
            Some(descriptors.get(name).cloned().unwrap_or_else(|| {
                json!({
                    "name": name,
                    "description": format!("Frozen catalog method {}", entry.name),
                    "inputSchema": { "type": "object", "additionalProperties": true }
                })
            }))
        })
        .collect()
}

fn descriptor_map() -> BTreeMap<String, Value> {
    let mut map = BTreeMap::new();
    for descriptor in all_tool_descriptors() {
        if let Some(name) = descriptor.get("name").and_then(Value::as_str) {
            map.insert(name.to_string(), descriptor);
        }
    }
    for descriptor in rigorix_mcp::auth::interfaces::mcp::auth_tool_descriptors() {
        if let Some(name) = descriptor.get("name").and_then(Value::as_str) {
            map.insert(name.to_string(), descriptor);
        }
    }
    map
}

// ---------------------------------------------------------------------------
// MCP dispatch (transport-agnostic)
// ---------------------------------------------------------------------------

/// Dispatch one decoded MCP JSON-RPC message.
///
/// Returns `None` for notifications (no response). `initialize` is handled by
/// the caller for session creation; this function still produces its result.
pub async fn dispatch(value: &Value, backend: &dyn MethodBackend) -> Option<Value> {
    let object = value.as_object()?;
    let id = object.get("id").cloned().unwrap_or(Value::Null);
    let is_notification = !object.contains_key("id");

    let Some(method) = object.get("method").and_then(Value::as_str) else {
        // A JSON-RPC response (result/error, no method) is accepted and
        // dropped; anything else is an invalid request.
        if object.contains_key("result") || object.contains_key("error") {
            return None;
        }
        return Some(rpc::invalid_request(
            "Invalid Request: message is neither a request nor a response",
        ));
    };

    let params = object.get("params").cloned().unwrap_or(Value::Null);

    let reply: Result<Value, HostError> = match method {
        "initialize" => initialize_result(&params),
        // The spec name and the stdio host's legacy name are both no-ops.
        "notifications/initialized" | "initialized" => return None,
        "ping" => Ok(json!({})),
        "tools/list" => Ok(json!({ "tools": tools_list() })),
        "tools/call" => call_tool(&params, backend).await,
        other => Err(HostError::method_not_found(other)),
    };

    if is_notification {
        return None;
    }
    Some(match reply {
        Ok(result) => rpc::success(id, result),
        Err(error) => rpc::error(id, &error),
    })
}

fn initialize_result(params: &Value) -> Result<Value, HostError> {
    let protocol_version = params
        .get("protocolVersion")
        .and_then(Value::as_str)
        .unwrap_or(DEFAULT_PROTOCOL_VERSION);
    Ok(json!({
        "protocolVersion": protocol_version,
        "capabilities": { "tools": { "listChanged": false } },
        "serverInfo": {
            "name": "rigorix-server",
            "version": env!("CARGO_PKG_VERSION"),
        },
    }))
}

/// Translate a `tools/call` into its catalog method and invoke it through the
/// shared governed path. Tool failures are returned as MCP `isError` results
/// (matching the stdio host), never as a JSON-RPC protocol error.
async fn call_tool(params: &Value, backend: &dyn MethodBackend) -> Result<Value, HostError> {
    let name = params.get("name").and_then(Value::as_str).ok_or_else(|| {
        HostError::new(
            ErrorType::InvalidRequest,
            "Invalid tools/call: missing tool name",
        )
    })?;
    let arguments = params.get("arguments").cloned().unwrap_or(Value::Null);

    let Some(entry) = method_for_tool(name) else {
        return Ok(tool_content(
            &Value::String(format!("Unknown tool: {name}")),
            true,
        ));
    };

    match rpc::invoke(entry.name, arguments, backend).await {
        Ok(result) => Ok(tool_content(&result, false)),
        Err(error) => Ok(tool_content(&Value::String(error.message), true)),
    }
}

fn tool_content(result: &Value, is_error: bool) -> Value {
    let text = match result {
        Value::String(text) => text.clone(),
        other => serde_json::to_string(other).unwrap_or_default(),
    };
    json!({
        "content": [ { "type": "text", "text": text } ],
        "isError": is_error,
    })
}

// ---------------------------------------------------------------------------
// HTTP handlers
// ---------------------------------------------------------------------------

/// `POST /mcp` — receive a JSON-RPC message.
pub async fn post_handler(
    State(state): State<ServerState>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if !origin_allowed(&headers) {
        return (
            StatusCode::FORBIDDEN,
            "Forbidden: cross-origin request rejected",
        )
            .into_response();
    }

    let value: Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return with_json(StatusCode::OK, rpc::parse_error()),
    };

    let method = value.get("method").and_then(Value::as_str).unwrap_or("");
    let is_initialize = method == "initialize";

    // Sessions: `initialize` mints one; everything else must present a live id.
    if !is_initialize && let Some(response) = missing_session(&state, &headers, &value) {
        return response;
    }

    let new_session = is_initialize.then(|| {
        let protocol_version = value
            .get("params")
            .and_then(|params| params.get("protocolVersion"))
            .and_then(Value::as_str)
            .unwrap_or(DEFAULT_PROTOCOL_VERSION);
        state.mcp_sessions.create(protocol_version)
    });

    if (method == "notifications/initialized" || method == "initialized")
        && let Some(id) = headers.get(SESSION_HEADER).and_then(|h| h.to_str().ok())
    {
        state.mcp_sessions.mark_initialized(id);
    }

    let Some(response) = dispatch(&value, state.backend.as_ref()).await else {
        return with_session(StatusCode::ACCEPTED.into_response(), new_session);
    };

    let mut http = if prefers_sse(&headers) {
        sse_response(response)
    } else {
        with_json(StatusCode::OK, response)
    };
    if let Some(id) = new_session {
        attach_session(&mut http, &id);
    }
    http
}

/// `GET /mcp` — the optional server-initiated stream. This host does not offer
/// one (the catalog is request/response; live events are on `GET /events`), so
/// it answers `405` as the spec permits.
pub async fn get_handler() -> Response {
    (
        StatusCode::METHOD_NOT_ALLOWED,
        [(header::ALLOW, "POST, DELETE")],
        "This host does not offer a server-initiated MCP stream; use POST /mcp (live run events: GET /events).",
    )
        .into_response()
}

/// `DELETE /mcp` — terminate the session.
pub async fn delete_handler(State(state): State<ServerState>, headers: HeaderMap) -> Response {
    let Some(id) = headers.get(SESSION_HEADER).and_then(|h| h.to_str().ok()) else {
        return session_not_found(&Value::Null);
    };
    if state.mcp_sessions.terminate(id) {
        StatusCode::NO_CONTENT.into_response()
    } else {
        session_not_found(&Value::Null)
    }
}

fn missing_session(state: &ServerState, headers: &HeaderMap, value: &Value) -> Option<Response> {
    let id = headers.get(SESSION_HEADER).and_then(|h| h.to_str().ok());
    match id {
        Some(id) if state.mcp_sessions.contains(id) => None,
        _ => {
            let id = value.get("id").cloned().unwrap_or(Value::Null);
            Some(session_not_found(&id))
        }
    }
}

fn session_not_found(id: &Value) -> Response {
    let error = HostError::new(
        ErrorType::InvalidRequest,
        "Session not found or expired — call initialize first",
    );
    with_json(StatusCode::NOT_FOUND, rpc::error(id.clone(), &error))
}

// ---------------------------------------------------------------------------
// Response helpers
// ---------------------------------------------------------------------------

fn with_json(status: StatusCode, value: Value) -> Response {
    (status, Json(value)).into_response()
}

fn attach_session(response: &mut Response, id: &str) {
    if let Ok(value) = id.parse() {
        response.headers_mut().insert(SESSION_HEADER, value);
    }
}

fn with_session(mut response: Response, id: Option<String>) -> Response {
    if let Some(id) = id {
        attach_session(&mut response, &id);
    }
    response
}

fn sse_response(value: Value) -> Response {
    let data = serde_json::to_string(&value).unwrap_or_else(|_| "{}".to_string());
    let event = Event::default().event("message").data(data);
    let stream = futures::stream::iter([Ok::<Event, Infallible>(event)]);
    Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(Duration::from_secs(15))
                .text("keep-alive"),
        )
        .into_response()
}

/// Whether the client asked for a request-scoped SSE response. The spec
/// requires clients to accept both; JSON is the default when offered.
fn prefers_sse(headers: &HeaderMap) -> bool {
    let accept = headers
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("");
    accept.contains("text/event-stream") && !accept.contains("application/json")
}

/// DNS-rebinding guard (MCP security best practice). A missing `Origin` or
/// `null` is allowed (non-browser clients); otherwise the origin host must be
/// loopback or match the request `Host`.
fn origin_allowed(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN).and_then(|v| v.to_str().ok()) else {
        return true;
    };
    if origin == "null" {
        return true;
    }
    let origin_host = origin
        .split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .and_then(|authority| authority.rsplit('@').next())
        .map(strip_port)
        .unwrap_or("");
    let host_header = headers
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .map(strip_port);
    is_loopback(origin_host) || host_header == Some(origin_host)
}

fn strip_port(value: &str) -> &str {
    if let Some(rest) = value.strip_prefix('[') {
        // IPv6 literal: [::1]:3001
        return rest.split(']').next().unwrap_or(rest);
    }
    value.split(':').next().unwrap_or(value)
}

fn is_loopback(host: &str) -> bool {
    matches!(host, "localhost" | "127.0.0.1" | "::1")
}

// ---------------------------------------------------------------------------
// Tests (pure, no live engine)
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    struct EchoBackend;

    #[async_trait]
    impl MethodBackend for EchoBackend {
        async fn dispatch(&self, method: &str, params: Value) -> Result<Value, HostError> {
            Ok(json!({ "method": method, "params": params }))
        }
    }

    fn request(id: i64, method: &str, params: Value) -> Value {
        json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
    }

    #[tokio::test]
    async fn tools_list_is_the_catalog_governed_subset() {
        let tools = tools_list();
        let exposed: Vec<&str> = tools
            .iter()
            .filter_map(|tool| tool["name"].as_str())
            .collect();
        let expected: Vec<&str> = governed_tools()
            .iter()
            .filter_map(|entry| entry.mcp_tool)
            .collect();
        assert_eq!(exposed, expected);
        assert_eq!(exposed.len(), 17, "17 governed tools");
        // Never expose admin/host-only/enterprise tools.
        assert!(!exposed.contains(&"rigorix_policy_bundle"));
        assert!(
            exposed
                .iter()
                .all(|name| !name.starts_with("rigorix_enterprise"))
        );
    }

    #[tokio::test]
    async fn tools_call_maps_to_catalog_and_forwards_params() {
        let response = dispatch(
            &request(
                1,
                "tools/call",
                json!({ "name": "rigorix_plan", "arguments": { "intent": "x" } }),
            ),
            &EchoBackend,
        )
        .await
        .expect("responds");
        let text = response["result"]["content"][0]["text"]
            .as_str()
            .expect("text content");
        let inner: Value = serde_json::from_str(text).expect("inner json");
        assert_eq!(inner["method"], "rigorix.plan");
        assert_eq!(inner["params"]["intent"], "x");
        assert_eq!(response["result"]["isError"], false);
    }

    #[tokio::test]
    async fn unknown_tool_is_an_iserror_result_not_a_raw_dispatch() {
        let response = dispatch(
            &request(1, "tools/call", json!({ "name": "rigorix_step_run" })),
            &EchoBackend,
        )
        .await
        .expect("responds");
        assert_eq!(response["result"]["isError"], true);
        assert!(
            response["result"]["content"][0]["text"]
                .as_str()
                .expect("text")
                .contains("Unknown tool")
        );
    }

    #[tokio::test]
    async fn initialize_negotiates_protocol_and_advertises_tools() {
        let response = dispatch(
            &request(
                1,
                "initialize",
                json!({ "protocolVersion": "2025-06-18", "clientInfo": { "name": "gui" } }),
            ),
            &EchoBackend,
        )
        .await
        .expect("responds");
        assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
        assert!(response["result"]["capabilities"]["tools"].is_object());
        assert_eq!(response["result"]["serverInfo"]["name"], "rigorix-server");
    }

    #[test]
    fn sessions_lifecycle() {
        let sessions = McpSessions::default();
        assert!(sessions.is_empty());
        let id = sessions.create("2025-03-26");
        assert!(sessions.contains(&id));
        assert_eq!(
            sessions.protocol_version(&id).as_deref(),
            Some("2025-03-26")
        );
        assert!(sessions.mark_initialized(&id));
        assert!(sessions.terminate(&id));
        assert!(!sessions.contains(&id));
        assert!(!sessions.terminate(&id));
    }

    #[test]
    fn origin_guard_allows_loopback_and_rejects_cross_origin() {
        let mut headers = HeaderMap::new();
        assert!(origin_allowed(&headers), "missing origin allowed");

        headers.insert(header::ORIGIN, "http://localhost:5173".parse().unwrap());
        assert!(origin_allowed(&headers));

        headers.insert(header::HOST, "127.0.0.1:3001".parse().unwrap());
        headers.insert(header::ORIGIN, "http://evil.example".parse().unwrap());
        assert!(!origin_allowed(&headers));

        headers.insert(header::ORIGIN, "http://127.0.0.1:3001".parse().unwrap());
        assert!(origin_allowed(&headers));
    }

    #[test]
    fn accept_negotiation_defaults_to_json() {
        let mut headers = HeaderMap::new();
        assert!(!prefers_sse(&headers));
        headers.insert(
            header::ACCEPT,
            "application/json, text/event-stream".parse().unwrap(),
        );
        assert!(!prefers_sse(&headers), "both → JSON");
        headers.insert(header::ACCEPT, "text/event-stream".parse().unwrap());
        assert!(prefers_sse(&headers));
    }
}
