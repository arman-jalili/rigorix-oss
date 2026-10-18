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
//! This adapter provides **single-process atomicity**: every execution in this
//! process that shares the instance contends on the same aggregate. It MUST NOT
//! be described as "the distributed ledger"; cross-session atomicity (across
//! processes or replicas) requires the HTTP adapter against a real contested
//! store. Proven at composition level by
//! `two_concurrent_executions_share_the_in_process_aggregate`.
//!
//! # Invariants implemented
//! - **I1** one mutex linearizes every operation per process
//! - **I2** `(resource, idempotency key)` maps to at most one reservation; the
//!   same key with a different amount is a typed conflict, never a silent hit
//! - **I3** every grant carries a lease; a lapsed lease becomes `Ambiguous` and
//!   retains exposure until a late commit/release reconciles it
//! - **I5** expiry uses the store clock (`Utc::now`), never the caller's
//! - **I6** `committed + outstanding` never exceeds `hard_limit`

use std::collections::{HashMap, VecDeque};
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
    /// Timestamped commit amounts, per resource, oldest first. Only commits
    /// inside the resource's window count toward exposure (#1051).
    committed: HashMap<ResourceId, VecDeque<(DateTime<Utc>, u64)>>,
    outstanding: HashMap<ResourceId, u64>,
}

impl LedgerState {
    /// Sweep lapsed holds (I3/I5).
    ///
    /// A lapsed lease is **not** proof that no effect happened (#1050), so the
    /// reservation becomes `Ambiguous` and its exposure is retained until a late
    /// commit (proved success) or release (proved no effect) reconciles it. The
    /// idempotency key stays bound to the original reservation so a retry cannot
    /// silently create a second one (#1052).
    fn expire_lapsed(&mut self, now: DateTime<Utc>) {
        let lapsed: Vec<ReservationId> = self
            .reservations
            .values()
            .filter(|r| r.state == ReservationState::Outstanding && r.is_expired_at(now))
            .map(|r| r.id)
            .collect();

        for id in lapsed {
            if let Some(reservation) = self.reservations.get_mut(&id) {
                reservation.state = ReservationState::Ambiguous;
                tracing::warn!(
                    reservation = %id,
                    resource = %reservation.resource,
                    amount = reservation.amount,
                    "capacity lease lapsed; outcome unknown, exposure retained until reconciliation"
                );
            }
        }
    }

    /// Sum committed amounts still inside `window`, pruning older entries.
    ///
    /// A rolling-duration window (#1051): capacity committed more than W seconds
    /// ago no longer counts, so the cap is per trailing window rather than for
    /// the resource's lifetime. An unbounded window never prunes.
    fn committed_in_window(
        &mut self,
        resource: &ResourceId,
        window: Window,
        now: DateTime<Utc>,
    ) -> u64 {
        let Some(entries) = self.committed.get_mut(resource) else {
            return 0;
        };
        if window.is_bounded() {
            let delta =
                chrono::Duration::from_std(std::time::Duration::from_secs(window.duration_secs))
                    .unwrap_or(chrono::Duration::MAX);
            let cutoff = now - delta;
            while entries.front().is_some_and(|(at, _)| *at <= cutoff) {
                entries.pop_front();
            }
        }
        entries.iter().map(|(_, amount)| *amount).sum()
    }

