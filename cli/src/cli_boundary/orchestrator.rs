//! Orchestrator builder — wires the engine's OrchestratorService for the CLI.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#orchestrator
//! Implements: Contract Freeze — OrchestratorBuilder component
//! Issue: issue-contract-freeze
//!
//! # Contract (Frozen)
//!
//! The CLI builds its own `OrchestratorService` via `OrchestratorBuilder`.
//! This is the single point where CLI configuration meets the engine's
//! top-level entry point. The builder:
//!
//! 1. Receives a merged `Config` from the config loader
//! 2. Extracts `OrchestratorConfig` from it
//! 3. Wires internal engine dependencies (PlanningPipeline,
//!    ParallelExecutionService, StateManagerService, CancellationService,
//!    EventBus, AuditService)
//! 4. Returns a `Box<dyn OrchestratorService>`
//!
//! The same builder is used by both `cli_boundary::dispatch` and `tui::run`.

use tokio_util::sync::CancellationToken;
use rigorix_engine::orchestrator::application::service::OrchestratorService;

use crate::cli_boundary::config::CliConfig;

/// Build an `OrchestratorService` from CLI configuration.
///
/// This wraps the engine's own `OrchestratorBuilder` (defined in
/// `engine::orchestrator::application::builder`) with CLI-specific
/// setup logic.
///
/// # Parameters
///
/// * `config` — The merged CLI configuration (TOML + env + flags).
/// * `cancellation_token` — Shared cancellation handle installed by
///   the signal handler.
/// * `repo_root` — Root path of the repository for execution metadata.
///
/// # Returns
///
/// A fully initialised `OrchestratorService` ready for `run()`, `plan_only()`,
/// `cancel()`, and `status()` calls.
///
/// # Errors
///
/// Returns an error if:
/// - Required configuration fields are missing
/// - The orchestrator builder validation fails
/// - Engine dependency wiring fails
pub async fn build_orchestrator(
    config: CliConfig,
    cancellation_token: CancellationToken,
    repo_root: String,
) -> Result<Box<dyn OrchestratorService>, crate::cli_boundary::error::CliError> {
    // Placeholder: defers to engine's OrchestratorBuilder implementation.
    // Implementation issue: extract OrchestratorConfig from CliConfig,
    // then call engine::orchestrator::OrchestratorBuilder::new(orchestrator_config)
    // .with_repo_root(repo_root)
    // .build()
    // .await
    let _ = (config, cancellation_token, repo_root);
    Err(crate::cli_boundary::error::CliError::NotImplemented(
        "build_orchestrator".into(),
    ))
}
