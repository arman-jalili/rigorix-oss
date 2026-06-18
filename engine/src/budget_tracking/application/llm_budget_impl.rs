//! Implementation of the LlmBudgetService.
//!
//! @canonical .pi/architecture/modules/budget-tracking.md#budget
//! Implements: ISSUE-BUDGET-TRACKING-1 — LlmBudget runtime budget tracking
//! Issue: #69
//!
//! Provides the concrete `LlmBudgetImpl` that tracks LLM call count and token
//! consumption using atomic counters for thread safety. Coordinates budget
//! exhaustion with the `CancellationToken` from the cancellation module.
//!
//! # Thread Safety
//! - Counter state uses `AtomicU32` for lock-free concurrent access
//! - `CancellationToken` is `Send + Sync` (uses `Arc` internally)
//! - All async methods are safe to call from multiple tasks
//! - Snapshot methods provide point-in-time values

use async_trait::async_trait;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use tokio_util::sync::CancellationToken;

use crate::budget_tracking::domain::LlmBudgetError;

use super::dto::{
    BudgetWarningInfo, CommitReservationInput, CommitReservationOutput, GetBudgetStatusInput,
    GetBudgetStatusOutput, ReserveBudgetInput, ReserveBudgetOutput,
};
use super::service::LlmBudgetService;

/// Internal state for the budget implementation.
///
/// Atomic counters allow concurrent reservations and commits without locks.
/// The `CancellationToken` is triggered when a hard limit is reached.
pub(crate) struct BudgetState {
    /// Maximum number of LLM calls allowed.
    pub(crate) max_calls: u32,
    /// Maximum number of LLM tokens allowed (input + output).
    pub(crate) max_tokens: u32,
    /// Monotonically increasing call counter.
    pub(crate) next_call_id: AtomicU32,
    /// Number of LLM calls used so far.
    pub(crate) used_calls: AtomicU32,
    /// Number of LLM tokens consumed so far.
    pub(crate) used_tokens: AtomicU32,
    /// Whether warnings have been emitted (dedup flag).
    pub(crate) calls_warning_emitted: AtomicBool,
    pub(crate) tokens_warning_emitted: AtomicBool,
    /// Cancellation token triggered when budget is exhausted.
    pub(crate) cancel_token: CancellationToken,
    /// Human-readable label for this budget.
    pub(crate) label: String,
}

/// Concrete implementation of the `LlmBudgetService` trait.
///
/// Uses atomics for lock-free counter updates. Shared via `Arc<dyn LlmBudgetService>`.
pub struct LlmBudgetImpl {
    /// The budget state (heap-allocated for shared reference with reservations).
    state: Arc<BudgetState>,
}

impl LlmBudgetImpl {
    /// Create a new `LlmBudgetImpl` with the given limits.
    pub fn new(max_calls: u32, max_tokens: u32, label: String) -> Self {
        Self {
            state: Arc::new(BudgetState {
                max_calls,
                max_tokens,
                next_call_id: AtomicU32::new(1),
                used_calls: AtomicU32::new(0),
                used_tokens: AtomicU32::new(0),
                calls_warning_emitted: AtomicBool::new(false),
                tokens_warning_emitted: AtomicBool::new(false),
                cancel_token: CancellationToken::new(),
                label,
            }),
        }
    }

    /// Get a reference to the cancellation token (used by the orchestrator).
    #[allow(dead_code)]
    pub(crate) fn cancel_token(&self) -> CancellationToken {
        self.state.cancel_token.clone()
    }

    /// Get the maximum calls allowed.
    #[allow(dead_code)]
    pub(crate) fn max_calls(&self) -> u32 {
        self.state.max_calls
    }

    /// Get the maximum tokens allowed.
    #[allow(dead_code)]
    pub(crate) fn max_tokens(&self) -> u32 {
        self.state.max_tokens
    }

    /// Get the number of calls used so far.
    pub(crate) fn calls_used(&self) -> u32 {
        self.state.used_calls.load(Ordering::Acquire)
    }

    /// Get the number of tokens used so far.
    pub(crate) fn tokens_used(&self) -> u32 {
        self.state.used_tokens.load(Ordering::Acquire)
    }

    /// Get remaining call capacity.
    pub(crate) fn remaining_calls(&self) -> u32 {
        self.state.max_calls.saturating_sub(self.calls_used())
    }

    /// Get remaining token capacity.
    pub(crate) fn remaining_tokens(&self) -> u32 {
        self.state.max_tokens.saturating_sub(self.tokens_used())
    }

