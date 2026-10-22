//! #1165 / #1174: the receipt reader resolves an `Ambiguous` record when the
//! provider echoes the intent key.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#providerreceiptreader
//! Implements: ISSUE-1163 — L3 is reachable only if the key was sent
//! Issue: #1165, #1174
//!
//! The reader can look a receipt up by idempotency key, but that lookup can
//! only succeed if the provider was given the key. What this file proves:
//!
//! 1. a provider that exposes the key on its receipt can be read back, and the
//!    resulting evidence resolves the `Ambiguous` record;
//! 2. the read-back uses the same intent-derived key the service recorded.
//!
//! What it does **not** prove: that the engine sends the key to an HTTP
//! provider. The engine has no HTTP tool (its sinks are processes), so the
//! request below is a stand-in for a provider integration and the header is
//! reserved (#1174). The implemented transmission is the process env var, set
//! in `exec_run_command` and asserted by the `tools` unit test
//! `run_command_carries_the_effect_key_to_the_provider`.

use std::sync::Arc;

use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::application::{
    BeginEffectInput, EffectCommitmentService, EffectCommitmentServiceImpl, MarkAmbiguousInput,
    ProviderReceiptConfig, ProviderReceiptReader, ResolveEffectInput, Sha256IntentKeyDerivation,
};
use rigorix_engine::effect_commitment::domain::{
    EffectIntent, EffectState, ResolverIdentity, ResolverKind,
};
use rigorix_engine::effect_commitment::infrastructure::{
    HttpProviderReceiptReader, InMemoryEffectLedger,
};
use rigorix_engine::effect_commitment::interfaces::EFFECT_IDEMPOTENCY_HEADER;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn intent() -> EffectIntent {
    EffectIntent::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        0,
        IdempotencyKey::new("placeholder"),
        "payments.charge",
        "args-digest",
        "precondition-digest",
        "authority-digest",
    )
}

#[tokio::test]
async fn the_receipt_reader_resolves_an_ambiguous_record_when_the_provider_echoes_the_key() {
    let server = MockServer::start().await;
    let ledger = Arc::new(InMemoryEffectLedger::new());
    let service = EffectCommitmentServiceImpl::new(
        ledger.clone(),
        Arc::new(Sha256IntentKeyDerivation::new()),
    );

    // The service derives the authoritative key from the intent.
    let begun = service
        .begin(BeginEffectInput { intent: intent() })
        .await
        .expect("begin");
    let key = begun.record.intent().idempotency_key.as_str().to_string();

    // 1. Stand-in for a provider integration: the engine has no HTTP tool, so
    //    this request exercises the reader half, not an engine sender (#1174).
    Mock::given(method("POST"))
        .and(path("/v1/charges"))
        .and(header(EFFECT_IDEMPOTENCY_HEADER, key.as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "receipt_id": "re_123",
            "status": "succeeded"
        })))
        .expect(1)
        .mount(&server)
        .await;
    let provider_response = reqwest::Client::new()
        .post(format!("{}/v1/charges", server.uri()))
        .header(EFFECT_IDEMPOTENCY_HEADER, &key)
        .json(&json!({ "amount": 100 }))
        .send()
        .await
        .expect("provider request");
    assert!(provider_response.status().is_success());

    // 2. The reader looks the receipt up by the same key and produces evidence.
    Mock::given(method("GET"))
        .and(path("/v1/receipts"))
        .and(query_param("idempotency_key", key.as_str()))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "receipt_id": "re_123",
            "status": "succeeded"
        })))
        .mount(&server)
        .await;

    let reader = HttpProviderReceiptReader::new(ProviderReceiptConfig::new(
        server.uri(),
        "stripe",
        2_000,
        65_536,
        None,
    ));
    let evidence = reader
        .observe(begun.record.intent(), None)
        .await
        .expect("read-back by idempotency key");
    assert_eq!(evidence.receipt_id, "re_123");

    // 3. The ambiguous record is resolvable with that evidence.
    service
        .mark_ambiguous(MarkAmbiguousInput {
            record_id: begun.record.id(),
            reason: "response lost".to_string(),
        })
        .await
        .expect("mark ambiguous");
    let resolved = service
        .resolve(ResolveEffectInput {
            record_id: begun.record.id(),
            outcome: evidence,
            resolver: ResolverIdentity::new("stripe", ResolverKind::Provider, "readback-1"),
        })
        .await
        .expect("resolve with provider evidence");
    assert_eq!(resolved.record.state(), EffectState::Committed);
}

#[tokio::test]
async fn the_provider_key_is_the_intent_key_not_an_attempt_key() {
    let ledger = Arc::new(InMemoryEffectLedger::new());
    let service =
        EffectCommitmentServiceImpl::new(ledger, Arc::new(Sha256IntentKeyDerivation::new()));
    let a = service
        .begin(BeginEffectInput { intent: intent() })
        .await
        .expect("begin");
    let b = service
        .begin(BeginEffectInput {
            intent: a.record.intent().clone(),
        })
        .await
        .expect("custody read");
    assert_eq!(a.record.id(), b.record.id());
    assert_eq!(
        a.record.intent().idempotency_key,
        b.record.intent().idempotency_key,
        "a retry must carry the same provider key, or the provider cannot dedupe"
    );
}
