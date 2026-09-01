//! HTTP/MCP backend transport tests (GAP-A-23) using wiremock.
//!
//! Verifies the network transports actually work: HttpBackend posts the
//! evaluation payload and deserializes a ScoringResult; McpBackend speaks
//! JSON-RPC 2.0 and unwraps the result. Error paths (non-2xx) surface as
//! typed ScoredEvaluationError.

use std::collections::HashMap;

use rigorix_engine::scored_evaluation::domain::backend::ScoringBackend;
use rigorix_engine::scored_evaluation::domain::{Rubric, ScoredEvaluationError};
use rigorix_engine::scored_evaluation::infrastructure::backends::{HttpBackend, McpBackend};
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn sample_result_json() -> serde_json::Value {
    let mut dims = HashMap::new();
    dims.insert(
        "correctness".to_string(),
        serde_json::json!({"score": 0.95, "max": 1.0, "label": "Correctness", "passed": true}),
    );
    serde_json::json!({
        "passed": true,
        "dimensions": dims,
        "summary": "all good",
        "backend": "mock",
        "duration_ms": 10,
        "raw": null
    })
}

#[tokio::test]
async fn test_http_backend_transport_round_trip() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/score"))
        .respond_with(ResponseTemplate::new(200).set_body_json(sample_result_json()))
        .mount(&server)
        .await;

    let backend = HttpBackend::new(format!("{}/score", server.uri()), HashMap::new(), 5000);
    let result = backend
        .evaluate(
            &serde_json::json!({"code": "fn main() {}"}),
            &Rubric::inline(serde_json::json!({"quality": 0.9})),
        )
        .await
        .unwrap();

    assert!(result.passed);
    assert_eq!(result.backend, "mock");
    let dim = result.dimensions.get("correctness").unwrap();
    assert_eq!(dim.score, 0.95);
}

#[tokio::test]
async fn test_http_backend_transport_error_status() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/score"))
        .respond_with(ResponseTemplate::new(500).set_body_string("boom"))
        .mount(&server)
        .await;

    let backend = HttpBackend::new(format!("{}/score", server.uri()), HashMap::new(), 5000);
    let err = backend
        .evaluate(
            &serde_json::json!({}),
            &Rubric::inline(serde_json::json!({})),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, ScoredEvaluationError::BackendError(_)));
}

#[tokio::test]
async fn test_mcp_backend_transport_round_trip() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "result": sample_result_json(),
            "error": null,
            "id": 1
        })))
        .mount(&server)
        .await;

    let backend = McpBackend::new(format!("{}/mcp", server.uri()), 5000);
    let result = backend
        .evaluate(
            &serde_json::json!({"code": "fn main() {}"}),
            &Rubric::inline(serde_json::json!({"quality": 0.9})),
        )
        .await
        .unwrap();

    assert!(result.passed);
    assert_eq!(result.backend, "mock");
}

#[tokio::test]
async fn test_mcp_backend_transport_jsonrpc_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/mcp"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "jsonrpc": "2.0",
            "result": null,
            "error": {"code": -32000, "message": "no rubric"},
            "id": 1
        })))
        .mount(&server)
        .await;

    let backend = McpBackend::new(format!("{}/mcp", server.uri()), 5000);
    let err = backend
        .evaluate(
            &serde_json::json!({}),
            &Rubric::inline(serde_json::json!({})),
        )
        .await
        .unwrap_err();

    assert!(matches!(err, ScoredEvaluationError::BackendError(_)));
    assert!(err.to_string().contains("no rubric"), "got: {err}");
}