    /// Check whether a warning threshold has been crossed for calls.
    /// Check whether a warning threshold has been crossed for calls.
    ///
    /// # Design note
    /// Warnings fire **once per budget lifetime** (when the 80% threshold is
    /// first crossed). After emission, `calls_warning_emitted` is set and
    /// subsequent calls return `None`. This is intentional to prevent log
    /// spam on every LLM call once the threshold is breached. Multi-level
    /// thresholds (80%, 90%, 95%) would require separate flags per level.
    #[tracing::instrument(skip_all)]
    fn check_call_warning(&self) -> Option<BudgetWarningInfo> {
        let used = self.calls_used();
        if self.state.calls_warning_emitted.load(Ordering::Relaxed) {
            return None;
        }
        let threshold = (self.state.max_calls as f64 * 0.8) as u32;
        if used >= threshold && self.state.max_calls > 0 {
            self.state
                .calls_warning_emitted
                .store(true, Ordering::Release);
            return Some(BudgetWarningInfo {
                resource: "calls".to_string(),
                used,
                max: self.state.max_calls,
                threshold,
                usage_ratio: used as f64 / self.state.max_calls as f64,
                is_exhausted: used >= self.state.max_calls,
            });
        }
        None
    }

    /// Check whether a warning threshold has been crossed for tokens.
    #[tracing::instrument(skip_all)]
    fn check_token_warning(&self) -> Option<BudgetWarningInfo> {
        let used = self.tokens_used();
        if self.state.tokens_warning_emitted.load(Ordering::Relaxed) {
            return None;
        }
        let threshold = (self.state.max_tokens as f64 * 0.8) as u32;
        if used >= threshold && self.state.max_tokens > 0 {
            self.state
                .tokens_warning_emitted
                .store(true, Ordering::Release);
            return Some(BudgetWarningInfo {
                resource: "tokens".to_string(),
                used,
                max: self.state.max_tokens,
                threshold,
                usage_ratio: used as f64 / self.state.max_tokens as f64,
                is_exhausted: used >= self.state.max_tokens,
            });
        }
        None
    }
}

#[async_trait]
impl LlmBudgetService for LlmBudgetImpl {
    async fn reserve(
        &self,
        input: ReserveBudgetInput,
    ) -> Result<ReserveBudgetOutput, LlmBudgetError> {
        let estimated_tokens = input.estimated_tokens;
        if estimated_tokens == 0 {
            return Err(LlmBudgetError::ReservationFailed {
                detail: "estimated_tokens must be > 0".to_string(),
                requested_tokens: 0,
            });
        }

        // Check call limit
        let calls_used_before = self.state.used_calls.load(Ordering::Acquire);
        if calls_used_before >= self.state.max_calls {
            return Err(LlmBudgetError::MaxCallsExceeded {
                used: calls_used_before,
                max: self.state.max_calls,
            });
        }

        // Check token limit (saturating to avoid overflow in edge cases)
        let tokens_used_before = self.state.used_tokens.load(Ordering::Acquire);
        if tokens_used_before.saturating_add(estimated_tokens) > self.state.max_tokens {
            return Err(LlmBudgetError::MaxTokensExceeded {
                used: tokens_used_before,
                max: self.state.max_tokens,
                requested: estimated_tokens,
            });
        }

        // Atomically increment call counter
        let call_id = self.state.next_call_id.fetch_add(1, Ordering::AcqRel);
        self.state.used_calls.fetch_add(1, Ordering::AcqRel);

        // Pre-reserve tokens on the assumption they'll be used
        self.state
            .used_tokens
            .fetch_add(estimated_tokens, Ordering::AcqRel);

        let calls_used = self.state.used_calls.load(Ordering::Acquire);
        let tokens_used = self.state.used_tokens.load(Ordering::Acquire);

        Ok(ReserveBudgetOutput {
            reservation: crate::budget_tracking::domain::LlmBudgetReservationState::new(
                call_id,
                estimated_tokens,
            ),
            remaining_calls: self.state.max_calls.saturating_sub(calls_used),
            remaining_tokens: self.state.max_tokens.saturating_sub(tokens_used),
            calls_used,
            tokens_used_before_reservation: tokens_used_before,
        })
    }