    fn snapshot(&mut self, resource: &ResourceId) -> Exposure {
        let now = Utc::now();
        let spec = self.specs.get(resource).cloned();
        let hard_limit = spec.as_ref().map(|s| s.hard_limit).unwrap_or(0);
        let window = spec.as_ref().map(|s| s.window).unwrap_or(Window::UNBOUNDED);
        let committed = self.committed_in_window(resource, window, now);
        let outstanding = self.outstanding.get(resource).copied().unwrap_or(0);
        Exposure::new(resource.clone(), hard_limit, committed, outstanding, window)
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

        // Validate against the *new* window, so re-declaring with a different
        // period is judged on the period it will actually enforce.
        let committed = state.committed_in_window(&spec.id, spec.window, now);
        let outstanding = state.outstanding.get(&spec.id).copied().unwrap_or(0);
        let claimed = committed.saturating_add(outstanding);
        if claimed > spec.hard_limit {
            return Err(LedgerError::Insufficient {
                resource: spec.id.clone(),
                requested: claimed,
                available: spec.hard_limit,
            });
        }
        tracing::info!(
            resource = %spec.id,
            hard_limit = spec.hard_limit,
            "capacity resource declared"
        );
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

        // I2: a retried reserve collapses onto the original reservation — but
        // only for the identical request. A different amount is a conflict, not
        // an idempotent hit (#1052).
        if let Some(existing_id) = state.by_key.get(&(resource.clone(), key.clone())).copied() {
            return match state.reservations.get(&existing_id) {
                Some(reservation) if reservation.state == ReservationState::Outstanding => {
                    if reservation.amount == amount {
                        Ok(Self::grant_from(reservation))
                    } else {
                        Err(LedgerError::Contended {
                            resource: resource.clone(),
                            detail: format!(
                                "idempotency key reused with a different amount (existing {}, requested {})",
                                reservation.amount, amount
                            ),
                        })
                    }
                }
                Some(reservation) if reservation.state == ReservationState::Ambiguous => {
                    Err(LedgerError::Contended {
                        resource: resource.clone(),
                        detail: format!(
                            "idempotency key refers to ambiguous reservation {existing_id}; commit or release it before retrying"
                        ),
                    })
                }
                _ => Err(LedgerError::Contended {
                    resource: resource.clone(),
                    detail: "idempotency key already resolved".to_string(),
                }),
            };
        }

        let committed = state.committed_in_window(resource, spec.window, now);
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

        tracing::debug!(
            resource = %resource,
            amount,
            reservation = %reservation.id,
            "capacity reserved"
        );
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

        if reservation.state == ReservationState::Expired {
            return Err(LedgerError::LeaseExpired { resource, key });
        }
        // `Ambiguous` is reconciled by a late commit or release. `expire_lapsed`
        // above is what moves an expired `Outstanding` hold into that state.
        if !matches!(
            reservation.state,
            ReservationState::Outstanding | ReservationState::Ambiguous
        ) {
            return Err(LedgerError::Contended {
                resource: resource.clone(),
                detail: format!("reservation {id} is already {:?}", reservation.state),
            });
        }

        reservation.state = to;
        if let Some(outstanding) = state.outstanding.get_mut(&resource) {
            *outstanding = outstanding.saturating_sub(amount);
        }
        if to == ReservationState::Committed {
            state
                .committed
                .entry(resource)
                .or_default()
                .push_back((now, amount));
        }
        tracing::debug!(reservation = %id, state = ?to, "capacity reservation resolved");
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
    async fn lapsed_lease_becomes_ambiguous_and_retains_exposure() {
        let ledger = InProcessLedger::with_lease_ttl(Duration::from_millis(250));
        ledger.declare_resource(spec(1)).await.unwrap();
        let resource = ResourceId::new("gpu");
        let grant = ledger
            .reserve(&resource, 1, IdempotencyKey::new("k1"))
            .await
            .unwrap();
        let id = grant.reservation_id();

        tokio::time::sleep(Duration::from_millis(400)).await;

        // A lapsed lease is not proof of no effect: exposure is retained.
        assert_eq!(ledger.exposure(&resource).await.unwrap().outstanding, 1);

        // Another caller cannot steal the ambiguous hold.
        let err = ledger
            .reserve(&resource, 1, IdempotencyKey::new("k2"))
            .await
            .unwrap_err();
        assert!(matches!(err, LedgerError::Insufficient { .. }));

        // A late commit proves the effect happened and reconciles the hold.
        let committed = ledger.commit(grant).await.unwrap();
        assert_eq!(committed.id, id);
        assert_eq!(committed.state, ReservationState::Committed);
        let after = ledger.exposure(&resource).await.unwrap();
        assert_eq!(after.committed, 1);
        assert_eq!(after.outstanding, 0);
    }

    #[tokio::test]
    async fn reusing_a_key_with_a_different_amount_is_a_conflict() {
        let ledger = declared(10).await;
        let resource = ResourceId::new("gpu");
        let grant = ledger
            .reserve(&resource, 4, IdempotencyKey::new("k1"))
            .await
            .unwrap();
        assert_eq!(grant.amount(), 4);

        let err = ledger
            .reserve(&resource, 999, IdempotencyKey::new("k1"))
            .await
            .unwrap_err();
        assert!(
            matches!(err, LedgerError::Contended { .. }),
            "different amount must not return the 4-unit grant: {err:?}"
        );

        // The identical request still collapses onto the original reservation.
        let same = ledger
            .reserve(&resource, 4, IdempotencyKey::new("k1"))
            .await
            .unwrap();
        assert_eq!(same.reservation_id(), grant.reservation_id());
    }

    #[tokio::test]
    async fn retry_after_expiry_does_not_create_a_second_reservation() {
        let ledger = InProcessLedger::with_lease_ttl(Duration::from_millis(100));
        ledger.declare_resource(spec(10)).await.unwrap();
        let resource = ResourceId::new("gpu");
        ledger
            .reserve(&resource, 4, IdempotencyKey::new("k1"))
            .await
            .unwrap();

        tokio::time::sleep(Duration::from_millis(200)).await;

        // The key is still bound to the ambiguous reservation, so a retry is
        // rejected instead of silently reserving a second 4 units.
        let err = ledger
            .reserve(&resource, 4, IdempotencyKey::new("k1"))
            .await
            .unwrap_err();
        assert!(matches!(err, LedgerError::Contended { .. }));
        assert_eq!(ledger.exposure(&resource).await.unwrap().outstanding, 4);
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
    async fn committed_exposure_ages_out_of_a_bounded_window() {
        let ledger = InProcessLedger::new();
        let resource = ResourceId::new("gpu");
        ledger
            .declare_resource(ResourceSpec::new(
                resource.clone(),
                "GPU".to_string(),
                100,
                Window::bounded(1),
            ))
            .await
            .unwrap();

        let grant = ledger
            .reserve(&resource, 100, IdempotencyKey::new("k1"))
            .await
            .unwrap();
        ledger.commit(grant).await.unwrap();

        let full = ledger.exposure(&resource).await.unwrap();
        assert_eq!(full.committed, 100);
        assert_eq!(full.available(), 0);
        assert!(matches!(
            ledger
                .reserve(&resource, 1, IdempotencyKey::new("k2"))
                .await
                .unwrap_err(),
            LedgerError::Insufficient { .. }
        ));

        tokio::time::sleep(Duration::from_millis(1100)).await;

        // The trailing window has rolled, so the committed capacity is available
        // again — a per-hour cap is not a lifetime cap (#1051).
        let aged = ledger.exposure(&resource).await.unwrap();
        assert_eq!(aged.committed, 0);
        assert_eq!(aged.available(), 100);
        ledger
            .reserve(&resource, 1, IdempotencyKey::new("k3"))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn unbounded_window_never_ages_out() {
        let ledger = declared(10).await;
        let resource = ResourceId::new("gpu");
        let grant = ledger
            .reserve(&resource, 5, IdempotencyKey::new("k1"))
            .await
            .unwrap();
        ledger.commit(grant).await.unwrap();

        tokio::time::sleep(Duration::from_millis(50)).await;
        assert_eq!(ledger.exposure(&resource).await.unwrap().committed, 5);
    }

    #[tokio::test]
    async fn satisfies_the_frozen_port_contract() {
        let ledger = InProcessLedger::new();
        crate::capacity_ledger::interfaces::contract::run_contract(&ledger).await;
    }
}
