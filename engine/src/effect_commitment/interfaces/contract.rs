//! Shared contract suite for every [`EffectCommitmentLedger`] adapter.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#effectcommitmentledger
//! Implements: ISSUE-EFFECT-COMMITMENT-15 — one contract suite per adapter
//! Issue: #1128
//!
//! The port invariants are the specification, so the same suite must pass
//! against every adapter. This module is test-only: it asserts with `expect`
//! and panic-prone helpers would otherwise be a production panic surface.
//!
//! [`EffectCommitmentLedger`]: super::EffectCommitmentLedger

use chrono::Utc;
use uuid::Uuid;

use crate::capacity_ledger::domain::IdempotencyKey;
use crate::effect_commitment::domain::{
    CommitmentError, EffectIntent, EffectQuery, EffectRecordId, ObservedOutcome, ResolverIdentity,
    ResolverKind,
};

use super::EffectCommitmentLedger;

fn intent(key: &str, attempt: u32, args: &str) -> EffectIntent {
    EffectIntent::new(
        Uuid::from_u128(1),
        Uuid::from_u128(2),
        attempt,
        IdempotencyKey::new(key),
        "payments.charge",
        args,
        "precondition-digest",
        "authority-digest",
    )
}

fn evidence() -> ObservedOutcome {
    ObservedOutcome::committed("receipt-1", "readback-1", Utc::now(), "stripe")
}

fn provider() -> ResolverIdentity {
    ResolverIdentity::new("stripe", ResolverKind::Provider, "readback-1")
}

/// Exercise the frozen port contract against `ledger`.
///
/// The adapter must have no pre-existing state for the contract keys.
pub async fn run_contract(ledger: &dyn EffectCommitmentLedger) {
    // Write-ahead: begin returns a dispatched record.
    let first = ledger
        .begin(intent("contract-1", 1, "args"))
        .await
        .expect("begin");
    assert_eq!(first.state().as_str(), "dispatched");
    assert!(!first.is_terminal());

    // Custody: same key + same payload reads the existing record.
    let retry = ledger
        .begin(intent("contract-1", 2, "args"))
        .await
        .expect("retry reads the existing record");
    assert_eq!(first.id(), retry.id());

    // Same key + different payload is contended, never a second record.
    let contended = ledger
        .begin(intent("contract-1", 3, "different-args"))
        .await
        .expect_err("same key with a different payload must be refused");
    assert!(matches!(contended, CommitmentError::Contended { .. }));

    // Terminal uniqueness.
    let settled = ledger
        .settle_definite(first.id(), evidence())
        .await
        .expect("settle");
    assert_eq!(settled.state().as_str(), "committed");
    let replay = ledger
        .settle_definite(first.id(), evidence())
        .await
        .expect_err("a terminal record cannot settle again");
    assert!(matches!(replay, CommitmentError::AlreadySettled { .. }));

    // Ambiguity fences, and only external authority with evidence resolves it.
    let second = ledger
        .begin(intent("contract-2", 1, "args"))
        .await
        .expect("begin second");

    // The crash sweep's read side: only the in-flight record is returned.
    let in_flight = ledger
        .query(EffectQuery::dispatched(None))
        .await
        .expect("dispatched query");
    assert_eq!(in_flight, vec![second.clone()]);

    let ambiguous = ledger
        .mark_ambiguous(second.id(), "response lost".to_string())
        .await
        .expect("mark ambiguous");
    assert!(ambiguous.blocks_retry());
    assert!(ambiguous.blocks_dependents());
    let no_longer_in_flight = ledger
        .query(EffectQuery::dispatched(None))
        .await
        .expect("dispatched query after mark");
    assert!(no_longer_in_flight.is_empty());

    let pending = ledger
        .query(EffectQuery::ambiguous(None))
        .await
        .expect("ambiguous query");
    assert_eq!(pending, vec![ambiguous.clone()]);

    let rejected = ledger
        .resolve(
            second.id(),
            evidence(),
            ResolverIdentity::new("agent", ResolverKind::Agent, "readback-1"),
        )
        .await
        .expect_err("the agent is not a resolver");
    assert!(matches!(
        rejected,
        CommitmentError::UnauthorizedResolver { .. }
    ));

    let resolved = ledger
        .resolve(second.id(), evidence(), provider())
        .await
        .expect("provider evidence resolves");
    assert_eq!(resolved.state().as_str(), "committed");
    assert_eq!(resolved.resolver(), Some(&provider()));

    let still_pending = ledger
        .query(EffectQuery::ambiguous(None))
        .await
        .expect("ambiguous query after resolve");
    assert!(still_pending.is_empty());

    // Read side by id and DAG.
    let by_id = ledger
        .query(EffectQuery::by_id(second.id()))
        .await
        .expect("query by id");
    assert_eq!(by_id, vec![resolved]);
    let by_dag = ledger
        .query(EffectQuery::by_dag(Uuid::from_u128(1)))
        .await
        .expect("query by dag");
    assert_eq!(by_dag.len(), 2);

    // Unknown records fail closed.
    let unknown = ledger
        .settle_definite(EffectRecordId::new(), evidence())
        .await
        .expect_err("unknown record must fail closed");
    assert!(matches!(unknown, CommitmentError::Unavailable { .. }));

    // A terminal record cannot be marked ambiguous.
    let terminal = ledger
        .mark_ambiguous(first.id(), "late".to_string())
        .await
        .expect_err("a terminal record cannot become ambiguous");
    assert!(matches!(terminal, CommitmentError::AlreadySettled { .. }));
}
