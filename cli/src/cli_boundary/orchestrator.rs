//! Orchestrator builder — wires the engine's OrchestratorService for the CLI.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#orchestrator
//! Implements: OrchestratorBuilder component
//! Issue: issue-orchestratorbuilder

use tokio_util::sync::CancellationToken;

use rigorix_engine::orchestrator::application::service::OrchestratorService;

use crate::cli_boundary::config::CliConfig;
use crate::cli_boundary::error::CliError;

/// Build an `OrchestratorService` from CLI configuration.
///
/// Wires all internal engine dependencies using the engine's
/// `OrchestratorBuilderImpl`. Each sub-service must be created
/// via its factory and injected.
///
/// # Todo
///
/// Wire all 7 sub-services via the builder's `with_*` methods:
/// ```rust,ignore
/// use rigorix_engine::orchestrator::application::builder_impl::OrchestratorBuilderImpl;
///
/// OrchestratorBuilderImpl::new(orch_config)
///     .with_repo_root(repo_root)
///     .with_planning_pipeline(planning_pipeline)
///     .with_execution_service(execution_service)
///     .with_state_manager(state_manager)
///     .with_cancellation_service(cancellation_service)
///     .with_event_bus(event_bus)
///     .with_audit_service(audit_service)
///     .with_budget_service(budget_service)
///     .build().await
/// ```
pub async fn build_orchestrator(
    config: CliConfig,
    cancellation_token: CancellationToken,
    repo_root: String,
) -> Result<Box<dyn OrchestratorService>, CliError> {
    let _ = (config, cancellation_token, repo_root);
    Err(CliError::NotImplemented(
        "build_orchestrator — engine sub-services need wiring via OrchestratorBuilderImpl".into(),
    ))
}
