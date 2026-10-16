//! InProcessLedger adapter.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#inprocessledger
//! Implements: ISSUE-CAPACITY-LEDGER-9 — standalone in-process ledger
//! Issue: #1030
//! ADR-018: distributed capacity ledger
//! ADR-008: RAII reservation pattern
//!
//! The standalone default. Single-execution atomicity via a single mutex over
//! all state: every reserve/commit/release/re-exposure is linearized. No
//! database, no network.
//!
//! # Guarantee ceiling (Frozen)
//! This adapter provides **single-execution atomicity only**. It MUST NOT be
//! described as "the distributed ledger"; cross-session atomicity requires the
//! HTTP adapter against a real contested store.
//!
//! # Invariants implemented
//! - **I1** one mutex linearizes every operation per process
//! - **I2** `(resource, idempotency key)` maps to at most one reservation
//! - **I3** every grant carries a lease and lapsed leases are reclaimed
//! - **I5** expiry uses the store clock (`Utc::now`), never the caller's
//! - **I6** `committed + outstanding` never exceeds `hard_limit`

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::capacity_ledger::domain::{
    Exposure, Grant, IdempotencyKey, LedgerError, Reservation, ReservationId, ReservationState,
    ResourceId, ResourceSpec, Window,
};
use crate::capacity_ledger::interfaces::CapacityLedger;

/// Default lease for a reservation when the caller does not tune it.
const DEFAULT_LEASE_TTL: Duration = Duration::from_secs(60);

/// All ledger state, guarded by one mutex (I1).
#[derive(Default)]
struct LedgerState {
    specs: HashMap<ResourceId, ResourceSpec>,
    reservations: HashMap<ReservationId, Reservation>,
    /// `(resource, idempotency key) -> reservation` — the I2 index.
    by_key: HashMap<(ResourceId, IdempotencyKey), ReservationId>,
    committed: HashMap<ResourceId, u64>,
    outstanding: HashMap<ResourceId, u64>,
}

impl LedgerState {
    /// Reclaim any outstanding reservation whose lease has lapsed (I3/I5).
    fn expire_lapsed(&mut self, now: DateTime<Utc>) {
        let lapsed: Vec<ReservationId> = self
            .reservations
            .values()
            .filter(|r| r.state == ReservationState::Outstanding && r.is_expired_at(now))
            .map(|r| r.id)
            .collect();

        for id in lapsed {
            if let Some(mut reservation) = self.reservations.remove(&id) {
                reservation.state = ReservationState::Expired;
                if let Some(outstanding) = self.outstanding.get_mut(&reservation.resource) {
                    *outstanding = outstanding.saturating_sub(reservation.amount);
                }
                // The claim is void, so the key may be retried afresh.
                self.by_key.remove(&(
                    reservation.resource.clone(),
                    reservation.idempotency_key.clone(),
                ));
                self.reservations.insert(id, reservation);
            }
        }
    }

    fn snapshot(&self, resource: &ResourceId) -> Exposure {
        let spec = self.specs.get(resource);
        let hard_limit = spec.map(|s| s.hard_limit).unwrap_or(0);
        let window = spec.map(|s| s.window).unwrap_or(Window::UNBOUNDED);
        Exposure::new(
            resource.clone(),
            hard_limit,
            self.committed.get(resource).copied().unwrap_or(0),
            self.outstanding.get(resource).copied().unwrap_or(0),
            window,
        )
    }
}

/// The in-process capacity-ledger adapter.
pub struct InProcessLedger {
    state: Mutex<LedgerState>,
    lease_ttl: Duration,
}

impl InProcessLedger {
    /// Create an empty in-process ledger with the default lease.
    pub fn new() -> Self {
        Self::with_lease_ttl(DEFAULT_LEASE_TTL)
    }

    /// Create an empty in-process ledger with a custom lease TTL.
    pub fn with_lease_ttl(lease_ttl: Duration) -> Self {
        Self {
            state: Mutex::new(LedgerState::default()),
            lease_ttl,
        }
    }

    fn locked(&self) -> Result<std::sync::MutexGuard<'_, LedgerState>, LedgerError> {
        self.state.lock().map_err(|_| LedgerError::Unavailable {
            detail: "in-process capacity ledger poisoned".to_string(),
        })
    }

    fn grant_from(reservation: &Reservation) -> Grant {
        Grant::issue(
            reservation.id,
            reservation.resource.clone(),
            reservation.amount,
            reservation.lease,
            reservation.idempotency_key.clone(),
        )
    }
}

