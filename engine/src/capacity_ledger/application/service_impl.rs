//! Concrete CapacityLedgerService implementation.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#capacityledgerservice
//! Implements: ISSUE-CAPACITY-LEDGER-7 — reserve/commit/release/exposure use cases
//! Issue: #1028
//!
//! `CapacityLedgerServiceImpl` is a thin use-case layer over the
//! [`CapacityLedger`] port. It holds **no capacity counter** — the port is the
//! single source of truth. Its only state is the set of outstanding guards, so
//! a caller can commit or release by reservation id.
//!
//! # Ownership (single owner)
//! The service owns every outstanding [`ReservationGuardImpl`]; no DTO hands a
//! guard to the caller. `commit(id)`/`release(id)` take the guard out of the
//! registry and settle through it. Dropping the service drops every remaining
//! guard, whose `Drop` releases the hold — RAII at service lifetime, with the
//! lease as the backstop if the process dies (ADR-008, ADR-018 I3).
//!
//! # Contract notes
//! - The guard registry is keyed by reservation id; a guard is removed before
//!   the async port call, so the mutex is never held across an `await`.
//! - A poisoned registry or an unknown reservation maps to
//!   [`LedgerError::Unavailable`] (fail closed, I4).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::capacity_ledger::domain::{
    Exposure, Grant, LedgerError, Reservation, ReservationId, ReservationState,
};
use crate::capacity_ledger::interfaces::CapacityLedger;

use super::dto::{
    CommitCapacityInput, CommitCapacityOutput, ExposureOutput, ExposureQuery, ReleaseCapacityInput,
    ReleaseCapacityOutput, ReserveCapacityInput, ReserveCapacityOutput,
};
use super::guard::ReservationGuard;
use super::guard_impl::ReservationGuardImpl;
use super::service::CapacityLedgerService;

/// Use-case service over a [`CapacityLedger`] port.
pub struct CapacityLedgerServiceImpl {
    ledger: Arc<dyn CapacityLedger>,
    guards: Mutex<HashMap<ReservationId, ReservationGuardImpl>>,
}

impl CapacityLedgerServiceImpl {
    /// Wrap a ledger port.
    pub fn new(ledger: Arc<dyn CapacityLedger>) -> Self {
        Self {
            ledger,
            guards: Mutex::new(HashMap::new()),
        }
    }

    /// Take the guard for `id`, if it is still outstanding.
    fn take_guard(&self, id: &ReservationId) -> Result<ReservationGuardImpl, LedgerError> {
        let mut guards = self.guards.lock().map_err(|_| LedgerError::Unavailable {
            detail: "capacity guard registry poisoned".to_string(),
        })?;
        guards.remove(id).ok_or_else(|| LedgerError::Unavailable {
            detail: format!("no outstanding guard for reservation {id}"),
        })
    }
}

/// Snapshot the reservation a grant proves (it is always still outstanding).
fn reservation_from_grant(grant: &Grant) -> Reservation {
    Reservation {
        id: grant.reservation_id(),
        resource: grant.resource().clone(),
        amount: grant.amount(),
        state: ReservationState::Outstanding,
        lease: *grant.lease(),
        idempotency_key: grant.idempotency_key().clone(),
    }
}

#[async_trait]
impl CapacityLedgerService for CapacityLedgerServiceImpl {
    async fn reserve(
        &self,
        input: ReserveCapacityInput,
    ) -> Result<ReserveCapacityOutput, LedgerError> {
        let grant = self
            .ledger
            .reserve(&input.resource, input.amount, input.idempotency_key)
            .await?;
        let reservation = reservation_from_grant(&grant);
        let exposure: Exposure = self.ledger.exposure(&reservation.resource).await?;

        // The service owns the guard, so commit/release-by-id can find it and
        // dropping the service releases it. The lock is released at the end of
        // this statement, before any further await.
        let guard = ReservationGuardImpl::new(self.ledger.clone(), grant);
        self.guards
            .lock()
            .map_err(|_| LedgerError::Unavailable {
                detail: "capacity guard registry poisoned".to_string(),
            })?
            .insert(reservation.id, guard);

        Ok(ReserveCapacityOutput {
            reservation,
            exposure,
        })
    }

    async fn commit(
        &self,
        input: CommitCapacityInput,
    ) -> Result<CommitCapacityOutput, LedgerError> {
        let guard = self.take_guard(&input.reservation_id)?;
        let reservation = Box::new(guard).commit().await?;
        let exposure = self.ledger.exposure(&reservation.resource).await?;
        Ok(CommitCapacityOutput {
            reservation,
            exposure,
        })
    }

    async fn release(
        &self,
        input: ReleaseCapacityInput,
    ) -> Result<ReleaseCapacityOutput, LedgerError> {
        let guard = self.take_guard(&input.reservation_id)?;
        let reservation = Box::new(guard).release().await?;
        let exposure = self.ledger.exposure(&reservation.resource).await?;
        Ok(ReleaseCapacityOutput {
            reservation,
            exposure,
        })
    }