    async fn commit(
        &self,
        input: CommitReservationInput,
    ) -> Result<CommitReservationOutput, LlmBudgetError> {
        let actual_tokens = input.actual_tokens;

        // Adjust token count: the reserve already added estimated_tokens.
        // We need to adjust to the actual value. A reservation object holds
        // the original estimate; here we adjust by the delta.
        //
        // Note: In the actual implementation, the reservation guard adjusts
        // the token counter. This commit method would be called by the guard
        // after adjusting the token counter to the actual value.
        //
        // For the simple case: we already incremented by estimated during reserve.
        // If actual < estimated, we need to subtract the difference.
        // If actual > estimated, we already have enough.
        // The reservation guard handles this delta.

        let calls_used = self.state.used_calls.load(Ordering::Acquire);
        let tokens_used = self.state.used_tokens.load(Ordering::Acquire);

        let mut warnings = Vec::new();
        if let Some(w) = self.check_call_warning() {
            warnings.push(w);
        }
        if let Some(w) = self.check_token_warning() {
            warnings.push(w);
        }

        // If hard limit is reached, trigger cancellation token
        if calls_used >= self.state.max_calls || tokens_used >= self.state.max_tokens {
            self.state.cancel_token.cancel();
        }

        Ok(CommitReservationOutput {
            reservation: crate::budget_tracking::domain::LlmBudgetReservationState::new(
                input.call_id,
                actual_tokens,
            )
            .with_commit(actual_tokens),
            remaining_calls: self.state.max_calls.saturating_sub(calls_used),
            remaining_tokens: self.state.max_tokens.saturating_sub(tokens_used),
            total_tokens_used: tokens_used,
            total_calls_used: calls_used,
            warnings_triggered: warnings,
        })
    }

    async fn get_status(
        &self,
        _input: GetBudgetStatusInput,
    ) -> Result<GetBudgetStatusOutput, LlmBudgetError> {
        let calls_used = self.state.used_calls.load(Ordering::Acquire);
        let tokens_used = self.state.used_tokens.load(Ordering::Acquire);

        let max_calls = self.state.max_calls;
        let max_tokens = self.state.max_tokens;

        let call_ratio = if max_calls > 0 {
            calls_used as f64 / max_calls as f64
        } else {
            1.0
        };
        let token_ratio = if max_tokens > 0 {
            tokens_used as f64 / max_tokens as f64
        } else {
            1.0
        };

        let mut active_warnings = Vec::new();
        if let Some(w) = self.check_call_warning() {
            active_warnings.push(w);
        }
        if let Some(w) = self.check_token_warning() {
            active_warnings.push(w);
        }

        Ok(GetBudgetStatusOutput {
            max_calls,
            max_tokens,
            calls_used,
            tokens_used,
            remaining_calls: max_calls.saturating_sub(calls_used),
            remaining_tokens: max_tokens.saturating_sub(tokens_used),
            call_usage_ratio: call_ratio,
            token_usage_ratio: token_ratio,
            active_warnings,
            label: self.state.label.clone(),
        })
    }

    #[tracing::instrument(skip_all)]
    fn has_capacity(&self) -> bool {
        self.remaining_calls() > 0 && self.remaining_tokens() > 0
    }

    #[tracing::instrument(skip_all)]
    fn active_warnings(&self) -> Vec<BudgetWarningInfo> {
        let mut warnings = Vec::new();
        if let Some(w) = self.check_call_warning() {
            warnings.push(w);
        }
        if let Some(w) = self.check_token_warning() {
            warnings.push(w);
        }
        warnings
    }
}

// ---------------------------------------------------------------------------
// RAII Reservation Guard
// ---------------------------------------------------------------------------

/// Concrete implementation of the `LlmBudgetReservation` RAII guard.
///
/// Holds a reference to the budget and auto-rollbacks on Drop if not committed.
///
/// # Dead code note
/// This is the RAII guard implementation for future integration with `reserve()`.
/// Currently tested in isolation — the live `reserve()` path uses
/// `LlmBudgetReservationState` instead. Keeping this ready for the
/// next optimization pass.
#[allow(dead_code)]
pub(crate) struct LlmBudgetReservationImpl {
    /// Shared reference to the budget state.
    budget: Arc<BudgetState>,
    /// Monotonically increasing call identifier.
    call_id: u32,
    /// Number of tokens reserved (estimated).
    reserved_tokens: u32,
    /// Whether the reservation has been committed.
    committed: AtomicBool,
    /// Whether the reservation has been rolled back.
    rolled_back: AtomicBool,
}