impl Default for InProcessLedger {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CapacityLedger for InProcessLedger {
    async fn declare_resource(&self, spec: ResourceSpec) -> Result<(), LedgerError> {
        let now = Utc::now();
        let mut state = self.locked()?;
        state.expire_lapsed(now);

        let existing = state.snapshot(&spec.id);
        let claimed = existing.committed.saturating_add(existing.outstanding);
        if claimed > spec.hard_limit {
            return Err(LedgerError::Insufficient {
                resource: spec.id.clone(),
                requested: claimed,
                available: spec.hard_limit,
            });
        }
        state.specs.insert(spec.id.clone(), spec);
        Ok(())
    }

    async fn reserve(
        &self,
        resource: &ResourceId,
        amount: u64,
        key: IdempotencyKey,
    ) -> Result<Grant, LedgerError> {
        let now = Utc::now();
        let mut state = self.locked()?;
        state.expire_lapsed(now);

        let spec = state
            .specs
            .get(resource)
            .cloned()
            .ok_or_else(|| LedgerError::Unavailable {
                detail: format!("resource '{resource}' has not been declared"),
            })?;

        // I2: a retried reserve collapses onto the original reservation.
        if let Some(existing_id) = state.by_key.get(&(resource.clone(), key.clone())).copied() {
            return match state.reservations.get(&existing_id) {
                Some(reservation) if reservation.state == ReservationState::Outstanding => {
                    Ok(Self::grant_from(reservation))
                }
                _ => Err(LedgerError::Contended {
                    resource: resource.clone(),
                    detail: "idempotency key already resolved".to_string(),
                }),
            };
        }

        let committed = state.committed.get(resource).copied().unwrap_or(0);
        let outstanding = state.outstanding.get(resource).copied().unwrap_or(0);
        let available = spec
            .hard_limit
            .saturating_sub(committed)
            .saturating_sub(outstanding);
        if amount > available {
            return Err(LedgerError::Insufficient {
                resource: resource.clone(),
                requested: amount,
                available,
            });
        }

        let reservation = Reservation::new(
            ReservationId::new(),
            resource.clone(),
            amount,
            crate::capacity_ledger::domain::Lease::expiring_in(now, self.lease_ttl),
            key.clone(),
        );
        let grant = Self::grant_from(&reservation);

        state.by_key.insert((resource.clone(), key), reservation.id);
        state
            .outstanding
            .insert(resource.clone(), outstanding + amount);
        state.reservations.insert(reservation.id, reservation);

        Ok(grant)
    }

    async fn commit(&self, grant: Grant) -> Result<Reservation, LedgerError> {
        self.resolve(grant, ReservationState::Committed)
    }

    async fn release(&self, grant: Grant) -> Result<Reservation, LedgerError> {
        self.resolve(grant, ReservationState::Released)
    }

