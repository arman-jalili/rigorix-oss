//! Provider transport hardening (ISSUE-PRECONDITION-PROVIDERS).
//!
//! The provider unit tests inject a **stub** `ProviderClient`, so the real
//! `HttpProviderClient` transport was untested. These exercise it against a local
//! `wiremock` server.
//!
//! The cap is the point of this file. A provider is an untrusted input source, so
//! the cap must bound what the engine **allocates**, not merely what it **records**
//! — a `Response::bytes()` read would buffer an arbitrarily large body before the
//! size could be checked.
//!
//! Note on coverage: `wiremock` sets `Content-Length`, so these tests exercise the
//! early `content_length` rejection. The streaming loop in `provider_client.rs` is
//! the backstop for a chunked response that omits the length; it is not covered
//! here (a chunked-response server would be needed).

use rigorix_engine::precondition::application::PreconditionCheckInput;
use rigorix_engine::precondition::domain::{ProviderDetermination, ProviderMethod, ProviderSpec};
use rigorix_engine::precondition::infrastructure::{
    HttpProviderClient, MAX_RESPONSE_BYTES, ProviderClient,
};
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn spec(endpoint: String) -> ProviderSpec {
    ProviderSpec {
        id: "standing".to_string(),
        endpoint,
        method: ProviderMethod::Post,
        timeout_ms: 5_000,
        credential_ref: None,
        determination_field: "determination".to_string(),
        permit_values: Vec::new(),
        deny_values: Vec::new(),
    }
}

fn input() -> PreconditionCheckInput {
    PreconditionCheckInput {
        precondition_id: "beneficiary-authorized".to_string(),
        execution_id: uuid::Uuid::new_v4(),
        step: "payout_execute".to_string(),
        tool: "payout".to_string(),
        parameters: json!({ "beneficiary": "acct-1" }),
    }
}

/// A body over the cap must be refused — and refused by the cap, not by a parse
/// failure or a timeout.
#[tokio::test]
async fn oversized_response_is_refused_by_the_cap() {
    let server = MockServer::start().await;
    let body = "x".repeat(MAX_RESPONSE_BYTES + 1024);
    Mock::given(method("POST"))
        .and(path("/determine"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body))
        .mount(&server)
        .await;

    let client = HttpProviderClient;
    let error = client
        .determine(&spec(format!("{}/determine", server.uri())), &input())
        .await
        .expect_err("an over-cap response must be refused (fail closed)");

    let message = error.to_string();
    assert!(
        message.contains("cap"),
        "refusal should name the cap, got: {message}"
    );
}

/// A response inside the cap is accepted and its determination mapped.
#[tokio::test]
async fn response_within_the_cap_is_accepted() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/determine"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(r#"{"determination":"allowed","provider_version":"1.2.3"}"#),
        )
        .mount(&server)
        .await;

    let client = HttpProviderClient;
    let observation = client
        .determine(&spec(format!("{}/determine", server.uri())), &input())
        .await
        .expect("a permitted response inside the cap must be accepted");

    assert_eq!(observation.determination, ProviderDetermination::Permit);
    assert_eq!(observation.provider_version.as_deref(), Some("1.2.3"));
    // The exact response bytes are bound, and the endpoint is redacted.
    assert!(observation.response_digest.starts_with("sha256:"));
    assert!(observation.request_digest.starts_with("sha256:"));
    // Endpoint redaction keeps scheme + host (operability) and replaces the path
    // with a one-way hash — the path itself must not appear.
    assert!(observation.endpoint_redacted.starts_with("http://"));
    assert!(observation.endpoint_redacted.contains("sha256:"));
    assert!(!observation.endpoint_redacted.contains("determine"));
}

/// A non-2xx answer is *indeterminate*, not a transport failure: the bytes are
/// still bound so the record shows the provider was consulted.
#[tokio::test]
async fn non_2xx_is_indeterminate_with_bytes_bound() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/determine"))
        .respond_with(ResponseTemplate::new(503).set_body_string(r#"{"detail":"unavailable"}"#))
        .mount(&server)
        .await;

    let client = HttpProviderClient;
    let observation = client
        .determine(&spec(format!("{}/determine", server.uri())), &input())
        .await
        .expect("a provider that answers with 503 was still consulted");

    assert_eq!(observation.determination, ProviderDetermination::Unknown);
    assert!(observation.response_digest.starts_with("sha256:"));
}

/// An unreachable provider fails closed with an error whose message does not leak
/// the endpoint (a transport error string can carry the URL and its query).
#[tokio::test]
async fn unreachable_provider_fails_closed_without_leaking_the_endpoint() {
    // Port 1 on loopback: nothing listens.
    let spec = spec("http://127.0.0.1:1/determine?tenant=acme".to_string());
    let client = HttpProviderClient;
    let error = client
        .determine(&spec, &input())
        .await
        .expect_err("an unreachable provider must refuse (fail closed)");

    let message = error.to_string();
    assert!(
        !message.contains("tenant=acme") && !message.contains("127.0.0.1"),
        "the refusal must not leak the endpoint, got: {message}"
    );
}
