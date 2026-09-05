//! Factory interfaces for constructing Execution Engine service instances.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: Contract Freeze — ParallelExecutionFactory and RetryEvaluationFactory traits
//! Issue: issue-contract-freeze
//!
//! Factories encapsulate the construction of ParallelExecutionService and
//! RetryEvaluationService instances with appropriate configuration, dependencies,
//! and integration points (event bus, cancellation, enforcement).
//!
//! # Contract (Frozen)
//! - Every factory method returns a configured service instance
//! - Configuration is applied during construction
//! - No mutable state in factory implementations

use async_trait::async_trait;
use std::sync::Arc;

use crate::event_system::application::EventBusService;
use crate::execution_engine::domain::ExecutionError;
use crate::hooks::application::service::HookRunnerService;
use crate::permission::application::enforcer::PermissionEnforcer;

use super::service::{ParallelExecutionService, RetryEvaluationService};

/// Factory for constructing `ParallelExecutionService` instances.
///
/// Handles creation of the parallel executor with all necessary dependencies:
/// - Tool system for executing node tools
/// - Event bus for emitting execution events
/// - Cancellation token for graceful shutdown
/// - Enforcement service for limiting operations
/// - Retry evaluation service for retry decisions
#[async_trait]
pub trait ParallelExecutionFactory: Send + Sync {
    /// Create a `ParallelExecutionService` instance.
    ///
    /// Builds the executor with all configuration settings and integration
    /// points wired together.
    async fn create(
        &self,
        config: ParallelExecutionFactoryConfig,
    ) -> Result<Box<dyn ParallelExecutionService>, ExecutionError>;
}

/// Configuration for creating a `ParallelExecutionService` instance.
#[derive(Clone)]
/// ADR-011 production approval binding setup.
///
/// When present, the engine factory attaches an `ApprovalServiceImpl`
/// (repository + run key + TTL) with a session-graph intent resolver to the
/// executor, turning on approval capture/verification/consume at the runtime
/// choke point. `None` keeps the legacy `session.approved` gate.
pub struct ApprovalBindingSetup {
    /// Durable approval-record repository (node-scoped).
    pub repository:
        std::sync::Arc<dyn crate::approval::infrastructure::repository::ApprovalRepository>,
    /// Run key — must equal the envelope HMAC key (ADR-011 §key).
    pub run_key: Vec<u8>,
    /// Approval lifetime (expires_at = decided_at + ttl).
    pub ttl_seconds: u64,
}

pub struct ParallelExecutionFactoryConfig {
    /// The parallel executor configuration.
    pub executor_config: crate::execution_engine::domain::ParallelExecutorConfig,

    /// Whether to register event bus subscribers for execution events.
    pub register_event_handlers: bool,

    /// Whether to enable progress callbacks.
    pub enable_progress_callbacks: bool,

    /// Event bus channel capacity for execution events.
    pub event_channel_capacity: usize,

    /// The event bus service for publishing execution lifecycle events.
    pub event_bus: Option<Arc<dyn EventBusService>>,

    /// Optional permission enforcer for mode-based gating of tool calls.
    ///
    /// When set, every tool invocation (including bash commands) is checked
    /// against the active permission mode before execution. When `None`, no
    /// permission gating is applied.
    pub permission_enforcer: Option<Arc<dyn PermissionEnforcer>>,

    /// Optional hook runner for PreToolUse/PostToolUse interception.
    ///
    /// When set, every tool execution runs the configured shell hooks
    /// (which can block, override permissions, or enrich audit context).
    pub hook_runner: Option<Arc<dyn HookRunnerService>>,

    /// ADR-011: optional approval binding (see [`ApprovalBindingSetup`]).
    pub approval_binding: Option<ApprovalBindingSetup>,

    /// R3: optional sequence-policy prefix gate (see module doc §R3).
    ///
    /// When set, the dispatch loop evaluates the session's completed prefix
    /// plus each ready node before dispatch: a matched `deny` rule fails the
    /// node before its tool is called; a matched `promote` rule routes the
    /// node into the existing approval pause. When `None`, no sequence-policy
    /// gating is applied (status quo).
    pub sequence_policy: Option<
        std::sync::Arc<dyn crate::sequence_policy::application::service::SequencePolicyService>,
    >,
}

