//! Repository interfaces for the Failure Parser bounded context.
//!
//! @canonical .pi/architecture/modules/failure-parser.md
//! Implements: Contract Freeze — ParserConfigRepository, FailureLogRepository traits
//! Issue: #495
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use filesystem, environment, or mock storage
//! without coupling domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions
//! - Implementations are hidden behind these interfaces

use async_trait::async_trait;

use crate::failure_parser::domain::{FailureParserError, TemplateFailure};

/// Repository for storing and retrieving parser configuration.
///
/// Supports custom parser registrations and tool-specific settings
/// that persist across sessions.
#[async_trait]
pub trait ParserConfigRepository: Send + Sync {
    /// Store a custom tool-to-parser mapping.
    ///
    /// Allows users to register custom parsers for tools that are not
    /// natively supported.
    async fn store_custom_parser(
        &self,
        tool: &str,
        parser_type: &str,
    ) -> Result<(), FailureParserError>;

    /// Retrieve the parser type for a given tool.
    ///
    /// Returns `None` if no custom parser is configured for this tool.
    async fn get_custom_parser(&self, tool: &str) -> Result<Option<String>, FailureParserError>;

    /// Retrieve all custom parser registrations.
    ///
    /// Returns a map of tool → parser_type.
    async fn get_all_custom_parsers(
        &self,
    ) -> Result<std::collections::HashMap<String, String>, FailureParserError>;

    /// Remove a custom parser registration.
    async fn remove_custom_parser(&self, tool: &str) -> Result<bool, FailureParserError>;
}

/// Repository for persisting parse results (for audit/replay).
///
/// Optional — only needed if parsing traceability is required
/// for debugging and quality monitoring.
#[async_trait]
pub trait FailureLogRepository: Send + Sync {
    /// Record a parsed failure for audit.
    async fn record_failure(
        &self,
        tool: &str,
        failure: &TemplateFailure,
    ) -> Result<(), FailureParserError>;

    /// Record a batch of parsed failures.
    async fn record_failures_batch(
        &self,
        tool: &str,
        failures: &[TemplateFailure],
    ) -> Result<(), FailureParserError>;

    /// Get recent failure history for a given tool.
    ///
    /// Returns the most recent `limit` entries.
    async fn get_recent_failures(
        &self,
        tool: &str,
        limit: usize,
    ) -> Result<Vec<(String, TemplateFailure)>, FailureParserError>;

    /// Get failure frequency statistics for a tool.
    ///
    /// Returns a map of failure variant name → count.
    async fn get_failure_stats(
        &self,
        tool: &str,
    ) -> Result<std::collections::HashMap<String, usize>, FailureParserError>;
}
