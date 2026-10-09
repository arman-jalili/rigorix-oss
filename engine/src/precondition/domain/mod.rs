//! Domain entities and interfaces for the Consequence Gating bounded context.
//!
//! @canonical .pi/architecture/modules/precondition.md#ddd-layers
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — Precondition, FailureAction, SafetyCaps,
//!   PreconditionConfig, GatingMode, PreconditionError, PreconditionOutcome,
//!   PreconditionFinding, PreconditionChecked, PreconditionVerdict
//! Issue: #938 (consequence-gating epic — contract freeze)
//!
//! Pure business logic with zero framework imports (`thiserror`, `serde`,
//! `chrono`, and `uuid` value types only). The domain owns the operator's
//! config model, the distinct outcome taxonomy, the dispatch verdict, and the
//! typed failure modes.
//!
//! # Contract (Frozen)
//! - The primitive is exactly: **match a step → run a check → refuse on
//!   failure → record the outcome** (ADR-017 §Scope Guard). Loops, chained
//!   checks, retries that change a verdict, conditionals/policy DSL, and
//!   multi-step orchestration are explicitly **out of scope**
//! - The check is deterministic: no LLM, no network semantics in the engine,
//!   no retries that change the verdict
//! - `failed` and `error` both refuse and are recorded distinctly; there is no
//!   allow-on-error mode
//! - `PreconditionError::is_retriable()` is always `false`
//! - The `StepPredicate` matcher is reused from `sequence_policy`, never
//!   forked

pub mod error;
pub mod finding;
pub mod precondition;
pub mod verdict;

pub use error::PreconditionError;
pub use finding::{
    AttributionReason, PreconditionChecked, PreconditionFinding, PreconditionOutcome,
};
// ADR-017 R2 (`[gating].release_dependents_on_failure`) is an execution/graph
// concern: `GatingMode` now lives in `dag_engine::domain` (ISSUE-PF-REL-1). It is
// re-exported here because the operator config model (`PreconditionConfig.gating`)
// is parsed with it, but execution_engine reads the type from `dag_engine`.
pub use crate::dag_engine::domain::{GatingMode, default_release_dependents_on_failure};
pub use precondition::{
    DEFAULT_TIMEOUT_MS, FailureAction, Precondition, PreconditionConfig, SafetyCaps,
};
pub use verdict::PreconditionVerdict;
