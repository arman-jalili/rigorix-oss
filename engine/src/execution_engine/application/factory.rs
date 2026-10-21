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

    /// ADR-017: optional dispatch-time precondition gate + R2 gating mode.
    ///
    /// When set, the factory attaches the armed gate and gating mode to the
    /// executor; the composition roots build it from the operator config via
    /// [`crate::precondition::PreconditionSetup::from_env`]. `None` (default)
    /// preserves today's behavior (no gate — status quo).
    pub precondition: Option<crate::precondition::PreconditionSetup>,

    /// ADR-018: optional capacity-ledger dispatch seam.
    ///
    /// When set, the dispatch loop reserves capacity for every step a declared
    /// rule governs, commits on success and releases on failure (RAII). The
    /// composition roots build it from the operator declaration via
    /// [`crate::capacity_ledger::CapacitySetup::from_env`]. `None` (default)
    /// preserves today's behavior exactly (no seam — status quo).
    pub capacity: Option<crate::capacity_ledger::CapacitySetup>,

    /// ADR-019: optional effect-commitment dispatch seam.
    ///
    /// When set, `run_dispatch_loop` records a write-ahead intent before each
    /// dispatch, refuses the dispatch if the record cannot be written, and
    /// settles or marks the effect ambiguous when the node returns. The
    /// composition roots build it from `.rigorix/effect-commitment.toml` via
    /// `EffectCommitmentSetup`. `None` (default) preserves the status quo.
    pub effect_commitment: Option<std::sync::Arc<dyn crate::effect_commitment::DispatchSeam>>,

    /// Optional recovery-recipe service (ADR-0xx recovery recipes, #1108).
    ///
    /// When set, a node failure is mapped to a failure scenario and one recovery
    /// attempt is made before retry evaluation. `None` (default) keeps today's
    /// behaviour: no recovery attempt.
    ///
    /// The composition roots build it with [`RecoverySetup::from_env`], which is
    /// **opt-in** (`RIGORIX_RECOVERY=1`): recovery inserts an attempt into the
    /// failure path, and changing that by default would change how every failure
    /// is retried. Wiring it opt-in makes the capability reachable and switchable
    /// instead of unreachable (#1108, audit row W14).
    pub recovery: Option<Arc<dyn crate::recovery_recipes::application::service::RecoveryService>>,
}

/// Opt-in arming for the recovery-recipe seam (#1108).
pub struct RecoverySetup;

impl RecoverySetup {
    /// The environment variable that enables recovery recipes.
    pub const ENV: &'static str = "RIGORIX_RECOVERY";

    /// Build the recovery service when the operator asked for it.
    ///
    /// Truthy: `1`, `true`, `yes` (case-insensitive). Anything else — including
    /// unset — leaves recovery off, which is the documented default.
    pub fn from_env()
    -> Option<Arc<dyn crate::recovery_recipes::application::service::RecoveryService>> {
        let requested = std::env::var(Self::ENV)
            .map(|value| matches!(value.to_ascii_lowercase().as_str(), "1" | "true" | "yes"))
            .unwrap_or(false);
        if !requested {
            return None;
        }
        Some(Arc::new(
            crate::recovery_recipes::application::service_impl::RecoveryServiceImpl::new(),
        ))
    }
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
            precondition: None,
            capacity: None,
            effect_commitment: None,
            recovery: None,
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

/// Why an explicitly requested approval binding could not be armed.
///
/// A requested guarantee must not silently disappear (#1070): `=1` without a
/// key is "requested and unsatisfiable" and must fail at composition time, not
/// log a warning and serve unbound. Unset stays quiet (`Ok(None)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApprovalBindingError {
    /// `RIGORIX_APPROVAL_BINDING` requested binding but `RIGORIX_HMAC_KEY` is
    /// unset or empty.
    MissingKey {
        /// The environment variable that is missing.
        var: &'static str,
        /// What silently happened before this became an error.
        consequence: &'static str,
    },
    /// The approval store could not be opened at the configured path.
    StoreUnavailable {
        /// Underlying I/O detail.
        detail: String,
    },
}

impl std::fmt::Display for ApprovalBindingError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::MissingKey { var, consequence } => {
                write!(
                    f,
                    "approval binding requested but {var} is unset: {consequence}"
                )
            }
            Self::StoreUnavailable { detail } => {
                write!(f, "approval store unavailable: {detail}")
            }
        }
    }
}

impl std::error::Error for ApprovalBindingError {}

