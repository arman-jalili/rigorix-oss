//! #1163: the capacity hold and the effect record share one intent identity.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#idempotency
//! Implements: ISSUE-1161 — same intent ⇒ same hold; different amount ⇒ refused
//! Issue: #1163
//!
//! The reservation key is `H(dag_id, node_id, intent_digest)` — the same
//! derivation the effect service applies. Repeating the reserve for the same
//! intent and amount must return the original hold; reusing the key with a
//! different amount must be a distinguishable conflict, never a silent
//! under-reservation.

use rigorix_engine::capacity_ledger::domain::{IdempotencyKey, ResourceId, ResourceSpec, Window};
use rigorix_engine::capacity_ledger::infrastructure::in_process::InProcessLedger;
use rigorix_engine::capacity_ledger::interfaces::CapacityLedger;
use rigorix_engine::effect_commitment::application::key::IntentKeyDerivation;
use rigorix_engine::effect_commitment::application::key_impl::Sha256IntentKeyDerivation;
use rigorix_engine::effect_commitment::domain::EffectIntent;
use uuid::Uuid;

fn intent(dag_id: Uuid, node_id: Uuid, args: &str) -> EffectIntent {
    EffectIntent::new(
        dag_id,
        node_id,
        0,
        IdempotencyKey::new("placeholder"),
        "run_command",
        args,
        "precondition",
        "authority",
    )
}

fn spec() -> ResourceSpec {
    ResourceSpec::new(
        ResourceId::new("gpu"),
        "gpu".to_string(),
        10,
        Window::bounded(60),
    )
}

#[tokio::test]
async fn the_same_intent_finds_the_hold_it_already_owns() {
    let ledger = InProcessLedger::new();
    ledger.declare_resource(spec()).await.expect("declare");

    let key = Sha256IntentKeyDerivation::new().derive_key(&intent(
        Uuid::now_v7(),
        Uuid::now_v7(),
        "echo hello",
    ));
    let first = ledger
        .reserve(&ResourceId::new("gpu"), 4, key.clone())
        .await
        .expect("first reserve");

    // A resumed dispatch derives the same key (the intent did not change).
    let second = ledger
        .reserve(&ResourceId::new("gpu"), 4, key.clone())
        .await
        .expect("retry collapses onto the original hold");

    assert_eq!(first.reservation_id(), second.reservation_id());
    let exposure = ledger
        .exposure(&ResourceId::new("gpu"))
        .await
        .expect("exposure");
    assert_eq!(
        exposure.outstanding, 4,
        "the resource is held once, not twice"
    );
}

#[tokio::test]
async fn the_same_intent_with_a_different_amount_is_refused() {
    let ledger = InProcessLedger::new();
    ledger.declare_resource(spec()).await.expect("declare");

    let key = Sha256IntentKeyDerivation::new().derive_key(&intent(
        Uuid::now_v7(),
        Uuid::now_v7(),
        "echo hello",
    ));
    ledger
        .reserve(&ResourceId::new("gpu"), 4, key.clone())
        .await
        .expect("first reserve");

    let error = ledger
        .reserve(&ResourceId::new("gpu"), 9, key)
        .await
        .expect_err("a different amount is a conflict");
    assert!(
        matches!(
            error,
            rigorix_engine::capacity_ledger::domain::LedgerError::Contended { .. }
        ),
        "different amount must be Contended, got {error:?}"
    );
}