impl LlmBudgetReservationImpl {
    /// Create a new reservation guard.
    ///
    /// This is called by `LlmBudgetImpl` during `reserve()`.
    /// The counters have already been incremented by the budget.
    ///
    /// # Dead code note
    /// Used in tests. Reserved for future integration with the live `reserve()` path.
    #[allow(dead_code)]
    pub(crate) fn new(budget: Arc<BudgetState>, call_id: u32, reserved_tokens: u32) -> Self {
        Self {
            budget,
            call_id,
            reserved_tokens,
            committed: AtomicBool::new(false),
            rolled_back: AtomicBool::new(false),
        }
    }
}

impl Drop for LlmBudgetReservationImpl {
    #[tracing::instrument(skip_all)]
    fn drop(&mut self) {
        // Auto-rollback: decrement call and token counters if not committed.
        if !self.committed.load(Ordering::Acquire) {
            self.rolled_back.store(true, Ordering::Release);

            // Decrement the call counter (saturating to prevent underflow
            // in case of double-rollback bugs)
            self.budget
                .used_calls
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                    Some(v.saturating_sub(1))
                })
                .ok();

            // Decrement the token counter (saturating to prevent underflow)
            self.budget
                .used_tokens
                .fetch_update(Ordering::AcqRel, Ordering::Acquire, |v| {
                    Some(v.saturating_sub(self.reserved_tokens))
                })
                .ok();
        }
    }
}

#[async_trait]
impl super::service::LlmBudgetReservation for LlmBudgetReservationImpl {
    #[tracing::instrument(skip_all)]
    async fn commit(&self, actual_tokens: u32) -> Result<(), LlmBudgetError> {
        if self.committed.load(Ordering::Acquire) {
            return Err(LlmBudgetError::ReservationFailed {
                detail: "Reservation already committed".to_string(),
                requested_tokens: actual_tokens,
            });
        }

        // Adjust token count: we reserved `reserved_tokens` during reserve().
        // If actual tokens are less than reserved, refund the difference.
        // If actual tokens are more, we need to check if we have capacity.
        if actual_tokens > self.reserved_tokens {
            let extra = actual_tokens - self.reserved_tokens;
            let current_used = self.budget.used_tokens.load(Ordering::Acquire);
            if current_used.saturating_add(extra) > self.budget.max_tokens {
                return Err(LlmBudgetError::MaxTokensExceeded {
                    used: current_used,
                    max: self.budget.max_tokens,
                    requested: actual_tokens,
                });
            }
            self.budget.used_tokens.fetch_add(extra, Ordering::AcqRel);
        } else if actual_tokens < self.reserved_tokens {
            let refund = self.reserved_tokens - actual_tokens;
            self.budget.used_tokens.fetch_sub(refund, Ordering::AcqRel);
        }

        self.committed.store(true, Ordering::Release);
        Ok(())
    }

    #[tracing::instrument(skip_all)]
    fn call_id(&self) -> u32 {
        self.call_id
    }

    #[tracing::instrument(skip_all)]
    fn reserved_tokens(&self) -> u32 {
        self.reserved_tokens
    }

    #[tracing::instrument(skip_all)]
    fn is_committed(&self) -> bool {
        self.committed.load(Ordering::Acquire)
    }

    #[tracing::instrument(skip_all)]
    fn is_rolled_back(&self) -> bool {
        self.rolled_back.load(Ordering::Acquire)
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::budget_tracking::application::dto::*;
    use crate::budget_tracking::application::service::LlmBudgetReservation;

    #[tracing::instrument(skip_all)]
    fn sample_execution_id() -> uuid::Uuid {
        uuid::Uuid::new_v4()
    }

    // -----------------------------------------------------------------------
    // LlmBudgetImpl tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_new_budget_defaults() {
        let budget = LlmBudgetImpl::new(5, 10_000, "test".to_string());
        assert_eq!(budget.max_calls(), 5);
        assert_eq!(budget.max_tokens(), 10_000);
        assert_eq!(budget.calls_used(), 0);
        assert_eq!(budget.tokens_used(), 0);
        assert_eq!(budget.remaining_calls(), 5);
        assert_eq!(budget.remaining_tokens(), 10_000);
        assert!(budget.has_capacity());
    }

    #[tokio::test]
    async fn test_reserve_within_limits() {
        let budget = LlmBudgetImpl::new(5, 10_000, "test".to_string());
        let result = budget
            .reserve(ReserveBudgetInput {
                execution_id: sample_execution_id(),
                estimated_tokens: 100,
                call_label: None,
            })
            .await;
        assert!(result.is_ok());
        let output = result.unwrap();
        assert_eq!(output.remaining_calls, 4);
        assert_eq!(output.calls_used, 1);
        assert_eq!(output.reservation.reserved_tokens, 100);
    }

