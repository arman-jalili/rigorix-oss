//! DefaultEffectCommitmentFactory integration: composition actually runs.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#defaulteffectcommitmentfactory
//! Implements: ISSUE-EFFECT-COMMITMENT-14 — upstream/downstream integration
//! Issue: #1127
//! ADR-019: the factory exists so the context cannot be compiled and unreachable

use std::sync::Arc;

use chrono::Utc;
use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::application::{
    BeginEffectInput, EffectCommitmentBackendConfig, EffectCommitmentFactory,
    EffectCommitmentService, EffectCommitmentServiceImpl, ProviderReceiptConfig,
    SettleDefiniteInput, Sha256IntentKeyDerivation,
};
use rigorix_engine::effect_commitment::domain::{EffectIntent, ObservedOutcome};
use rigorix_engine::effect_commitment::infrastructure::DefaultEffectCommitmentFactory;
use rigorix_engine::effect_commitment::interfaces::http::BEGIN_PATH;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn intent() -> EffectIntent {
    EffectIntent::new(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        1,
        IdempotencyKey::new("attempt-derived"),
        "payments.charge",
        "args-digest",
        "precondition-digest",
        "authority-digest",
    )
}

fn evidence() -> ObservedOutcome {
    ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe")
}

#[tokio::test]
async fn the_factory_builds_a_working_in_memory_service() {
    let factory = DefaultEffectCommitmentFactory::new();
    let ledger = Arc::from(factory.create(EffectCommitmentBackendConfig::InMemory));
    let service =
        EffectCommitmentServiceImpl::new(ledger, Arc::new(Sha256IntentKeyDerivation::new()));

    let begun = service
        .begin(BeginEffectInput { intent: intent() })
        .await
        .unwrap();
    assert_eq!(begun.record.state().as_str(), "dispatched");

    let settled = service
        .settle_definite(SettleDefiniteInput {
            record_id: begun.record.id(),
            outcome: evidence(),
        })
        .await
        .unwrap();
    assert_eq!(settled.record.state().as_str(), "committed");
}

#[tokio::test]
async fn the_factory_builds_an_http_ledger_that_reaches_the_configured_store() {
    let server = MockServer::start().await;
    let record = rigorix_engine::effect_commitment::domain::EffectRecord::begin(intent());
    Mock::given(method("POST"))
        .and(path(BEGIN_PATH))
        .respond_with(ResponseTemplate::new(200).set_body_json(
            rigorix_engine::effect_commitment::application::BeginEffectOutput {
                record: record.clone(),
            },
        ))
        .mount(&server)
        .await;

    let factory = DefaultEffectCommitmentFactory::new();
    let ledger = factory.create(EffectCommitmentBackendConfig::Http {
        base_url: server.uri(),
        timeout_ms: 2_000,
        max_response_body_bytes: 65_536,
        auth_token: None,
    });
    let begun = ledger.begin(intent()).await.unwrap();
    assert_eq!(begun, record);
}

#[tokio::test]
async fn the_factory_builds_a_receipt_reader_that_observes_the_provider() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/receipts/re_1"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "receipt_id": "re_1",
            "status": "succeeded"
        })))
        .mount(&server)
        .await;

    let factory = DefaultEffectCommitmentFactory::new();
    let reader = factory.create_receipt_reader(ProviderReceiptConfig::new(
        server.uri(),
        "stripe",
        2_000,
        65_536,
        None,
    ));
    let outcome = reader.observe(&intent(), Some("re_1")).await.unwrap();
    assert!(outcome.is_committed());
    assert_eq!(outcome.source, "stripe");
}
