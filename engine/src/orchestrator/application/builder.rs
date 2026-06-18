//! OrchestratorBuilder — Constructs an OrchestratorService from Config.
//!
//! @canonical .pi/architecture/modules/orchestrator.md#builder
//! Implements: Contract Freeze — OrchestratorBuilder trait
//! Issue: #338
//!
//! Builder pattern for constructing an `OrchestratorService` from a `Config`,
//! wiring all internal dependencies (PlanningPipeline, ParallelExecutionService,
//! StateManagerService, CancellationService, EventBus, AuditService).
//!
//! Usage:
//! ```rust,ignore
//! let orchestrator = OrchestratorBuilder::new(config)
//!     .with_repo_root(repo_root)
//!     .with_enforcement_preset(enforcement_preset)
//!     .build()
//!     .await?;
//!
//! let result = orchestrator.run(RunInput { intent }).await?;
//! ```
//!
//! # Contract (Frozen)
//! - Builder methods return `Self` for chaining
//! - `build()` is async and performs validation
//! - No mutable state exposed after `build()`
//! - Default behaviour when optional fields are omitted

use async_trait::async_trait;
use std::sync::Arc;

use crate::orchestrator::domain::OrchestratorConfig;

use super::service::OrchestratorService;

/// Builder for constructing an `OrchestratorService`.
///
/// Wires all internal dependencies and configuration. The resulting service
/// is fully initialised and ready for `run()` calls.
#[async_trait]
pub trait OrchestratorBuilder: Send + Sync {
    /// Create a new builder with the given orchestrator configuration.
    fn new(config: OrchestratorConfig) -> Self
    where
        Self: Sized;

    /// Set the repository root path.
    ///
    /// Used to resolve relative paths in execution context metadata.
    /// Required — omitted `build()` calls will return an error.
    fn with_repo_root(self, repo_root: String) -> Self
    where
        Self: Sized;

    /// Set the enforcement preset.
    ///
    /// Controls execution limits (max nodes, max LLM calls, etc.).
    /// Optional — defaults to standard enforcement.
    fn with_enforcement_preset(self, preset: String) -> Self
    where
        Self: Sized;

    /// Set the LLM budget for this execution.
    ///
    /// Optional — defaults to unlimited.
    fn with_llm_budget(self, max_calls: u32, max_tokens: u64) -> Self
    where
        Self: Sized;

    /// Inject a CodeGraphService for module-level dependency graph construction.
    ///
    /// Optional — if omitted, no module dependency graph will be available
    /// for the template generation pipeline.
    fn with_code_graph_service(self, svc: Arc<dyn crate::code_graph::application::CodeGraphService>) -> Self
    where
        Self: Sized;

    /// Build the `OrchestratorService`.
    ///
    /// Validates configuration and wires all dependencies.
    /// Returns an error if required fields are missing.
    async fn build(self) -> Result<Box<dyn OrchestratorService>, crate::orchestrator::domain::OrchestratorError>
    where
        Self: Sized;
}