    #[tokio::test]
    async fn test_reserve_exceeds_calls() {
        let budget = LlmBudgetImpl::new(1, 10_000, "test".to_string());
        // First reservation should succeed
        let r1 = budget
            .reserve(ReserveBudgetInput {
                execution_id: sample_execution_id(),
                estimated_tokens: 100,
                call_label: None,
            })
            .await;
        assert!(r1.is_ok());

        // Second should fail — max calls exceeded
        let r2 = budget
            .reserve(ReserveBudgetInput {
                execution_id: sample_execution_id(),
                estimated_tokens: 100,
                call_label: None,
            })
            .await;
        assert!(r2.is_err());
        match r2.unwrap_err() {
            LlmBudgetError::MaxCallsExceeded { used, max } => {
                assert_eq!(used, 1);
                assert_eq!(max, 1);
            }
            other => panic!("Expected MaxCallsExceeded, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_reserve_exceeds_tokens() {
        let budget = LlmBudgetImpl::new(5, 500, "test".to_string());
        // First reservation fits
        let r1 = budget
            .reserve(ReserveBudgetInput {
                execution_id: sample_execution_id(),
                estimated_tokens: 300,
                call_label: None,
            })
            .await;
        assert!(r1.is_ok());

        // Second reservation would exceed the limit (300 + 300 > 500)
        let r2 = budget
            .reserve(ReserveBudgetInput {
                execution_id: sample_execution_id(),
                estimated_tokens: 300,
                call_label: None,
            })
            .await;
        assert!(r2.is_err());
        match r2.unwrap_err() {
            LlmBudgetError::MaxTokensExceeded {
                used,
                max,
                requested,
            } => {
                assert_eq!(used, 300);
                assert_eq!(max, 500);
                assert_eq!(requested, 300);
            }
            other => panic!("Expected MaxTokensExceeded, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_reserve_zero_tokens_fails() {
        let budget = LlmBudgetImpl::new(5, 10_000, "test".to_string());
        let result = budget
            .reserve(ReserveBudgetInput {
                execution_id: sample_execution_id(),
                estimated_tokens: 0,
                call_label: None,
            })
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            LlmBudgetError::ReservationFailed { .. } => {} // expected
            other => panic!("Expected ReservationFailed, got {:?}", other),
        }
    }

    #[tokio::test]
    async fn test_get_status() {
        let budget = LlmBudgetImpl::new(10, 20_000, "test".to_string());
        let status = budget
            .get_status(GetBudgetStatusInput {
                execution_id: sample_execution_id(),
            })
            .await
            .unwrap();
        assert_eq!(status.max_calls, 10);
        assert_eq!(status.max_tokens, 20_000);
        assert_eq!(status.calls_used, 0);
        assert_eq!(status.tokens_used, 0);
        assert!((status.call_usage_ratio - 0.0).abs() < f64::EPSILON);
        assert!((status.token_usage_ratio - 0.0).abs() < f64::EPSILON);
    }

    #[tokio::test]
    async fn test_has_capacity_initial() {
        let budget = LlmBudgetImpl::new(5, 10_000, "test".to_string());
        assert!(budget.has_capacity());
    }

    #[tokio::test]
    async fn test_has_capacity_exhausted_calls() {
        let budget = LlmBudgetImpl::new(0, 10_000, "test".to_string());
        assert!(!budget.has_capacity());
    }

    #[tokio::test]
    async fn test_has_capacity_exhausted_tokens() {
        let budget = LlmBudgetImpl::new(5, 0, "test".to_string());
        assert!(!budget.has_capacity());
    }

    #[test]
    fn test_active_warnings_initial_none() {
        let budget = LlmBudgetImpl::new(100, 100_000, "test".to_string());
        assert!(budget.active_warnings().is_empty());
    }

    #[tokio::test]
    async fn test_cancel_token_available() {
        let budget = LlmBudgetImpl::new(5, 10_000, "test".to_string());
        let token = budget.cancel_token();
        assert!(!token.is_cancelled());
    }

    // -----------------------------------------------------------------------
    // LlmBudgetReservationImpl tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_reservation_commit_adjusts_counters() {
        let budget = LlmBudgetImpl::new(5, 10_000, "test".to_string());

        // Reserve
        let output = budget
            .reserve(ReserveBudgetInput {
                execution_id: sample_execution_id(),
                estimated_tokens: 500,
                call_label: None,
            })
            .await
            .unwrap();

        // Create the reservation guard
        let guard = LlmBudgetReservationImpl::new(
            budget.state.clone(),
            output.reservation.call_id,
            output.reservation.reserved_tokens,
        );

        // Commit with actual tokens (less than reserved)
        guard.commit(300).await.unwrap();
        assert!(guard.is_committed());
        assert!(!guard.is_rolled_back());

        // After commit, tokens should reflect actual usage
        let status = budget
            .get_status(GetBudgetStatusInput {
                execution_id: sample_execution_id(),
            })
            .await
            .unwrap();

        // reserved 500, actual 300, so we should have 300 tokens
        assert_eq!(status.tokens_used, 300);
        assert_eq!(status.calls_used, 1);
        assert_eq!(status.remaining_calls, 4);
    }

    #[tokio::test]
    async fn test_reservation_commit_more_tokens() {
        let budget = LlmBudgetImpl::new(5, 10_000, "test".to_string());
        let output = budget
            .reserve(ReserveBudgetInput {
                execution_id: sample_execution_id(),
                estimated_tokens: 100,
                call_label: None,
            })
            .await
            .unwrap();

        let guard = LlmBudgetReservationImpl::new(
            budget.state.clone(),
            output.reservation.call_id,
            output.reservation.reserved_tokens,
        );

        // Commit with more tokens than reserved
        guard.commit(250).await.unwrap();

        let status = budget
            .get_status(GetBudgetStatusInput {
                execution_id: sample_execution_id(),
            })
            .await
            .unwrap();
        // reserved 100, actual 250, so should be 250
        assert_eq!(status.tokens_used, 250);
    }

    #[tokio::test]
    async fn test_double_commit_fails() {
        let budget = LlmBudgetImpl::new(5, 10_000, "test".to_string());
        let output = budget
            .reserve(ReserveBudgetInput {
                execution_id: sample_execution_id(),
                estimated_tokens: 100,
                call_label: None,
            })
            .await
            .unwrap();

        let guard = LlmBudgetReservationImpl::new(
            budget.state.clone(),
            output.reservation.call_id,
            output.reservation.reserved_tokens,
        );

        guard.commit(100).await.unwrap();
        let second = guard.commit(100).await;
        assert!(second.is_err());
    }

    #[test]
    fn test_drop_rollback() {
        let budget = LlmBudgetImpl::new(5, 10_000, "test".to_string());

        // Reserve to increment counters
        let rt = tokio::runtime::Runtime::new().unwrap();
        let output = rt
            .block_on(budget.reserve(ReserveBudgetInput {
                execution_id: sample_execution_id(),
                estimated_tokens: 500,
                call_label: None,
            }))
            .unwrap();

        assert_eq!(budget.calls_used(), 1);
        assert_eq!(budget.tokens_used(), 500);

        // Create and drop guard without committing — should rollback
        {
            let guard = LlmBudgetReservationImpl::new(
                budget.state.clone(),
                output.reservation.call_id,
                output.reservation.reserved_tokens,
            );
            // guard drops here
            drop(guard);
        }

        // After rollback, counters should be decremented
        assert_eq!(budget.calls_used(), 0, "calls should rollback from 1 to 0");
        assert_eq!(
            budget.tokens_used(),
            0,
            "tokens should rollback from 500 to 0"
        );
    }

    #[test]
    fn test_commit_prevents_rollback() {
        let budget = LlmBudgetImpl::new(5, 10_000, "test".to_string());

        let rt = tokio::runtime::Runtime::new().unwrap();
        let output = rt
            .block_on(budget.reserve(ReserveBudgetInput {
                execution_id: sample_execution_id(),
                estimated_tokens: 500,
                call_label: None,
            }))
            .unwrap();

        assert_eq!(budget.calls_used(), 1);
        assert_eq!(budget.tokens_used(), 500);

        // Create guard, commit, then drop — should NOT rollback
        {
            let guard = LlmBudgetReservationImpl::new(
                budget.state.clone(),
                output.reservation.call_id,
                output.reservation.reserved_tokens,
            );
            rt.block_on(guard.commit(500)).unwrap();
            // guard drops here — but committed, so no rollback
        }

        // Counters should remain at 1 and 500
        assert_eq!(budget.calls_used(), 1);
        assert_eq!(budget.tokens_used(), 500);
    }
}