impl ApprovalBindingSetup {
    /// Build an approval binding setup from environment configuration.
    ///
    /// Enabled by `RIGORIX_APPROVAL_BINDING=1|true`. The run key defaults to
    /// `RIGORIX_HMAC_KEY` (must equal the envelope HMAC key — ADR-011 §key);
    /// TTL via `RIGORIX_APPROVAL_TTL_SECONDS` (default 3600). Records are
    /// persisted to `<repo_root>/.rigorix/approvals.json` (atomic writes,
    /// cross-process resume).
    ///
    /// # Errors
    ///
    /// - `Ok(None)` — binding was not requested.
    /// - `Err(MissingKey)` — requested via `RIGORIX_APPROVAL_BINDING=1` but
    ///   `RIGORIX_HMAC_KEY` is unset. This used to warn and return `None`, so a
    ///   deployment could believe it was binding approvals for months while
    ///   approving unbound (#1070).
    /// - `Err(StoreUnavailable)` — the approval store could not be opened. This
    ///   used to panic in the composition root.
    pub fn from_env(repo_root: &std::path::Path) -> Result<Option<Self>, ApprovalBindingError> {
        let enabled = std::env::var("RIGORIX_APPROVAL_BINDING").ok();
        let run_key = std::env::var("RIGORIX_HMAC_KEY").ok();
        let ttl = std::env::var("RIGORIX_APPROVAL_TTL_SECONDS").ok();
        Self::from_env_values(
            enabled.as_deref(),
            run_key.as_deref(),
            ttl.as_deref(),
            repo_root,
        )
    }

    /// The environment read is separated from the decision so the decision is
    /// testable without mutating process-global state.
    pub(crate) fn from_env_values(
        enabled: Option<&str>,
        run_key: Option<&str>,
        ttl_seconds: Option<&str>,
        repo_root: &std::path::Path,
    ) -> Result<Option<Self>, ApprovalBindingError> {
        let on = matches!(
            enabled,
            Some("1") | Some("true") | Some("TRUE") | Some("yes")
        );
        if !on {
            return Ok(None);
        }
        let Some(run_key) = run_key
            .filter(|k| !k.is_empty())
            .map(|k| k.as_bytes().to_vec())
        else {
            return Err(ApprovalBindingError::MissingKey {
                var: "RIGORIX_HMAC_KEY",
                consequence: "approvals would be accepted unbound; set the key or unset \
                              RIGORIX_APPROVAL_BINDING",
            });
        };
        let ttl_seconds = ttl_seconds
            .and_then(|v| v.parse::<u64>().ok())
            .unwrap_or(3600);
        let store_dir = repo_root.join(".rigorix");
        let _ = std::fs::create_dir_all(&store_dir);
        let repository = std::sync::Arc::new(
            crate::approval::infrastructure::repository::FileBackedApprovalRepository::open(
                store_dir.join("approvals.json"),
            )
            .map_err(|e| ApprovalBindingError::StoreUnavailable {
                detail: format!("{}: {e}", store_dir.join("approvals.json").display()),
            })?,
        );
        tracing::info!(
            ttl_seconds,
            "approval binding ENABLED (ADR-011) — records at {}",
            store_dir.join("approvals.json").display()
        );
        Ok(Some(Self {
            repository,
            run_key,
            ttl_seconds,
        }))
    }
}

#[cfg(test)]
mod approval_binding_env_tests {
    use super::*;

    #[test]
    fn unset_binding_is_not_requested_and_stays_quiet() {
        assert!(
            ApprovalBindingSetup::from_env_values(None, None, None, std::path::Path::new("/tmp"))
                .expect("not requested is not an error")
                .is_none()
        );
        assert!(
            ApprovalBindingSetup::from_env_values(
                Some("0"),
                None,
                None,
                std::path::Path::new("/tmp")
            )
            .expect("explicitly off is not an error")
            .is_none()
        );
    }

    #[test]
    fn requested_binding_without_a_key_fails_instead_of_downgrading() {
        let result = ApprovalBindingSetup::from_env_values(
            Some("1"),
            None,
            None,
            std::path::Path::new("/tmp"),
        );
        let Err(err) = result else {
            panic!("requested and unsatisfiable must fail, not silently downgrade");
        };
        assert!(matches!(
            err,
            ApprovalBindingError::MissingKey {
                var: "RIGORIX_HMAC_KEY",
                ..
            }
        ));
        assert!(err.to_string().contains("RIGORIX_HMAC_KEY"));

        // An empty key is as unsatisfiable as an absent one.
        assert!(
            ApprovalBindingSetup::from_env_values(
                Some("1"),
                Some(""),
                None,
                std::path::Path::new("/tmp")
            )
            .is_err()
        );
    }

