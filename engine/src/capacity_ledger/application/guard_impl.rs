//! Concrete ReservationGuard (RAII, ADR-008).
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#reservationguard
//! Implements: ISSUE-CAPACITY-LEDGER-8 — release-on-drop reservation guard
//! Issue: #1029
//! ADR-008: RAII reservation pattern
//!
//! [`ReservationGuardImpl`] owns a [`Grant`] and releases it when dropped
//! without a successful commit. Because `Drop` cannot be async, a release is
//! dispatched onto the Tokio runtime the guard was created in; if there is no
//! runtime, the hold is left to expire with its lease (and a warning is logged).

use std::fmt;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;

use crate::capacity_ledger::domain::{Grant, LedgerError, Reservation, ReservationId};
use crate::capacity_ledger::interfaces::CapacityLedger;

use super::guard::ReservationGuard;

/// Mutable guard state, kept behind one mutex so commit/drop cannot race.
struct GuardState {
    grant: Option<Grant>,
    committed: bool,
    released: bool,
}

/// RAII guard that commits once and releases on drop.
pub struct ReservationGuardImpl {
    ledger: Arc<dyn CapacityLedger>,
    reservation_id: ReservationId,
    /// Handle captured at construction; `None` when built outside a runtime.
    runtime: Option<tokio::runtime::Handle>,
    state: Mutex<GuardState>,
}

impl ReservationGuardImpl {
    /// Bind `grant` to a ledger, capturing the current runtime for drop-release.
    pub fn new(ledger: Arc<dyn CapacityLedger>, grant: Grant) -> Self {
        let reservation_id = grant.reservation_id();
        Self {
            ledger,
            reservation_id,
            runtime: tokio::runtime::Handle::try_current().ok(),
            state: Mutex::new(GuardState {
                grant: Some(grant),
                committed: false,
                released: false,
            }),
        }
    }

    /// Release the underlying reservation, awaiting the ledger call.
    ///
    /// The service uses this for by-id release so the terminal snapshot and the
    /// post-release exposure are deterministic; a `Drop`-only release is
    /// scheduled asynchronously and cannot be awaited. The guard is marked
    /// released before the await, so its `Drop` does not release twice.
    pub(crate) async fn release(self: Box<Self>) -> Result<Reservation, LedgerError> {
        let grant = {
            let mut state = self.state.lock().map_err(|_| LedgerError::Unavailable {
                detail: "reservation guard poisoned".to_string(),
            })?;
            if state.released {
                return Err(LedgerError::Unavailable {
                    detail: format!("reservation {} is already released", self.reservation_id),
                });
            }
            state.released = true;
            state.grant.take().ok_or_else(|| LedgerError::Unavailable {
                detail: format!("reservation {} grant already consumed", self.reservation_id),
            })?
        };
        self.ledger.release(grant).await
    }
}

impl fmt::Debug for ReservationGuardImpl {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ReservationGuardImpl")
            .field("reservation_id", &self.reservation_id)
            .finish_non_exhaustive()
    }
}

#[async_trait]
impl ReservationGuard for ReservationGuardImpl {
    fn reservation_id(&self) -> ReservationId {
        self.reservation_id
    }

    fn is_committed(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.committed)
            .unwrap_or(false)
    }

    fn is_released(&self) -> bool {
        self.state
            .lock()
            .map(|state| state.released)
            .unwrap_or(false)
    }

    async fn commit(self: Box<Self>) -> Result<Reservation, LedgerError> {
        let grant = {
            let mut state = self.state.lock().map_err(|_| LedgerError::Unavailable {
                detail: "reservation guard poisoned".to_string(),
            })?;
            if state.committed {
                return Err(LedgerError::Unavailable {
                    detail: format!("reservation {} is already committed", self.reservation_id),
                });
            }
            state.committed = true;
            state.grant.take().ok_or_else(|| LedgerError::Unavailable {
                detail: format!("reservation {} grant already consumed", self.reservation_id),
            })?
        };
        self.ledger.commit(grant).await
    }
}

