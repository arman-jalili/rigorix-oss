//! Repository interfaces for the Code Graph bounded context.
//!
//! @canonical .pi/architecture/modules/code-graph.md
//! Implements: Contract Freeze — CodeGraphRepository trait
//! Issue: issue-contract-freeze
//!
//! CodeGraph records are persisted for crash recovery, audit trails,
//! and visualization re-use. Repositories abstract away the storage
//! backend (filesystem, database, S3, etc.).
//!
//! # Contract (Frozen)
//! - All repository methods are async
//! - All methods return domain error types
//! - No framework-specific annotations on trait definitions

use async_trait::async_trait;
use uuid::Uuid;

use crate::code_graph::domain::{CodeGraph, CodeGraphError};

/// Repository for CRUD operations on CodeGraph records.
///
/// The default implementation uses the local filesystem. Custom
/// implementations may use a database, S3, or any other storage backend.
///
/// # Contract (Frozen)
/// - `save` persists a CodeGraph for later retrieval
/// - `load` retrieves a CodeGraph by its ID
/// - `delete` removes a CodeGraph (idempotent)
/// - `list_ids` returns all available graph IDs
/// - `exists` checks if a graph exists in storage
/// - `count` returns the total number of stored graphs
#[async_trait]
pub trait CodeGraphRepository: Send + Sync {
    /// Persist a CodeGraph to storage.
    ///
    /// Must be atomic — either the full graph is persisted or the
    /// previous state remains intact.
    async fn save(&self, graph: &CodeGraph) -> Result<(), CodeGraphError>;

    /// Load a CodeGraph from storage by its ID.
    ///
    /// The graph ID is typically embedded as part of the graph's metadata.
    /// Returns `CodeGraphError::InvalidOperation` with reason "Graph not found"
    /// if the graph does not exist.
    async fn load(&self, graph_id: Uuid) -> Result<CodeGraph, CodeGraphError>;

    /// Check if a CodeGraph exists in storage.
    async fn exists(&self, graph_id: Uuid) -> Result<bool, CodeGraphError>;

    /// Delete a CodeGraph from storage.
    ///
    /// Idempotent — returns `Ok(())` even if the graph does not exist.
    async fn delete(&self, graph_id: Uuid) -> Result<(), CodeGraphError>;

    /// List all available graph IDs in storage.
    async fn list_ids(&self) -> Result<Vec<Uuid>, CodeGraphError>;

    /// List available graph IDs with pagination.
    async fn list_ids_paginated(
        &self,
        limit: u32,
        offset: u32,
    ) -> Result<Vec<Uuid>, CodeGraphError>;

    /// Count the number of CodeGraph records in storage.
    async fn count(&self) -> Result<u64, CodeGraphError>;

    /// Search for graphs by name or source.
    async fn search(
        &self,
        query: &str,
        limit: u32,
    ) -> Result<Vec<CodeGraph>, CodeGraphError>;
}