    async fn exposure(&self, input: ExposureQuery) -> Result<ExposureOutput, LedgerError> {
        let exposure = self.ledger.exposure(&input.resource).await?;
        Ok(ExposureOutput { exposure })
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use chrono::Utc;

    use super::*;
    use crate::capacity_ledger::domain::{IdempotencyKey, Lease, ResourceId, ResourceSpec, Window};

    /// Minimal in-memory port used to exercise the service without a real adapter.
    struct FakeLedger {
        hard_limit: u64,
        committed: Mutex<u64>,
        outstanding: Mutex<u64>,
    }

    impl FakeLedger {
        fn new(hard_limit: u64) -> Self {
            Self {
                hard_limit,
                committed: Mutex::new(0),
                outstanding: Mutex::new(0),
            }
        }
    }

    fn poisoned() -> LedgerError {
        LedgerError::Unavailable {
            detail: "fake ledger poisoned".to_string(),
        }
    }

    #[async_trait]
    impl CapacityLedger for FakeLedger {
        async fn declare_resource(&self, _spec: ResourceSpec) -> Result<(), LedgerError> {
            Ok(())
        }

        async fn reserve(
            &self,
            resource: &ResourceId,
            amount: u64,
            key: IdempotencyKey,
        ) -> Result<Grant, LedgerError> {
            let mut outstanding = self.outstanding.lock().map_err(|_| poisoned())?;
            let committed = *self.committed.lock().map_err(|_| poisoned())?;
            let taken = committed.saturating_add(*outstanding);
            if taken.saturating_add(amount) > self.hard_limit {
                return Err(LedgerError::Insufficient {
                    resource: resource.clone(),
                    requested: amount,
                    available: self.hard_limit.saturating_sub(taken),
                });
            }
            *outstanding += amount;
            Ok(Grant::issue(
                ReservationId::new(),
                resource.clone(),
                amount,
                Lease::expiring_in(Utc::now(), Duration::from_secs(60)),
                key,
            ))
        }

        async fn commit(&self, grant: Grant) -> Result<Reservation, LedgerError> {
            let (id, resource, amount, lease, key) = grant.into_parts();
            let mut outstanding = self.outstanding.lock().map_err(|_| poisoned())?;
            let mut committed = self.committed.lock().map_err(|_| poisoned())?;
            *outstanding = outstanding.saturating_sub(amount);
            *committed += amount;
            Ok(Reservation {
                id,
                resource,
                amount,
                state: ReservationState::Committed,
                lease,
                idempotency_key: key,
            })
        }

        async fn release(&self, grant: Grant) -> Result<Reservation, LedgerError> {
            let (id, resource, amount, lease, key) = grant.into_parts();
            let mut outstanding = self.outstanding.lock().map_err(|_| poisoned())?;
            *outstanding = outstanding.saturating_sub(amount);
            Ok(Reservation {
                id,
                resource,
                amount,
                state: ReservationState::Released,
                lease,
                idempotency_key: key,
            })
        }

        async fn exposure(&self, resource: &ResourceId) -> Result<Exposure, LedgerError> {
            let committed = *self.committed.lock().map_err(|_| poisoned())?;
            let outstanding = *self.outstanding.lock().map_err(|_| poisoned())?;
            Ok(Exposure::new(
                resource.clone(),
                self.hard_limit,
                committed,
                outstanding,
                Window::UNBOUNDED,
            ))
        }
    }

    fn service(limit: u64) -> CapacityLedgerServiceImpl {
        CapacityLedgerServiceImpl::new(Arc::new(FakeLedger::new(limit)))
    }

    fn reserve_input(amount: u64, key: &str) -> ReserveCapacityInput {
        ReserveCapacityInput {
            resource: ResourceId::new("gpu"),
            amount,
            idempotency_key: IdempotencyKey::new(key),
            requested_by: None,
        }
    }

    #[tokio::test]
    async fn reserve_then_commit_moves_outstanding_to_committed() {
        let svc = service(10);
        let reserved = svc.reserve(reserve_input(3, "k1")).await.unwrap();
        assert_eq!(reserved.reservation.state, ReservationState::Outstanding);
        assert_eq!(reserved.exposure.outstanding, 3);

        let committed = svc
            .commit(CommitCapacityInput {
                reservation_id: reserved.reservation.id,
            })
            .await
            .unwrap();
        assert_eq!(committed.reservation.state, ReservationState::Committed);
        assert_eq!(committed.exposure.committed, 3);
        assert_eq!(committed.exposure.outstanding, 0);
    }

    #[tokio::test]
    async fn release_returns_capacity_to_the_pool() {
        let svc = service(10);
        let reserved = svc.reserve(reserve_input(4, "k1")).await.unwrap();
        let released = svc
            .release(ReleaseCapacityInput {
                reservation_id: reserved.reservation.id,
                reason: Some("step failed".to_string()),
            })
            .await
            .unwrap();
        assert_eq!(released.reservation.state, ReservationState::Released);
        assert_eq!(released.exposure.outstanding, 0);
        assert_eq!(released.exposure.available(), 10);
    }

    #[tokio::test]
    async fn reserve_beyond_the_cap_fails_closed() {
        let svc = service(2);
        let err = svc.reserve(reserve_input(3, "k1")).await.unwrap_err();
        assert!(matches!(err, LedgerError::Insufficient { .. }));
    }

    #[tokio::test]
    async fn commit_of_an_unknown_reservation_is_unavailable() {
        let svc = service(10);
        let err = svc
            .commit(CommitCapacityInput {
                reservation_id: ReservationId::new(),
            })
            .await
            .unwrap_err();
        assert!(err.is_unavailable());
    }
}
