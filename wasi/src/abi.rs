//! The request/response protocol — one entry, the operation named inside the JSON.
//!
//! # Why the operation is inside the payload
//!
//! Four exports named after four operations is four bindings a host must generate, and the
//! fifth operation (verification, ADR-013) is exactly how that surface grows. One entry with
//! an `op` field is a boundary that cannot grow a dialect: adding an operation changes a
//! `match` arm, not the ABI.
//!
//! # The refusal shape is the crate's, not this file's
//!
//! A refusal from the gate already arrives as JSON with a `kind` — `capacity_exceeded`,
//! `duplicate_effect`, `gate_unavailable`, `request_invalid`. This layer adds only the
//! refusals that belong to the boundary itself (the payload was not JSON, the payload named
//! no operation) and uses the same shape, so a host parses one thing.

use serde::Serialize;
use serde_json::Value;

/// The largest request this boundary will read.
///
/// A host that hands over a gigabyte is not making a request; it is testing whether the
/// boundary trusts a length. 1 MiB is far above the largest scenario in the repository and
/// far below a size that makes a bad length expensive.
pub const MAX_REQUEST_BYTES: usize = 1 << 20;

/// A refusal produced at the boundary rather than by the gate.
#[derive(Debug, Serialize)]
struct BoundaryRefusal {
    /// The vocabulary the host already switches on.
    kind: &'static str,
    /// What was wrong with the request. Never a gate operand (NFR-003).
    message: String,
}

fn refuse(message: impl Into<String>) -> String {
    let refusal = BoundaryRefusal {
        kind: "request_invalid",
        message: message.into(),
    };
    serde_json::to_string(&refusal).unwrap_or_else(|_| {
        r#"{"kind":"request_invalid","message":"the refusal could not be encoded"}"#.to_string()
    })
}

/// Answer one request.
///
/// Infallible by signature: every failure is a JSON refusal, because a `Result` here would
/// become a trap on the target this runs on (ADR-012).
pub fn dispatch(request: &str) -> String {
    if request.len() > MAX_REQUEST_BYTES {
        return refuse("the request is larger than this boundary reads");
    }
    let value: Value = match serde_json::from_str(request) {
        Ok(value) => value,
        Err(_) => return refuse("the request is not JSON"),
    };
    let Some(object) = value.as_object() else {
        return refuse("the request is not an object");
    };
    let Some(op) = object.get("op").and_then(Value::as_str) else {
        return refuse("the request names no operation: `op` is missing or is not a string");
    };

    match op {
        "contract_version" => pass(Ok(
            rigorix_gate_wasm::interfaces::wasm_api::contract_version(),
        )),
        "open_session" => {
            let (Some(scenario), Some(fixtures), Some(clock), Some(seed)) = (
                field(object, "scenario"),
                field(object, "fixtures"),
                field(object, "clock"),
                field(object, "seed"),
            ) else {
                return refuse(
                    "open_session requires `scenario`, `fixtures`, `clock` and `seed` as JSON values",
                );
            };
            pass(rigorix_gate_wasm::interfaces::wasm_api::open_session(
                inline(scenario),
                inline(fixtures),
                inline(clock),
                inline(seed),
            ))
        }
        "attempt_run" => {
            let (Some(handle), Some(action)) = (field(object, "handle"), field(object, "action"))
            else {
                return refuse("attempt_run requires `handle` and `action` as JSON values");
            };
            pass(rigorix_gate_wasm::interfaces::wasm_api::attempt_run(
                inline(handle),
                inline(action),
            ))
        }
        "export_artifact_set" => {
            let Some(handle) = field(object, "handle") else {
                return refuse("export_artifact_set requires `handle` as a JSON value");
            };
            pass(rigorix_gate_wasm::interfaces::wasm_api::export_artifact_set(inline(handle)))
        }
        other => refuse(format!(
            "`{other}` is not an operation this boundary has; the four are open_session, \
             attempt_run, export_artifact_set and contract_version"
        )),
    }
}

/// A named member of the request, if it is present and is not JSON `null`.
fn field<'a>(object: &'a serde_json::Map<String, Value>, name: &str) -> Option<&'a Value> {
    object.get(name).filter(|value| !value.is_null())
}

/// Re-serialise a request member so the crate's own DTOs parse it.
///
/// The bytes are not the host's bytes: the crate takes a JSON *string*, so the member is
/// encoded once here. A vector's decision does not depend on this layout — the engine
/// canonicalises what it decides.
fn inline(value: &Value) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "null".to_string())
}

/// Forward the crate's answer.
///
/// The one `Err` on this contract is a malformed handle — a programmer mistake, not a
/// decision (ADR-012/ADR-004). The browser artifact raises it into a JS exception because a
/// JS host has somewhere to put one; a WASI host does not, and a trap is not a refusal
/// (ADR-012), so it crosses as JSON.
///
/// It already *is* JSON: the crate renders its handle refusal (`project_handle`) before
/// returning it. Re-wrapping that in `request_invalid` would both lose the vocabulary the
/// host switches on and quote a JSON document inside a message field — so a payload that is
/// a refusal is forwarded, and only a bare message becomes `request_invalid`.
fn pass(answer: Result<String, String>) -> String {
    match answer {
        Ok(json) => json,
        Err(carried) => match serde_json::from_str::<Value>(&carried) {
            Ok(value) if value.get("kind").is_some() => carried,
            _ => refuse(carried),
        },
    }
}
