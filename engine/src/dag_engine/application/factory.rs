//! Factory interfaces for constructing DAG Engine service instances.
//!
//! @canonical .pi/architecture/modules/dag-engine.md
//! Implements: Contract Freeze — DagGraphFactory and DagPlanningFactory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of DagGraphService and
//! DagPlanningService instances with appropriate storage paths, locking
//! strategies, and configuration.
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured service instance
//! - Configuration is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;

use crate::dag_engine::domain::DagError;

use super::service::{DagGraphService, DagPlanningService};

/// Factory for constructing `DagGraphService` instances.
///
/// Handles creation of the graph service with appropriate storage
/// configuration for persisting TaskGraph records.
#[async_trait]
pub trait DagGraphFactory: Send + Sync {
    /// Create a `DagGraphService` instance.
    ///
    /// Initialises the graph storage directory (creating it if it doesn't
    /// exist) and configures the graph persistence backend.
    async fn create(
        &self,
        config: DagGraphFactoryConfig,
    ) -> Result<Box<dyn DagGraphService>, DagError>;
}

/// Configuration for creating a `DagGraphService` instance.
#[derive(Debug, Clone)]
pub struct DagGraphFactoryConfig {
    /// Directory path for persisting TaskGraph records.
    pub graph_storage_dir: Option<String>,

    /// Maximum number of concurrent graph construction operations.
    pub max_concurrent_operations: usize,

    /// Whether to create the storage directory if it doesn't exist.
    pub create_dir_if_missing: bool,
}

impl Default for DagGraphFactoryConfig {
    fn default() -> Self {
        Self {
            graph_storage_dir: None,
            max_concurrent_operations: 4,
            create_dir_if_missing: true,
        }
    }
}

/// Factory for constructing `DagPlanningService` instances.
///
/// Handles creation of the planning service with audit trail
/// integration and policy configuration for plan comparisons.
#[async_trait]
pub trait DagPlanningFactory: Send + Sync {
    /// Create a `DagPlanningService` instance.
    ///
    /// Configures the planning service with the given settings
    /// for audit integration and plan comparison policies.
    async fn create(
        &self,
        config: DagPlanningFactoryConfig,
    ) -> Result<Box<dyn DagPlanningService>, DagError>;
}

/// Configuration for creating a `DagPlanningService` instance.
#[derive(Debug, Clone)]
pub struct DagPlanningFactoryConfig {
    /// Whether to emit audit events for plan comparisons.
    pub emit_audit_events: bool,

    /// Whether to record plan diffs for historical analysis.
    pub record_plan_diffs: bool,

    /// Maximum number of plan diffs to retain in history.
    pub max_plan_history: Option<u32>,
}

impl Default for DagPlanningFactoryConfig {
    fn default() -> Self {
        Self {
            emit_audit_events: true,
            record_plan_diffs: true,
            max_plan_history: Some(1000),
        }
    }
}
