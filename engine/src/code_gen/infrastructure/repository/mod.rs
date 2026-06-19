//! Repository interfaces for the Code Generation Pipeline.
//!
//! @canonical .pi/architecture/modules/code-generation.md
//! Implements: Contract Freeze — CodeGenEventRepository trait
//! Issue: #424
//!
//! Repositories abstract data access behind interfaces, allowing
//! implementations to use various storage backends without coupling
//! domain logic to infrastructure.
//!
//! # Contract (Frozen)
//! - All repository methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;

use crate::code_gen::domain::error::CodeGenError;
use crate::code_gen::domain::event::CodeGenEvent;

/// Repository for persisting and retrieving code generation events.
#[async_trait]
///
/// Provides append-only storage of code generation events for audit
/// trail and observability.
pub trait CodeGenEventRepository: Send + Sync {
    /// Record a code generation event.
    async fn record_event(&self, event: &CodeGenEvent) -> Result<(), CodeGenError>;

    /// Query code generation events for a session.
    async fn query_by_session(&self, session_id: &str) -> Result<Vec<CodeGenEvent>, CodeGenError>;

    /// Query code generation events for a file path.
    async fn query_by_path(&self, file_path: &str) -> Result<Vec<CodeGenEvent>, CodeGenError>;

    /// Get the total count of recorded events.
    async fn event_count(&self) -> Result<u64, CodeGenError>;

    /// Prune events older than the given timestamp.
    async fn prune(&self, older_than: chrono::DateTime<chrono::Utc>) -> Result<u64, CodeGenError>;
}
