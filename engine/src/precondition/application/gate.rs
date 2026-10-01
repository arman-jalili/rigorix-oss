//! DispatchGate — the single dispatch choke point contract (ADR-017 §R1).
//!
//! @canonical .pi/architecture/modules/consequence-gating.md#dispatch-integration
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — DispatchGate trait + fail-closed arming
//! Issue: #938 (consequence-gating epic — contract freeze); behavior closed in
//!   ISSUE-CONSEQUENCE-GATING-5 (DispatchGate)
//!
//! `run_dispatch_loop`, for the node about to dispatch, evaluates in order:
//!
//! ```text
//! 1. ADR-011 approval verification   (verify_before_dispatch)
//! 2. DispatchGate::assess            (NEW — precondition service + arming)
//! 3. ADR-013 R3 sequence-policy prefix
//! 4. spawn_concurrent_node
//! ```
//!
//! A refusal records a deterministic node failure and **never** calls the tool.
//!
//! # Contract (Frozen)
//! - `assess` is the single decision point; a `Deny` verdict means the tool is
//!   never called and the node is marked failed
//! - **Fail-closed arming**: when preconditions are configured but cannot arm,
//!   a matching step is refused with `PreconditionError::NotArmed` (recorded as
//!   an `unarmed` finding) — never a silent downgrade
//! - The gate is a no-op when no preconditions are configured (fail-open-absent,
//!   status quo)
//! - The gate is async and trait-object safe (`Send + Sync`, `async-trait`)

use async_trait::async_trait;

use crate::precondition::domain::{PreconditionError, PreconditionVerdict};

use super::dto::DispatchStep;

/// The dispatch choke-point gate.
#[async_trait]
pub trait DispatchGate: Send + Sync {
    /// Assess the about-to-dispatch step.
    ///
    /// # Returns
    /// - `Ok(PreconditionVerdict::Dispatch)` — proceed to the next gate
    /// - `Ok(PreconditionVerdict::Deny { .. })` — refuse; never call the tool
    ///
    /// # Errors
    /// - `PreconditionError::ConfigInvalid` — configured but corrupt (fail
    ///   closed)
    /// - `PreconditionError::NotArmed` — configured but unarmed (fail closed)
    async fn assess(
        &self,
        execution_id: uuid::Uuid,
        step: &DispatchStep,
    ) -> Result<PreconditionVerdict, PreconditionError>;

    /// Whether the gate is armed.
    ///
    /// `Ok(true)` — preconditions are present and loadable. `Ok(false)` — no
    /// preconditions configured (the gate is a no-op). `Err` — configured but
    /// unarmable (fail closed).
    ///
    /// # Errors
    /// - `PreconditionError::ConfigInvalid` — configured but corrupt /
    ///   over-cap (fail closed)
    async fn is_armed(&self) -> Result<bool, PreconditionError>;
}
