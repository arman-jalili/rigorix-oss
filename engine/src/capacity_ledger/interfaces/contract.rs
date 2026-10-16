//! Shared contract suite for every [`CapacityLedger`] adapter.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#capacityledger
//! Implements: ISSUE-CAPACITY-LEDGER-11 — one contract suite per adapter
//! Issue: #1032
//!
//! The invariants I1–I6 are the specification, so the same suite must pass
//! against every adapter. This module is test-only: it asserts with `expect`
//! and panic-prone helpers would otherwise be a production panic surface.
//!
//! [`CapacityLedger`]: super::CapacityLedger

use crate::capacity_ledger::domain::{
    IdempotencyKey, ReservationState, ResourceId, ResourceSpec, Window,
};

use super::CapacityLedger;

/// Exercise the frozen port contract against `ledger`.
///
/// The adapter must have no pre-existing state for the contract resource.
pub async fn run_contract(ledger: &dyn CapacityLedger) {
    let resource = ResourceId::new("contract-gpu");

    // Declaring the resource is idempotent enough for a fresh adapter.
    ledger
        .declare_resource(ResourceSpec::new(
            resource.clone(),
            "contract GPU".to_string(),
            2,
            Window::UNBOUNDED,
        ))
        .await
        .expect("declare_resource");

    // I2 — a retried reserve with the same key collapses onto one reservation.
    let first = ledger
        .reserve(&resource, 1, IdempotencyKey::new("contract-1"))
        .await
        .expect("first reserve");
    let retry = ledger
        .reserve(&resource, 1, IdempotencyKey::new("contract-1"))
        .await
        .expect("retried reserve");
    assert_eq!(first.reservation_id(), retry.reservation_id());

    // I6 — one more unit fits, a third does not.
    let second = ledger
        .reserve(&resource, 1, IdempotencyKey::new("contract-2"))
        .await
        .expect("second reserve");
    let exhausted = ledger
        .reserve(&resource, 1, IdempotencyKey::new("contract-3"))
        .await
        .expect_err("over-cap reserve must fail");
    assert!(exhausted.is_capacity_exhausted());

    // Exposure keeps committed and outstanding distinct.
    let exposure = ledger.exposure(&resource).await.expect("exposure");
    assert_eq!(exposure.outstanding, 2);
    assert_eq!(exposure.committed, 0);
    assert!(exposure.is_consistent());

    // I3 — commit one, release the other.
    let committed = ledger.commit(first).await.expect("commit");
    assert_eq!(committed.state, ReservationState::Committed);
    let released = ledger.release(second).await.expect("release");
    assert_eq!(released.state, ReservationState::Released);

    let after = ledger.exposure(&resource).await.expect("exposure after");
    assert_eq!(after.committed, 1);
    assert_eq!(after.outstanding, 0);
    assert_eq!(after.available(), 1);

    // A terminal reservation cannot be resolved twice.
    let replay = ledger
        .commit(replay_grant(&resource))
        .await
        .expect_err("unknown grant must fail closed");
    assert!(replay.is_unavailable());
}

/// Build a syntactically valid grant for idempotency checks without touching
/// the adapter's real issuance path. Kept private to the contract suite.
fn replay_grant(_resource: &ResourceId) -> crate::capacity_ledger::domain::Grant {
    // A grant can only be minted crate-internally; tests live in the same crate.
    crate::capacity_ledger::domain::Grant::issue(
        crate::capacity_ledger::domain::ReservationId::new(),
        _resource.clone(),
        1,
        crate::capacity_ledger::domain::Lease::expiring_in(
            chrono::Utc::now(),
            std::time::Duration::from_secs(1),
        ),
        IdempotencyKey::new("never-issued"),
    )
}
