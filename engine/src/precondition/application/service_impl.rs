//! PreconditionServiceImpl — the concrete dispatch-time precondition service.
//!
//! @canonical .pi/architecture/modules/consequence-gating.md#preconditionservice
//! Implements: Contract Freeze — PreconditionServiceImpl stub
//! Issue: #938 (consequence-gating epic — contract freeze); behavior closed in
//!   ISSUE-CONSEQUENCE-GATING-4 (`evaluate`) and -3 (runner)
//!
//! The implementation loads the operator config per dispatch through the
//! injected `PreconditionRepository`, matches each `Precondition` against the
//! about-to-dispatch step, enforces `require_params` presence, runs the check
//! through the injected `PreconditionRunner`, and folds the outcomes into a
//! single [`PreconditionVerdict`](crate::precondition::domain::PreconditionVerdict).
//!
//! Method bodies are `todo!()` stubs — behavior lands in the implementation
//! issues. The construction shape (repository + runner injected, never
//! defaulted) is frozen here.

use async_trait::async_trait;

use crate::precondition::domain::{PreconditionError, PreconditionVerdict};
use crate::precondition::infrastructure::PreconditionRunner;
use crate::precondition::infrastructure::repository::PreconditionRepository;

use super::dto::DispatchStep;
use super::service::PreconditionService;

/// Default `PreconditionService` implementation.
///
/// # Construction
/// - `new(repository, runner)` — inject the config repository (filesystem
///   `.rigorix/preconditions.toml`, or an in-memory test double) and the
///   deterministic argv runner. Both are always injected — never defaulted
///   inside the service.
pub struct PreconditionServiceImpl {
    /// Config repository — re-read per dispatch so a change at ΔN is observed.
    #[allow(dead_code)] // consumed by the upcoming `evaluate` implementation
    repository: Box<dyn PreconditionRepository>,
    /// Check runner — argv only, JSON stdin, timeout, trust boundary.
    #[allow(dead_code)] // consumed by the upcoming `evaluate` implementation
    runner: Box<dyn PreconditionRunner>,
}

impl PreconditionServiceImpl {
    /// Create the service over the given repository and runner.
    pub fn new(
        repository: Box<dyn PreconditionRepository>,
        runner: Box<dyn PreconditionRunner>,
    ) -> Self {
        Self { repository, runner }
    }
}

#[async_trait]
impl PreconditionService for PreconditionServiceImpl {
    async fn evaluate(
        &self,
        _execution_id: uuid::Uuid,
        _step: &DispatchStep,
    ) -> Result<PreconditionVerdict, PreconditionError> {
        todo!(
            "ISSUE-CONSEQUENCE-GATING-4: load config, match step, enforce require_params, run check, fold verdict"
        )
    }

    async fn is_configured(&self) -> Result<bool, PreconditionError> {
        todo!(
            "ISSUE-CONSEQUENCE-GATING-4: load config and report whether any precondition is configured"
        )
    }
}
