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

/// R3 sequence-policy setup (ADR-013) — arms the runtime prefix gate.
///
/// Loads the operator-authored rule file (`.rigorix/sequence-policy.toml`,
/// same trust surface as `policy.toml` / `permissions.toml`) and builds a
/// `SequencePolicyService` that reads it per run. Absent file = fail-open-
/// absent (no gating — status quo); a present file gates the dispatch
/// prefix (deny fails before the tool is called, promote routes into the
/// approval pause).
pub struct SequencePolicySetup;

impl SequencePolicySetup {
    /// Build the sequence-policy service over the repo's rule file.
    ///
    /// Default config path: `<repo_root>/.rigorix/sequence-policy.toml`
    /// (override with `RIGORIX_SEQUENCE_POLICY_PATH`). Disable entirely
    /// with `RIGORIX_SEQUENCE_POLICY=0|false|no` (the file alone is the
    /// operator's opt-in, so the default is enabled-attempt — an absent
    /// file yields no rules and zero behavior change).
    ///
    /// Corrupt / over-cap files are NOT rejected here: the repository
    /// surfaces them per-run as `SequencePolicyError` and the evaluation
    /// choke points fail closed (plan refused / run halted before dispatch)
    /// — matching the frozen loading semantics.
    pub fn from_env(
        repo_root: &std::path::Path,
    ) -> Option<
        std::sync::Arc<dyn crate::sequence_policy::application::service::SequencePolicyService>,
    > {
        let flag = std::env::var("RIGORIX_SEQUENCE_POLICY").ok();
        let disabled = matches!(
            flag.as_deref(),
            Some("0") | Some("false") | Some("FALSE") | Some("no") | Some("NO")
        );
        if disabled {
            tracing::info!(
                "sequence_policy: disabled via RIGORIX_SEQUENCE_POLICY=0 — no R3 prefix gating"
            );
            return None;
        }
        let path = std::env::var("RIGORIX_SEQUENCE_POLICY_PATH")
            .ok()
            .filter(|p| !p.is_empty())
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|| repo_root.join(".rigorix").join("sequence-policy.toml"));
        tracing::info!(
            "sequence_policy: R3 prefix gate armed — rules read per-run from {} (absent file = no gating)",
            path.display()
        );
        let mut service =
            crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl::new(
                Box::new(
                    crate::sequence_policy::infrastructure::TomlSequencePolicyRepository::new(path),
                ),
            );
        // R7: attach the signed-execution-history port over the same repo's
        // `.rigorix/audit` envelope store — cross-run rules read prior runs'
        // signed evidence. A missing audit dir yields empty history (status
        // quo); the audit dir itself is created by the composition roots when
        // they persist envelopes.
        service = service.with_history(std::sync::Arc::new(
            crate::sequence_policy::infrastructure::EnvelopeHistoryAdapter::new(
                std::sync::Arc::new(
                    crate::audit::infrastructure::LocalAuditEnvelopeRepository::new(
                        repo_root.join(".rigorix").join("audit"),
                    ),
                ),
            ),
        ));
        tracing::info!(
            "sequence_policy: R7 cross-run history armed over {}/.rigorix/audit",
            repo_root.display()
        );
        Some(std::sync::Arc::new(service))
    }
}

#[cfg(test)]
mod sequence_policy_setup_tests {
    use super::*;

    /// All `from_env` cases read the same env vars, so they share one lock
    /// to stay deterministic under the parallel test harness.
    static ENV_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

    fn write_policy_file(dir: &std::path::Path, action: &str) -> std::path::PathBuf {
        let rigorix = dir.join(".rigorix");
        std::fs::create_dir_all(&rigorix).expect("create .rigorix");
        let path = rigorix.join("sequence-policy.toml");
        std::fs::write(
            &path,
            format!(
                r#"fail_closed = true

[[rules]]
id = "no-remove-reassign"
name = "No remove-then-reassign"
description = "deny remove-then-reassign of a full seat"
steps = [
  {{ tool = "registration_remove", params = [{{ pointer = "/event_id", kind = "exact", value = "conf-2026" }}] }},
  {{ tool = "registration_add",    params = [{{ pointer = "/event_id", kind = "exact", value = "conf-2026" }}] }},
]
window = 3
action = "{action}"
"#
            ),
        )
        .expect("write rule file");
        path
    }

    /// ADR-013 wiring: `from_env` arms the R3 gate even when the rule file is
    /// absent — the per-run repository read keeps that case fail-open-absent
    /// (status quo), so arming is safe and picks up a later-created file.
    #[test]
    fn absent_policy_file_still_arms_gate() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let svc = SequencePolicySetup::from_env(dir.path());
        assert!(
            svc.is_some(),
            "absent rule file must still arm the gate (per-run fail-open)"
        );
    }

    #[test]
    fn present_policy_file_arms_gate() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        write_policy_file(dir.path(), "deny");
        let svc = SequencePolicySetup::from_env(dir.path());
        assert!(svc.is_some(), "rule file present → service attached");
    }

    #[test]
    fn disabled_env_returns_none() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        write_policy_file(dir.path(), "deny");
        unsafe { std::env::set_var("RIGORIX_SEQUENCE_POLICY", "0") };
        let svc = SequencePolicySetup::from_env(dir.path());
        unsafe { std::env::remove_var("RIGORIX_SEQUENCE_POLICY") };
        assert!(
            svc.is_none(),
            "RIGORIX_SEQUENCE_POLICY=0 must disable the gate"
        );
    }

    #[test]
    fn path_override_points_at_explicit_file() {
        let _guard = ENV_LOCK.lock().unwrap();
        let dir = tempfile::tempdir().expect("tempdir");
        let custom = dir.path().join("policies").join("seq.toml");
        std::fs::create_dir_all(custom.parent().unwrap()).expect("parent dir");
        std::fs::write(&custom, b"fail_closed = true\n").expect("write custom");
        unsafe { std::env::set_var("RIGORIX_SEQUENCE_POLICY_PATH", &custom) };
        let svc = SequencePolicySetup::from_env(dir.path());
        unsafe { std::env::remove_var("RIGORIX_SEQUENCE_POLICY_PATH") };
        assert!(
            svc.is_some(),
            "path override must point the repository at the explicit file"
        );
    }
}
