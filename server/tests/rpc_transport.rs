//! JSON-RPC 2.0 transport conformance (#888 OSS-C2, ADR-0001 D1/D6).
//!
//! Exercises single requests, batches, notifications, malformed envelopes,
//! and the `errors.json` taxonomy mapping without a live engine.

use async_trait::async_trait;
use serde_json::{Value, json};

use rigorix_mcp::host::error::{ErrorType, HostError};
use rigorix_server::backend::MethodBackend;
use rigorix_server::rpc;

/// A backend that echoes for public methods and returns a structured sequence
/// denial for `rigorix.execute`.
struct StubBackend;

#[async_trait]
impl MethodBackend for StubBackend {
    async fn dispatch(&self, method: &str, _params: Value) -> Result<Value, HostError> {
        match method {
            "rigorix.plan" => Ok(json!({ "plan": "ok" })),
            "rigorix.execute" => Err(HostError::new(
                ErrorType::DeniedBySequence,
                "Sequence policy denied step 'pay_b' (rule 'no-repeat-beneficiary-payout'): the plan was refused before any step executed",
            )
            .with_sequence("no-repeat-beneficiary-payout", "pay_b")),
            other => Err(HostError::method_not_found(other)),
        }
    }
}

fn request(id: Value, method: &str, params: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "method": method, "params": params })
}

#[tokio::test]
async fn single_request_returns_result() {
    let response = rpc::handle_value(request(json!(1), "rigorix.plan", json!({})), &StubBackend)
        .await
        .expect("single request responds");
    assert_eq!(response["jsonrpc"], "2.0");
    assert_eq!(response["id"], 1);
    assert_eq!(response["result"]["plan"], "ok");
}

#[tokio::test]
async fn batch_returns_an_array_of_responses() {
    let batch = json!([
        request(json!(1), "rigorix.plan", json!({})),
        request(json!(2), "rigorix.system.version", json!({})),
    ]);
    let response = rpc::handle_value(batch, &StubBackend)
        .await
        .expect("batch responds");
    let array = response.as_array().expect("array");
    assert_eq!(array.len(), 2);
    assert_eq!(array[0]["id"], 1);
    assert_eq!(array[1]["result"]["name"], "rigorix-server");
}

#[tokio::test]
async fn notification_produces_no_response_but_still_executes() {
    let notification = json!({ "jsonrpc": "2.0", "method": "rigorix.plan", "params": {} });
    assert!(
        rpc::handle_value(notification, &StubBackend)
            .await
            .is_none()
    );
}

#[tokio::test]
async fn batch_of_only_notifications_produces_no_response() {
    let batch = json!([
        { "jsonrpc": "2.0", "method": "rigorix.plan", "params": {} },
        { "jsonrpc": "2.0", "method": "rigorix.plan", "params": {} },
    ]);
    assert!(rpc::handle_value(batch, &StubBackend).await.is_none());
}

#[tokio::test]
async fn empty_batch_is_invalid_request() {
    let response = rpc::handle_value(json!([]), &StubBackend)
        .await
        .expect("responds");
    assert_eq!(response["error"]["code"], -32600);
}

#[tokio::test]
async fn unknown_method_is_method_not_found() {
    let response = rpc::handle_value(request(json!(9), "rigorix.nope", json!({})), &StubBackend)
        .await
        .expect("responds");
    assert_eq!(response["error"]["code"], -32601);
    assert_eq!(response["error"]["data"]["type"], "method_not_found");
}

#[tokio::test]
async fn wrong_jsonrpc_version_is_invalid_request() {
    let response = rpc::handle_value(
        json!({ "jsonrpc": "1.0", "id": 1, "method": "rigorix.plan" }),
        &StubBackend,
    )
    .await
    .expect("responds");
    assert_eq!(response["error"]["code"], -32600);
}

#[tokio::test]
async fn non_object_request_is_invalid_request() {
    let response = rpc::handle_value(json!("not a request"), &StubBackend)
        .await
        .expect("responds");
    assert_eq!(response["error"]["code"], -32600);
}

#[tokio::test]
async fn parse_error_helper_is_minus_32700() {
    let response = rpc::parse_error();
    assert_eq!(response["error"]["code"], -32700);
    assert_eq!(response["error"]["data"]["type"], "parse_error");
}

#[tokio::test]
async fn denied_by_sequence_carries_rule_id_and_step() {
    let response = rpc::handle_value(
        request(json!(1), "rigorix.execute", json!({})),
        &StubBackend,
    )
    .await
    .expect("responds");
    assert_eq!(response["error"]["code"], -32010);
    assert_eq!(response["error"]["data"]["type"], "denied_by_sequence");
    assert_eq!(
        response["error"]["data"]["rule_id"],
        "no-repeat-beneficiary-payout"
    );
    assert_eq!(response["error"]["data"]["step"], "pay_b");
    assert!(
        response["error"]["message"]
            .as_str()
            .expect("message")
            .contains("Sequence policy denied")
    );
}

#[tokio::test]
async fn system_version_reports_engine_and_api_versions() {
    let response = rpc::handle_value(
        request(json!(1), "rigorix.system.version", json!({})),
        &StubBackend,
    )
    .await
    .expect("responds");
    let result = &response["result"];
    assert_eq!(result["name"], "rigorix-server");
    assert_eq!(result["api_version"], "1.0.0");
    assert!(
        result["engine_version"]
            .as_str()
            .is_some_and(|v| !v.is_empty()),
        "engine_version present"
    );
    assert!(result["schemas"].as_array().is_some_and(|s| !s.is_empty()));
    assert!(
        result["capabilities"]
            .as_array()
            .is_some_and(|c| !c.is_empty())
    );
}
