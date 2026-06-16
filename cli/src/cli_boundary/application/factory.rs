//! Factory interfaces for constructing CLI domain objects.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — CLI Factory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of complex CLI objects,
//! allowing implementations to inject dependencies and apply defaults
//! without exposing construction logic to callers.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured domain object
//! - Validation is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::cli_boundary::domain::error::CliError;
use crate::configuration::domain::config::CliConfig;

use super::dto::RunInput;
use super::service::{CliOrchestrator, ExecutionSession};

/// Factory for constructing `CliOrchestrator` instances.
///
/// Handles creation of the top-level CLI orchestrator with all
/// required dependencies: config loader, signal handler, engine
/// orchestrator, output formatter, and TUI renderer.
#[async_trait]
pub trait CliOrchestratorFactory: Send + Sync {
    /// Create a new `CliOrchestrator` with the merged CLI config.
    ///
    /// Loads configuration from CLI flags → env vars → rigorix.toml
    /// → engine defaults. Initializes tracing, signal handlers, and
    /// engine wiring.
    async fn create_default(&self) -> Result<Box<dyn CliOrchestrator>, CliError>;

    /// Create a `CliOrchestrator` with an explicit config path.
    ///
    /// Overrides automatic config file discovery with the given path.
    async fn create_with_config(
        &self,
        config_path: &str,
    ) -> Result<Box<dyn CliOrchestrator>, CliError>;

    /// Create a `CliOrchestrator` from a pre-loaded `CliConfig`.
    ///
    /// Useful for testing where config loading is mocked.
    async fn create_from_config(
        &self,
        config: CliConfig,
    ) -> Result<Box<dyn CliOrchestrator>, CliError>;
}

/// Factory for constructing `ExecutionSession` instances.
///
/// Each execution session manages a single `rigorix run` invocation.
#[async_trait]
pub trait ExecutionSessionFactory: Send + Sync {
    /// Create a new execution session for the given run input.
    ///
    /// The session is not started yet — call `start()` to begin.
    async fn create_session(&self, input: RunInput) -> Result<Box<dyn ExecutionSession>, CliError>;
}
