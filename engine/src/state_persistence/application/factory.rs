//! Factory interfaces for constructing State Persistence domain objects.
//!
//! @canonical .pi/architecture/modules/state-persistence.md
//! Implements: Contract Freeze — StateManagerFactory and GraphManagerFactory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of StateManagerService and
//! GraphManagerService instances with appropriate storage paths, locking
//! strategies, and configuration.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured service instance
//! - Configuration is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;
use std::path::PathBuf;

use crate::state_persistence::domain::StateError;

use super::service::{GraphManagerService, StateManagerService};

/// Factory for constructing `StateManagerService` instances.
///
/// Handles creation of the state manager with the appropriate storage
/// directory, file locking strategy, and configuration.
#[async_trait]
pub trait StateManagerFactory: Send + Sync {
    /// Create a `StateManagerService` instance.
    ///
    /// Initialises the state directory (creating it if it doesn't exist)
    /// and sets up the storage backend with the given configuration.
    async fn create(
        &self,
        state_dir: PathBuf,
        config: CreateStateManagerConfig,
    ) -> Result<Box<dyn StateManagerService>, StateError>;
}

/// Configuration for creating a `StateManagerService` instance.
#[derive(Debug, Clone)]
pub struct CreateStateManagerConfig {
    /// Whether to enable cross-process file locking.
    /// When enabled, uses `fd-lock` to prevent concurrent access from
    /// multiple processes.
    pub enable_cross_process_locking: bool,

    /// Maximum number of concurrent state save operations.
    /// Controls the `tokio::sync::Semaphore` permits.
    pub max_concurrent_saves: usize,

    /// Whether to create the state directory if it doesn't exist.
    pub create_dir_if_missing: bool,
}

impl Default for CreateStateManagerConfig {
    fn default() -> Self {
        Self {
            enable_cross_process_locking: true,
            max_concurrent_saves: 4,
            create_dir_if_missing: true,
        }
    }
}

/// Factory for constructing `GraphManagerService` instances.
///
/// Handles creation of the graph manager with the appropriate storage
/// path and configuration for persisting execution graphs.
#[async_trait]
pub trait GraphManagerFactory: Send + Sync {
    /// Create a `GraphManagerService` instance.
    ///
    /// Initialises the graph storage directory (creating it if it doesn't
    /// exist) and sets up the storage backend.
    async fn create(
        &self,
        graph_dir: PathBuf,
        config: CreateGraphManagerConfig,
    ) -> Result<Box<dyn GraphManagerService>, StateError>;
}

/// Configuration for creating a `GraphManagerService` instance.
#[derive(Debug, Clone)]
pub struct CreateGraphManagerConfig {
    /// Whether to create the graph directory if it doesn't exist.
    pub create_dir_if_missing: bool,

    /// Maximum number of graph records to keep.
    /// When exceeded, the oldest records are purged.
    pub max_graph_records: Option<u32>,
}

impl Default for CreateGraphManagerConfig {
    fn default() -> Self {
        Self {
            create_dir_if_missing: true,
            max_graph_records: Some(1000),
        }
    }
}
