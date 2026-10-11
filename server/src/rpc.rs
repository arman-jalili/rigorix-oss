//! JSON-RPC 2.0 transport (#888 OSS-C2).
//!
//! Single request, batch array, and notifications (no `id`). A well-formed
//! envelope always yields HTTP 200 at the transport layer; method-level
//! failures are JSON-RPC error objects (ADR-0001 D1/D6). Malformed JSON is
//! `-32700`, structurally invalid envelopes are `-32600`, unknown methods are
//! `-32601`, and engine failures use the `errors.json` taxonomy via
//! [`rigorix_mcp::host::error::HostError`].

use serde_json::{Value, json};

use rigorix_mcp::host::error::{ErrorType, HostError};

use crate::backend::MethodBackend;
use crate::catalog::{self, AuthLevel};

/// The JSON-RPC protocol version this host speaks.
pub const JSONRPC: &str = "2.0";

/// A JSON-RPC success response.
pub fn success(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": JSONRPC, "id": id, "result": result })
}

/// A JSON-RPC error response derived from a [`HostError`].
pub fn error(id: Value, host_error: &HostError) -> Value {
    json!({ "jsonrpc": JSONRPC, "id": id, "error": host_error.to_jsonrpc_error() })
}

/// The `-32700` parse error (returned when the HTTP body is not JSON).
pub fn parse_error() -> Value {
    error(
        Value::Null,
        &HostError::new(ErrorType::ParseError, "Parse error: invalid JSON"),
    )
}

/// The `-32600` invalid request (returned for an empty batch).
pub fn invalid_request(message: &str) -> Value {
    error(
        Value::Null,
        &HostError::new(ErrorType::InvalidRequest, message),
    )
}

/// Handle a decoded JSON-RPC body.
///
/// Returns `None` when the input is a notification (or a batch containing only
/// notifications) — the transport then replies with an empty HTTP 200 body.
pub async fn handle_value(value: Value, backend: &dyn MethodBackend) -> Option<Value> {
    match value {
        Value::Array(items) => {
            if items.is_empty() {
                return Some(invalid_request("Invalid Request: empty batch"));
            }
            let mut responses = Vec::new();
            for item in items {
                if let Some(response) = handle_single(item, backend).await {
                    responses.push(response);
                }
            }
            if responses.is_empty() {
                None
            } else {
                Some(Value::Array(responses))
            }
        }
        other => handle_single(other, backend).await,
    }
}

/// Handle one JSON-RPC request object (or return an error for a non-object).
pub async fn handle_single(value: Value, backend: &dyn MethodBackend) -> Option<Value> {
    let Value::Object(object) = value else {
        return Some(invalid_request(
            "Invalid Request: request must be a JSON object",
        ));
    };

    let id = object.get("id").cloned().unwrap_or(Value::Null);
    let is_notification = !object.contains_key("id");

    if object.get("jsonrpc").and_then(Value::as_str) != Some(JSONRPC) {
        return respond(
            is_notification,
            error(
                id,
                &HostError::new(
                    ErrorType::InvalidRequest,
                    "Invalid Request: jsonrpc must be \"2.0\"",
                ),
            ),
        );
    }

    let Some(method) = object
        .get("method")
        .and_then(Value::as_str)
        .map(str::to_string)
    else {
        return respond(
            is_notification,
            error(
                id,
                &HostError::new(ErrorType::InvalidRequest, "Invalid Request: missing method"),
            ),
        );
    };
    let params = object.get("params").cloned().unwrap_or(Value::Null);

    let result = invoke(&method, params, backend).await;

    respond(
        is_notification,
        match result {
            Ok(result) => success(id, result),
            Err(host_error) => error(id, &host_error),
        },
    )
}

/// Resolve, authorize, and dispatch a frozen catalog method.
///
/// This is the SINGLE governed entry point behind both `POST /rpc`
/// ([`handle_single`]) and the MCP Streamable HTTP `tools/call` adapter
/// ([`crate::mcp_http`]). Sharing it guarantees the two transports enforce the
/// **same** auth level (ADR-0001 D4) and take the **same** dispatch path — there
/// is no second dialect and no raw-step bypass.
///
/// Unknown methods yield `method_not_found`; `admin` methods are refused
/// (`not_enabled`) in OSS; `session` methods are refused when an IdP is
/// configured and the caller is unattested (local/no-IdP mode is preserved).
pub async fn invoke(
    method: &str,
    params: Value,
    backend: &dyn MethodBackend,
) -> Result<Value, HostError> {
    let entry = catalog::find(method).ok_or_else(|| HostError::method_not_found(method))?;

    // ── Auth level (ADR-0001 D4) ──
    match entry.auth {
        AuthLevel::Public => {}
        AuthLevel::Session => {
            // Local/legacy mode (no IdP configured) does not refuse — mirrors
            // the stdio MCP host. With an IdP configured, an unattested caller
            // is refused.
            if backend.auth_configured().await && !backend.is_authenticated().await {
                return Err(HostError::not_authenticated(
                    "A session is required for this method — run rigorix.auth.login",
                ));
            }
        }
        AuthLevel::Admin => {
            return Err(HostError::not_enabled(format!(
                "{method} is an enterprise-only method"
            )));
        }
    }

    if method == "rigorix.system.version" {
        Ok(crate::version::system_version())
    } else {
        backend.dispatch(method, params).await
    }
}

/// Notifications produce no response.
fn respond(is_notification: bool, response: Value) -> Option<Value> {
    if is_notification {
        None
    } else {
        Some(response)
    }
}
