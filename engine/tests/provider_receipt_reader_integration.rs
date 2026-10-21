//! ProviderReceiptReader integration: L3 evidence settles an ambiguous effect.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#providerreceiptreader
//! Implements: ISSUE-EFFECT-COMMITMENT-12 — upstream/downstream integration
//! Issue: #1125
//! ADR-019: without provider evidence the context can prove not-settled only

use rigorix_engine::capacity_ledger::domain::IdempotencyKey;
use rigorix_engine::effect_commitment::application::{
    ProviderReceiptConfig, ProviderReceiptReader,
};
use rigorix_engine::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, ResolverIdentity, ResolverKind,
};
use rigorix_engine::effect_commitment::infrastructure::{
    HttpProviderReceiptReader, InMemoryEffectLedger,
};
use rigorix_engine::effect_commitment::interfaces::EffectCommitmentLedger;
use serde_json::json;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

fn intent() -> EffectIntent {
    EffectIntent::new(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        1,
        IdempotencyKey::new("durable-intent-key"),
        "payments.charge",
        "args-digest",
        "precondition-digest",
        "authority-digest",
    )
}

fn reader(server: &MockServer) -> HttpProviderReceiptReader {
    HttpProviderReceiptReader::new(ProviderReceiptConfig::new(
        server.uri(),
        "stripe",
        2_000,
        65_536,
        None,
    ))
}

async fn ambiguous_effect(
    ledger: &InMemoryEffectLedger,
) -> rigorix_engine::effect_commitment::domain::EffectRecord {
    let record = ledger.begin(intent()).await.unwrap();
    ledger
        .mark_ambiguous(record.id(), "response lost".to_string())
        .await
        .unwrap()
}

#[tokio::test]
async fn a_provider_success_resolves_the_effect_as_committed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/receipts/re_123"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "receipt_id": "re_123",
            "status": "succeeded",
            "observed_at": "2026-10-21T12:00:00Z"
        })))
        .mount(&server)
        .await;

    let evidence = reader(&server)
        .observe(&intent(), Some("re_123"))
        .await
        .unwrap();
    assert!(evidence.is_committed());
    assert!(evidence.is_complete());

    let ledger = InMemoryEffectLedger::new();
    let ambiguous = ambiguous_effect(&ledger).await;
    let resolved = ledger
        .resolve(
            ambiguous.id(),
            evidence.clone(),
            ResolverIdentity::new(
                "stripe",
                ResolverKind::Provider,
                evidence.readback_digest.clone(),
            ),
        )
        .await
        .unwrap();

    assert_eq!(resolved.state().as_str(), "committed");
    assert_eq!(resolved.outcome(), Some(&evidence));
    assert!(
        ledger
            .query(EffectQuery::ambiguous(None))
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn a_provider_failure_resolves_the_effect_as_not_committed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/receipts/re_500"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "receipt_id": "re_500",
            "status": "declined"
        })))
        .mount(&server)
        .await;

    let evidence = reader(&server)
        .observe(&intent(), Some("re_500"))
        .await
        .unwrap();
    assert_eq!(evidence.terminal_state().as_str(), "not_committed");

    let ledger = InMemoryEffectLedger::new();
    let ambiguous = ambiguous_effect(&ledger).await;
    let resolved = ledger
        .resolve(
            ambiguous.id(),
            evidence.clone(),
            ResolverIdentity::new(
                "stripe",
                ResolverKind::Provider,
                evidence.readback_digest.clone(),
            ),
        )
        .await
        .unwrap();
    assert_eq!(resolved.state().as_str(), "not_committed");
}

#[tokio::test]
async fn an_unsettled_provider_response_leaves_the_effect_ambiguous() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/v1/receipts/re_pending"))
        .respond_with(ResponseTemplate::new(200).set_body_json(json!({
            "receipt_id": "re_pending",
            "status": "pending"
        })))
        .mount(&server)
        .await;

    let ledger = InMemoryEffectLedger::new();
    let ambiguous = ambiguous_effect(&ledger).await;

    let err = reader(&server)
        .observe(&intent(), Some("re_pending"))
        .await
        .unwrap_err();
    assert!(matches!(err, CommitmentError::Unavailable { .. }));

    // Fail-closed: the effect is still ambiguous, not silently resolved.
    let still_pending = ledger
        .query(EffectQuery::by_id(ambiguous.id()))
        .await
        .unwrap();
    assert_eq!(still_pending, vec![ambiguous.clone()]);
    assert!(still_pending[0].blocks_retry());
    assert_eq!(still_pending[0].updated_at(), ambiguous.updated_at());
}