impl Default for ParallelExecutionFactoryConfig {
    fn default() -> Self {
        Self {
            executor_config: crate::execution_engine::domain::ParallelExecutorConfig::default(),
            register_event_handlers: true,
            enable_progress_callbacks: true,
            event_channel_capacity: 1024,
            event_bus: None,
            permission_enforcer: None,
            hook_runner: None,
            approval_binding: None,
            sequence_policy: None,
        }
    }
}

/// Factory for constructing `RetryEvaluationService` instances.
///
/// Handles creation of the retry evaluation service with retry policy defaults
/// and strategy mappings for different failure types.
#[async_trait]
pub trait RetryEvaluationFactory: Send + Sync {
    /// Create a `RetryEvaluationService` instance.
    ///
    /// Configures the service with default retry policies and strategy
    /// mappings for various failure classification types.
    async fn create(
        &self,
        config: RetryEvaluationFactoryConfig,
    ) -> Result<Box<dyn RetryEvaluationService>, ExecutionError>;
}

/// Configuration for creating a `RetryEvaluationService` instance.
#[derive(Debug, Clone)]
pub struct RetryEvaluationFactoryConfig {
    /// Default retry policy to use when no policy is specified.
    pub default_policy: crate::execution_engine::domain::RetryPolicy,

    /// Mapping of failure types to preferred RetryStrategy.
    /// Keys are failure type strings (e.g., "transient", "compile_error").
    pub failure_strategy_mapping: Vec<FailureStrategyOverride>,

    /// Whether to enable detailed logging of retry decisions.
    pub enable_decision_logging: bool,
}

impl Default for RetryEvaluationFactoryConfig {
    fn default() -> Self {
        Self {
            default_policy: crate::execution_engine::domain::RetryPolicy::default(),
            failure_strategy_mapping: Vec::new(),
            enable_decision_logging: true,
        }
    }
}

/// Override mapping between a failure type and its preferred retry strategy.
#[derive(Debug, Clone)]
pub struct FailureStrategyOverride {
    /// The failure type string (e.g., "transient", "compile_error").
    pub failure_type: String,
    /// The preferred retry strategy for this failure type.
    pub preferred_strategy: crate::execution_engine::domain::RetryStrategy,
}

impl ApprovalBindingSetup {
    /// Build an approval binding setup from environment configuration.
    ///
    /// Enabled by `RIGORIX_APPROVAL_BINDING=1|true`. The run key defaults to
    /// `RIGORIX_HMAC_KEY` (must equal the envelope HMAC key — ADR-011 §key);
    /// TTL via `RIGORIX_APPROVAL_TTL_SECONDS` (default 3600). Records are
    /// persisted to `<repo_root>/.rigorix/approvals.json` (atomic writes,
    /// cross-process resume).
    ///
    /// Returns `None` when disabled — the executor keeps the legacy
    /// `session.approved` gate (zero behavior change).
    pub fn from_env(repo_root: &std::path::Path) -> Option<Self> {
        let enabled = std::env::var("RIGORIX_APPROVAL_BINDING").ok();
        let on = matches!(
            enabled.as_deref(),
            Some("1") | Some("true") | Some("TRUE") | Some("yes")
        );
        if !on {
            return None;
        }
        let run_key = std::env::var("RIGORIX_HMAC_KEY")
            .ok()
            .filter(|k| !k.is_empty())
            .map(|k| k.into_bytes());
        let Some(run_key) = run_key else {
            tracing::warn!(
                "RIGORIX_APPROVAL_BINDING=1 but RIGORIX_HMAC_KEY is unset — approval binding disabled"
            );
            return None;
        };
        let ttl_seconds = std::env::var("RIGORIX_APPROVAL_TTL_SECONDS")
            .ok()
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3600);
        let store_dir = repo_root.join(".rigorix");
        let _ = std::fs::create_dir_all(&store_dir);
        let repository = std::sync::Arc::new(
            crate::approval::infrastructure::repository::FileBackedApprovalRepository::open(
                store_dir.join("approvals.json"),
            )
            .expect("approval store must open"),
        );
        tracing::info!(
            ttl_seconds,
            "approval binding ENABLED (ADR-011) — records at {}",
            store_dir.join("approvals.json").display()
        );
        Some(Self {
            repository,
            run_key,
            ttl_seconds,
        })
    }
}