    async fn exposure(&self, resource: &ResourceId) -> Result<Exposure, LedgerError> {
        let now = Utc::now();
        let mut state = self.locked()?;
        state.expire_lapsed(now);
        Ok(state.snapshot(resource))
    }
}

impl InProcessLedger {
    /// Shared commit/release transition, enforcing terminal state and the store clock.
    fn resolve(&self, grant: Grant, to: ReservationState) -> Result<Reservation, LedgerError> {
        let (id, resource, amount, _lease, key) = grant.into_parts();
        let now = Utc::now();
        let mut state = self.locked()?;
        state.expire_lapsed(now);

        let mut reservation =
            state
                .reservations
                .get(&id)
                .cloned()
                .ok_or_else(|| LedgerError::Unavailable {
                    detail: format!("unknown reservation {id}"),
                })?;

        if reservation.state != ReservationState::Outstanding {
            return Err(LedgerError::Contended {
                resource: resource.clone(),
                detail: format!("reservation {id} is already {:?}", reservation.state),
            });
        }

        if to == ReservationState::Committed && reservation.is_expired_at(now) {
            reservation.state = ReservationState::Expired;
            state.reservations.insert(id, reservation);
            return Err(LedgerError::LeaseExpired { resource, key });
        }

        reservation.state = to;
        if let Some(outstanding) = state.outstanding.get_mut(&resource) {
            *outstanding = outstanding.saturating_sub(amount);
        }
        if to == ReservationState::Committed {
            *state.committed.entry(resource).or_insert(0) += amount;
        }
        state.reservations.insert(id, reservation.clone());
        Ok(reservation)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::capacity_ledger::domain::{IdempotencyKey, Window};

    fn spec(limit: u64) -> ResourceSpec {
        ResourceSpec::new(
            ResourceId::new("gpu"),
            "GPU".to_string(),
            limit,
            Window::UNBOUNDED,
        )
    }

    async fn declared(limit: u64) -> InProcessLedger {
        let ledger = InProcessLedger::new();
        ledger.declare_resource(spec(limit)).await.unwrap();
        ledger
    }

    #[tokio::test]
    async fn reserve_then_commit_moves_outstanding_to_committed() {
        let ledger = declared(10).await;
        let resource = ResourceId::new("gpu");

        let grant = ledger
            .reserve(&resource, 3, IdempotencyKey::new("k1"))
            .await
            .unwrap();
        let after_reserve = ledger.exposure(&resource).await.unwrap();
        assert_eq!(after_reserve.outstanding, 3);

        let committed = ledger.commit(grant).await.unwrap();
        assert_eq!(committed.state, ReservationState::Committed);
        let after_commit = ledger.exposure(&resource).await.unwrap();
        assert_eq!(after_commit.committed, 3);
        assert_eq!(after_commit.outstanding, 0);
    }

    #[tokio::test]
    async fn release_returns_capacity_immediately() {
        let ledger = declared(10).await;
        let resource = ResourceId::new("gpu");
        let grant = ledger
            .reserve(&resource, 4, IdempotencyKey::new("k1"))
            .await
            .unwrap();
        ledger.release(grant).await.unwrap();
        let exposure = ledger.exposure(&resource).await.unwrap();
        assert_eq!(exposure.outstanding, 0);
        assert_eq!(exposure.available(), 10);
    }

    #[tokio::test]
    async fn retried_reserve_with_same_key_does_not_double_reserve() {
        let ledger = declared(10).await;
        let resource = ResourceId::new("gpu");

        let first = ledger
            .reserve(&resource, 4, IdempotencyKey::new("same"))
            .await
            .unwrap();
        let retry = ledger
            .reserve(&resource, 4, IdempotencyKey::new("same"))
            .await
            .unwrap();

        assert_eq!(first.reservation_id(), retry.reservation_id());
        assert_eq!(ledger.exposure(&resource).await.unwrap().outstanding, 4);
    }

    #[tokio::test]
    async fn reserve_beyond_the_cap_fails_insufficient() {
        let ledger = declared(5).await;
        let resource = ResourceId::new("gpu");
        ledger
            .reserve(&resource, 4, IdempotencyKey::new("k1"))
            .await
            .unwrap();
        let err = ledger
            .reserve(&resource, 2, IdempotencyKey::new("k2"))
            .await
            .unwrap_err();
        assert!(matches!(err, LedgerError::Insufficient { .. }));
    }

    #[tokio::test]
    async fn lapsed_lease_is_reclaimed_by_the_store_clock() {
        let ledger = InProcessLedger::with_lease_ttl(Duration::from_millis(250));
        ledger.declare_resource(spec(1)).await.unwrap();
        let resource = ResourceId::new("gpu");
        ledger
            .reserve(&resource, 1, IdempotencyKey::new("k1"))
            .await
            .unwrap();

        // Wait for k1's lease to lapse, but leave k2 a comfortable margin.
        tokio::time::sleep(Duration::from_millis(400)).await;

        // The next operation sweeps the lapsed hold, so another reserve fits.
        let grant = ledger
            .reserve(&resource, 1, IdempotencyKey::new("k2"))
            .await
            .unwrap();
        assert_eq!(grant.amount(), 1);
        assert_eq!(ledger.exposure(&resource).await.unwrap().outstanding, 1);
    }

    #[tokio::test]
    async fn concurrent_reservations_never_exceed_the_hard_limit() {
        let ledger = Arc::new(declared(10).await);
        let resource = ResourceId::new("gpu");

        let mut handles = Vec::new();
        for i in 0..50u32 {
            let ledger = ledger.clone();
            let resource = resource.clone();
            handles.push(tokio::spawn(async move {
                ledger
                    .reserve(&resource, 1, IdempotencyKey::new(format!("k{i}")))
                    .await
                    .is_ok()
            }));
        }

        let mut granted = 0;
        for handle in handles {
            if handle.await.unwrap() {
                granted += 1;
            }
        }

        assert_eq!(granted, 10, "exactly the hard limit may be granted");
        let exposure = ledger.exposure(&resource).await.unwrap();
        assert_eq!(exposure.outstanding, 10);
        assert!(exposure.is_consistent());
    }

    #[tokio::test]
    async fn satisfies_the_frozen_port_contract() {
        let ledger = InProcessLedger::new();
        crate::capacity_ledger::interfaces::contract::run_contract(&ledger).await;
    }
}
