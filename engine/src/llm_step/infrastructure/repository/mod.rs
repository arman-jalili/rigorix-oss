//! Repository interfaces and implementations for the LLM Step bounded context.
//!
//! @canonical .pi/architecture/modules/llm-step.md
//! Implements: Contract Freeze — LlmGenerateNodeRepository trait
//! Issue: issue-contract-freeze
//!
//! LlmGenerateNode records are persisted for crash recovery and execution
//! replay. LlmStepEvent records are persisted for audit trail analysis.
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

pub mod node_repository_impl;

use async_trait::async_trait;
use uuid::Uuid;

use crate::llm_step::domain::{LlmGenerateNode, LlmStepError};

/// Repository for CRUD operations on LlmGenerateNode records.
///
/// The default implementation uses in-memory storage. Custom
/// implementations may use a database, filesystem, or any other
/// storage backend.
///
/// # Contract (Frozen)
/// - `save` persists an LlmGenerateNode for later retrieval
/// - `load` retrieves an LlmGenerateNode by its ID
/// - `delete` removes an LlmGenerateNode (idempotent)
/// - `list_ids` returns all available node IDs
#[async_trait]
pub trait LlmGenerateNodeRepository: Send + Sync {
    /// Persist an LlmGenerateNode to storage.
    ///
    /// Must be atomic — either the full node is persisted or the
    /// previous state remains intact.
    async fn save(&self, node: &LlmGenerateNode) -> Result<(), LlmStepError>;

    /// Load an LlmGenerateNode from storage by its ID.
    ///
    /// Returns `LlmStepError::MissingDependency` if the node does not exist.
    async fn load(&self, node_id: Uuid) -> Result<LlmGenerateNode, LlmStepError>;

    /// Check if an LlmGenerateNode exists in storage.
    async fn exists(&self, node_id: Uuid) -> Result<bool, LlmStepError>;

    /// Delete an LlmGenerateNode from storage.
    ///
    /// Idempotent — returns `Ok(())` even if the node does not exist.
    async fn delete(&self, node_id: Uuid) -> Result<(), LlmStepError>;

    /// List all available generation node IDs.
    async fn list_ids(&self) -> Result<Vec<Uuid>, LlmStepError>;

    /// Count the number of LlmGenerateNode records in storage.
    async fn count(&self) -> Result<u64, LlmStepError>;

    /// Find nodes by execution ID.
    ///
    /// Returns all generation nodes associated with the given execution.
    async fn find_by_execution(
        &self,
        execution_id: Uuid,
    ) -> Result<Vec<LlmGenerateNode>, LlmStepError>;
}