impl Drop for ReservationGuardImpl {
    fn drop(&mut self) {
        // Mark released and take the grant; a committed guard has neither.
        let grant = match self.state.lock() {
            Ok(mut state) => {
                if state.committed || state.released {
                    None
                } else {
                    state.released = true;
                    state.grant.take()
                }
            }
            Err(_) => None,
        };

        if let Some(grant) = grant {
            match &self.runtime {
                Some(handle) => {
                    let ledger = self.ledger.clone();
                    handle.spawn(async move {
                        // Best-effort: an expired/committed reservation is fine.
                        let _ = ledger.release(grant).await;
                    });
                }
                None => {
                    tracing::warn!(
                        reservation = %self.reservation_id,
                        "capacity reservation dropped outside a Tokio runtime; relying on lease expiry to reclaim it"
                    );
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::time::Duration;

    use chrono::Utc;

    use super::*;
    use crate::capacity_ledger::domain::{
        Exposure, IdempotencyKey, Lease, ReservationState, ResourceId, ResourceSpec,
    };

    struct CountingLedger {
        commits: AtomicUsize,
        releases: AtomicUsize,
        released: tokio::sync::Notify,
    }

    impl CountingLedger {
        fn new() -> Self {
            Self {
                commits: AtomicUsize::new(0),
                releases: AtomicUsize::new(0),
                released: tokio::sync::Notify::new(),
            }
        }
    }

    fn fake_reservation(grant: Grant, state: ReservationState) -> Reservation {
        let (id, resource, amount, lease, key) = grant.into_parts();
        Reservation {
            id,
            resource,
            amount,
            state,
            lease,
            idempotency_key: key,
        }
    }

    #[async_trait]
    impl CapacityLedger for CountingLedger {
        async fn declare_resource(&self, _spec: ResourceSpec) -> Result<(), LedgerError> {
            Ok(())
        }

        async fn reserve(
            &self,
            _resource: &ResourceId,
            _amount: u64,
            _key: IdempotencyKey,
        ) -> Result<Grant, LedgerError> {
            Err(LedgerError::Unavailable {
                detail: "reserve not used in guard tests".to_string(),
            })
        }

        async fn commit(&self, grant: Grant) -> Result<Reservation, LedgerError> {
            self.commits.fetch_add(1, Ordering::SeqCst);
            Ok(fake_reservation(grant, ReservationState::Committed))
        }

        async fn release(&self, grant: Grant) -> Result<Reservation, LedgerError> {
            self.releases.fetch_add(1, Ordering::SeqCst);
            self.released.notify_one();
            Ok(fake_reservation(grant, ReservationState::Released))
        }

        async fn exposure(&self, _resource: &ResourceId) -> Result<Exposure, LedgerError> {
            Err(LedgerError::Unavailable {
                detail: "exposure not used in guard tests".to_string(),
            })
        }
    }

    fn grant() -> Grant {
        Grant::issue(
            ReservationId::new(),
            ResourceId::new("gpu"),
            2,
            Lease::expiring_in(Utc::now(), Duration::from_secs(60)),
            IdempotencyKey::new("k1"),
        )
    }

    #[tokio::test]
    async fn commit_is_terminal_and_does_not_release_on_drop() {
        let ledger = Arc::new(CountingLedger::new());
        let guard = ReservationGuardImpl::new(ledger.clone(), grant());
        assert!(!guard.is_committed());

        let reservation = Box::new(guard).commit().await.expect("commit");
        assert_eq!(reservation.state, ReservationState::Committed);

        tokio::time::sleep(Duration::from_millis(20)).await;
        assert_eq!(ledger.commits.load(Ordering::SeqCst), 1);
        assert_eq!(ledger.releases.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn drop_without_commit_releases_the_grant() {
        let ledger = Arc::new(CountingLedger::new());
        let guard = ReservationGuardImpl::new(ledger.clone(), grant());
        let reservation_id = guard.reservation_id();
        assert!(!guard.is_released());

        drop(guard);

        tokio::time::timeout(Duration::from_secs(1), ledger.released.notified())
            .await
            .expect("drop-release signal");
        assert_eq!(ledger.releases.load(Ordering::SeqCst), 1);
        assert_eq!(ledger.commits.load(Ordering::SeqCst), 0);
        // The guard is gone, but its identity was stable.
        assert_ne!(reservation_id.as_uuid(), uuid::Uuid::nil());
    }

    #[test]
    fn drop_outside_a_runtime_falls_back_to_lease_expiry() {
        let ledger = Arc::new(CountingLedger::new());
        let guard = ReservationGuardImpl::new(ledger.clone(), grant());
        // No Tokio runtime here: the guard must not panic, and must not leak a
        // half-released state.
        drop(guard);
        assert_eq!(ledger.releases.load(Ordering::SeqCst), 0);
    }
}