    #[test]
    fn requested_binding_with_a_key_is_armed() {
        let dir = tempfile::TempDir::new().expect("tempdir");
        let setup = ApprovalBindingSetup::from_env_values(
            Some("1"),
            Some("run-key"),
            Some("60"),
            dir.path(),
        )
        .expect("armed")
        .expect("some");
        assert_eq!(setup.ttl_seconds, 60);
        assert_eq!(setup.run_key, b"run-key".to_vec());
        assert!(dir.path().join(".rigorix").is_dir());
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
    /// #889 (OSS-C5): an enterprise-exported `policy.json` v1 bundle is a
    /// first-class source alongside the local TOML
    /// (`RIGORIX_SEQUENCE_POLICY_BUNDLE` / `RIGORIX_SEQUENCE_POLICY_BUNDLE_PATH`).
    /// Enterprise is the source of truth; the local-vs-bundle precedence is
    /// explicit and fail-closed (`RIGORIX_SEQUENCE_POLICY_PRECEDENCE`:
    /// `enterprise` default | `local` | `refuse`). Both sources are parsed and
    /// validated through the SAME `SequencePolicyConfig` validator.
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

        // #889 (OSS-C5): reconcile the local operator TOML with an
        // enterprise-exported policy bundle. Enterprise is the source of
        // truth; precedence is explicit (default: enterprise wins) and every
        // malformed source fails closed via the per-run repository read.
        let local_repo =
            crate::sequence_policy::infrastructure::TomlSequencePolicyRepository::new(path.clone());
        let bundle_repo =
            crate::sequence_policy::infrastructure::BundleSequencePolicyRepository::from_env();
        let bundle_source = if bundle_repo.is_configured() {
            tracing::info!(
                "sequence_policy: enterprise policy bundle source armed — local TOML {} is a \
                 fallback/override per RIGORIX_SEQUENCE_POLICY_PRECEDENCE",
                path.display()
            );
            Some(Box::new(bundle_repo)
                as Box<
                    dyn crate::sequence_policy::infrastructure::SequencePolicyRepository,
                >)
        } else {
            None
        };
        let precedence =
            crate::sequence_policy::infrastructure::SequencePolicyPrecedence::from_env();
        let repository =
            crate::sequence_policy::infrastructure::PrecedenceSequencePolicyRepository::new(
                Some(Box::new(local_repo)),
                bundle_source,
                precedence,
            );
        // GAP-A-29 (#895) / ADR-014: enforce the retention coupling on the
        // real load path. `validate_retention` is reused (never a parallel
        // check) and applied uniformly to whichever source precedence selects
        // — local TOML, enterprise bundle, or both. An effect-keyed rule whose
        // window outlives the configured audit retention refuses evaluation
        // (fail closed) instead of silently degrading once envelopes are
        // pruned. Unset `RIGORIX_AUDIT_RETENTION_SECS` = unlimited = status
        // quo.
        let repository =
            crate::sequence_policy::infrastructure::RetentionCoupledSequencePolicyRepository::from_env(
                Box::new(repository),
            );
        tracing::info!(
            "sequence_policy: R3 prefix gate armed — rules read per-run from {} (absent file = no gating)",
            path.display()
        );
        let mut service =
            crate::sequence_policy::application::service_impl::SequencePolicyServiceImpl::new(
                Box::new(repository),
            );
        // ADR-016 Phase C: prefer the anchor-signed projection when an anchor
        // is configured. `AnchoredHistoryAdapter::is_anchored()` is `true` only
        // because every read is signature-verified; an unreachable anchor or a
        // forged slice is an error, so a consequential (deny-class) run fails
        // closed. No anchor ⇒ the Phase A local adapter, unchanged.
        let anchor = crate::audit::infrastructure::anchor::runtime();
        if anchor.mode() == crate::audit::domain::anchor::AnchorMode::Anchored {
            service = service.with_history(std::sync::Arc::new(
                crate::sequence_policy::infrastructure::AnchoredHistoryAdapter::new(anchor),
            ));
            tracing::info!(
                "sequence_policy: R7 cross-run history armed in ANCHORED mode (ADR-016 Phase C)"
            );
        } else {
            // R7: attach the signed-execution-history port over the same repo's
            // `.rigorix/audit` envelope store — cross-run rules read prior runs'
            // signed evidence. A missing audit dir yields empty history (status
            // quo); the audit dir itself is created by the composition roots when
            // they persist envelopes.
            let mut history = crate::sequence_policy::infrastructure::EnvelopeHistoryAdapter::new(
                std::sync::Arc::new(
                    crate::audit::infrastructure::LocalAuditEnvelopeRepository::new(
                        repo_root.join(".rigorix").join("audit"),
                    ),
                ),
            );
            // ADR-016 verify-on-read (GAP-A-30): when the same HMAC key the audit
            // service signs with is available, each envelope is verified (and the
            // local chain is checked) before cross-run policy trusts it. Absent
            // key ⇒ unverifiable history — deny-class rules then refuse unless the
            // operator records `RIGORIX_HISTORY_POLICY=allow_unanchored`.
            let hmac_key = std::env::var("RIGORIX_HMAC_KEY")
                .ok()
                .filter(|k| !k.is_empty());
            if let Some(key) = hmac_key {
                history = history.with_verifier(std::sync::Arc::new(
                    crate::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl::new(
                        Some(key),
                    ),
                ));
            }
            service = service.with_history(std::sync::Arc::new(history));
            // ADR-016: record/apply the `allow_unanchored` opt-in. The audit service
            // reads the same resolver, so the guard's behavior and the signed
            // envelope field cannot diverge.
            service = service.with_unanchored_history_allowed(
                crate::audit::application::audit_service_impl::resolve_history_policy().is_some(),
            );
            tracing::info!(
                "sequence_policy: R7 cross-run history armed over {}/.rigorix/audit",
                repo_root.display()
            );
        }
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
