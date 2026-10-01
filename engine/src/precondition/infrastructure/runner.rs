//! PreconditionRunner — deterministic argv execution of an operator check.
//!
//! @canonical .pi/architecture/modules/consequence-gating.md#command-contract
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Implements: Contract Freeze — PreconditionRunner trait + ProcessPreconditionRunner
//! Issue: #938 (consequence-gating epic — contract freeze); behavior closed in
//!   ISSUE-CONSEQUENCE-GATING-3 (PreconditionRunner)
//!
//! The runner executes one precondition's `command` deterministically. The
//! contract is frozen (ADR-017 §R1):
//!
//! - **argv only**: `command` is an argv array, never a shell; no string
//!   interpolation of step values into argv
//! - **stdin**: the JSON object `{ "precondition_id", "execution_id", "step",
//!   "tool", "parameters" }` (see
//!   [`PreconditionCheckInput`](crate::precondition::application::PreconditionCheckInput))
//! - **env**: `RIGORIX_PRECONDITION_ID`, `RIGORIX_EXECUTION_ID`,
//!   `RIGORIX_STEP_NAME`, `RIGORIX_TOOL`
//! - **exit 0** → `passed`; **non-zero** → `failed`;
//!   **timeout / spawn failure** → `error`
//! - **trust boundary**: `command[0]` must resolve **outside the
//!   agent-writable workspace**; if it resolves inside, the run is refused with
//!   `error` (a check the agent can edit is no check)
//! - `failed` and `error` are recorded distinctly; both refuse — there is no
//!   allow-on-error mode
//! - stdout is captured only when the precondition sets `capture_output = true`
//!   (truncated, redacted); it is never captured by default
//!
//! # Implementation
//! Method bodies are `todo!()` stubs — behavior lands in the
//! PreconditionRunner implementation issue.

use std::path::{Path, PathBuf};

use async_trait::async_trait;

use crate::precondition::application::PreconditionCheckInput;
use crate::precondition::domain::{Precondition, PreconditionError, PreconditionOutcome};

/// The result of running one precondition check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PreconditionRun {
    /// The distinct outcome: `passed` (exit 0), `failed` (non-zero), `error`
    /// (timeout / spawn / trust boundary).
    pub outcome: PreconditionOutcome,
    /// Process exit code, when a process actually ran. `None` for a
    /// pre-spawn refusal.
    pub exit_code: Option<i32>,
    /// Captured (truncated, redacted) stdout — present only when the
    /// precondition sets `capture_output = true`.
    pub stdout: Option<String>,
}

/// Deterministic precondition-check runner.
#[async_trait]
pub trait PreconditionRunner: Send + Sync {
    /// Run `precondition`'s command for the given check input.
    ///
    /// # Returns
    /// - `Ok(PreconditionRun { outcome: Passed, .. })` — exit 0
    /// - `Ok(PreconditionRun { outcome: Failed, .. })` — non-zero exit
    /// - `Ok(PreconditionRun { outcome: Error, .. })` — timeout / spawn /
    ///   trust-boundary (indeterminate → refuse)
    ///
    /// # Errors
    /// - `PreconditionError::TrustBoundary` — `command[0]` resolves inside the
    ///   agent-writable workspace (fail closed)
    /// - `PreconditionError::Spawn` — the process could not be spawned
    /// - `PreconditionError::Timeout` — the wall-clock timeout elapsed
    async fn run(
        &self,
        precondition: &Precondition,
        input: &PreconditionCheckInput,
    ) -> Result<PreconditionRun, PreconditionError>;
}

/// Production runner executing a real child process.
///
/// Constructed with the **agent-writable workspace root** so the trust-boundary
/// check (`command[0]` resolves outside it) can be enforced before spawn.
pub struct ProcessPreconditionRunner {
    /// Root of the agent-writable workspace. A `command[0]` resolving inside
    /// this root is refused.
    #[allow(dead_code)] // consumed by the upcoming `run` implementation
    workspace_root: PathBuf,
}

impl ProcessPreconditionRunner {
    /// Create the runner over the agent-writable workspace root.
    pub fn new(workspace_root: impl Into<PathBuf>) -> Self {
        Self {
            workspace_root: workspace_root.into(),
        }
    }

    /// The workspace root the trust-boundary check is relative to.
    pub fn workspace_root(&self) -> &Path {
        &self.workspace_root
    }
}

#[async_trait]
impl PreconditionRunner for ProcessPreconditionRunner {
    async fn run(
        &self,
        _precondition: &Precondition,
        _input: &PreconditionCheckInput,
    ) -> Result<PreconditionRun, PreconditionError> {
        todo!(
            "ISSUE-CONSEQUENCE-GATING-3: trust-boundary check, spawn argv (no shell), JSON stdin, env, wall-clock timeout, exit mapping"
        )
    }
}
