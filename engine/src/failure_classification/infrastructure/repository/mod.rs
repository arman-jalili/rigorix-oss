//! Repository interfaces for the Failure Classification bounded context.
//!
//! @canonical .pi/architecture/modules/failure-classification.md
//! Implements: Contract Freeze — PatternRepository trait
//! Issue: #33
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

use crate::failure_classification::domain::{FailureClassificationError, FailureType};

/// Repository for storing and retrieving custom classification patterns.
///
/// Implementations can store patterns in memory, on disk, or in a database.
/// Custom patterns take precedence over built-in patterns during classification.
///
/// # Security
/// - Implementations MUST validate pattern input against injection attacks
/// - Patterns are matched as substrings (case-insensitive) against error messages
/// - Pattern length must not exceed 1024 characters
#[async_trait]
pub trait PatternRepository: Send + Sync {
    /// Store a custom pattern-to-FailureType mapping.
    ///
    /// Returns the total number of stored patterns after insertion.
    /// Duplicate patterns are overwritten (last write wins).
    async fn store_pattern(
        &self,
        pattern: &str,
        target: FailureType,
    ) -> Result<u32, FailureClassificationError>;

    /// Retrieve the `FailureType` for a given pattern.
    ///
    /// Returns `None` if the pattern is not in the repository.
    async fn get_pattern(
        &self,
        pattern: &str,
    ) -> Result<Option<FailureType>, FailureClassificationError>;

    /// Retrieve all stored patterns.
    ///
    /// Returns a map of pattern → FailureType.
    async fn get_all_patterns(
        &self,
    ) -> Result<std::collections::HashMap<String, FailureType>, FailureClassificationError>;

    /// Remove a pattern from the repository.
    ///
    /// Returns `true` if the pattern existed and was removed, `false` otherwise.
    async fn remove_pattern(&self, pattern: &str) -> Result<bool, FailureClassificationError>;

    /// Clear all custom patterns, resetting to empty.
    async fn clear_patterns(&self) -> Result<(), FailureClassificationError>;
}

/// Repository for persisting classification results (for audit/replay).
///
/// Optional — only needed if classification traceability is required.
#[async_trait]
pub trait ClassificationLogRepository: Send + Sync {
    /// Record a classification result.
    async fn record_classification(
        &self,
        error_message: &str,
        failure_type: &FailureType,
    ) -> Result<(), FailureClassificationError>;

    /// Get recent classification history for a given error pattern.
    ///
    /// Returns the most recent `limit` entries.
    async fn get_classification_history(
        &self,
        error_pattern: &str,
        limit: usize,
    ) -> Result<Vec<(String, FailureType)>, FailureClassificationError>;
}
