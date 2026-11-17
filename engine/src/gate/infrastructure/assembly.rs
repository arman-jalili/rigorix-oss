//! The governed assembly — the one place an orchestrator is built out of operator configuration.
//!
//! @canonical .pi/architecture/decisions/ADR-025-one-governed-assembly.md
//! @canonical .pi/architecture/decisions/ADR-017-consequence-gating.md
//! Issue: #1625 (ADR-025 D1/D5)
//!
//! # Why this exists
//!
//! The engine had one machine and four assemblies of it. `mcp::host::build_real_engine` wired nine
//! seams by hand, `actions::main` eight, the CLI used [`GateSetup`] and wired the rest, and
//! `receipt::api::run_receipt` — written *after* `GateSetup` existed, and the only one of the four
//! that looks like an API — consulted no precondition gate at all. A host that found it obtained an
//! engine that could not tell "no findings" from "no gate", because the two are the same output.
//!
//! [`GateSetup`] recorded the reason it took half the problem on in #1381: "moving the *order* into
//! the engine while leaving the *construction* in each caller would only move half the problem … the
//! next runtime would have to rediscover it." This module is the other half.
//!
//! # Shape: a function, not a builder
//!
//! [`assemble`] is one `async fn` returning a [`GovernedAssembly`]. `run_receipt` rejected builders
//! for a reason that applies here twice over: a builder invites a caller to assemble an engine
//! missing its audit service and discover it at the first refusal, and a builder with a `with_gate`
//! method invites the same for the gate. The config struct below is **data**, not a chain: what a
//! host brings is written down once, and what it does not bring is absent on purpose.
//!
//! # What a host may not delegate
//!
//! Four values, and none of them has a `Default`:
//!
//! - `workspace` — where `.rigorix/*` is read from, and the value a receipt's workspace identity is
//!   checked against. A wrong root arms the wrong operator files, which looks exactly like an engine
//!   whose gates found nothing.
//! - `state_dir` — the seal. A directory *inside* the workspace lets the executing agent rewrite
//!   its own attestation (ADR-024 §6 rule 1), so the assembly refuses it on resolved paths.
//! - `audit_key` — `None` is an unsigned run. The audit envelope refuses to sign, and the attestation
//!   store refuses to write, rather than producing evidence nobody can verify.
//! - `permission_mode` — an enforcer that was never built is a skipped gate (#1487), not a lenient
//!   one, so the mode is an argument and a failure to build it is an error, never a warning.
//!
//! # What the assembly attaches, in this order
//!
//! precondition gate → sequence policy → capacity ledger → effect commitment → approval binding →
//! permission enforcer → sandbox declaration → recovery recipes → the dispatch service that consumes
//! them → the seal and the audit service. The order is not a preference: a gate attached after the
//! executor is built is not consulted, which is the #1381 finding one layer down.
//!
//! Every seam whose arming is decided by an operator file is read here and nowhere else, so the
//! loading semantics are stated once: absent file → no seam (fail open, ADR-017), malformed file →
//! an unarmed gate that refuses everything (fail closed — governance that cannot be established must
//! not vanish).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::audit::application::service::AuditService;
use crate::budget_tracking::application::llm_budget_impl::LlmBudgetImpl;
use crate::budget_tracking::application::service::LlmBudgetService;
use crate::cancellation::application::cancellation_service_impl::CancellationManagerImpl;
use crate::cancellation::application::service::CancellationService;
use crate::capacity_ledger::CapacitySetup;
use crate::code_graph::application::service::CodeGraphService;
use crate::effect_commitment::EffectCommitmentSetup;
use crate::event_system::application::dto::EventBusConfig;
use crate::event_system::application::event_bus_service_impl::EventBusServiceImpl;
use crate::event_system::application::service::EventBusService;
use crate::execution_engine::application::factory::{
    ApprovalBindingSetup, ParallelExecutionFactory, ParallelExecutionFactoryConfig, RecoverySetup,
};
use crate::execution_engine::application::factory_impl::ParallelExecutionFactoryImpl;
use crate::execution_engine::application::service::ParallelExecutionService;
use crate::execution_engine::domain::ParallelExecutorConfig;
use crate::gate::infrastructure::GateSetup;
use crate::orchestrator::application::orchestrator_impl::OrchestratorServiceImpl;
use crate::orchestrator::application::service::OrchestratorService;
use crate::orchestrator::domain::OrchestratorConfig;
use crate::permission::application::enforcer::PermissionEnforcer;
use crate::permission::application::enforcer_factory_impl::PermissionEnforcerFactoryImpl;
use crate::permission::application::factory::PermissionEnforcerFactory;
use crate::permission::domain::PermissionMode;
use crate::planning::application::service::PlanningPipelineService;
use crate::policy_engine::application::engine::PolicyEngineService;
use crate::quality_gates::application::service::QualityGateService;
use crate::receipt::application::service::default_state_dir;
use crate::receipt::infrastructure::attestation_store::FileAttestationStore;
use crate::receipt::infrastructure::receipt_store::FileReceiptStore;
use crate::scored_evaluation::application::ScoredEvaluationService;
use crate::state_persistence::application::service::StateManagerService;
use crate::state_persistence::application::state_manager_service_impl::FileSystemStateManager;
use crate::state_persistence::infrastructure::filesystem_state_repository::FileSystemStateRepository;
use crate::tools::application::shell_policy::{SandboxDeclaration, declaration_for_host};

/// What the LLM budget for this run is.
///
/// Two states rather than an `Option<(u32, u32)>`, because "unlimited" and "1000 calls" are the same
/// kind of question and a caller should not have to spell `Some(u32::MAX)` to say the first.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LlmBudgetLimits {
    /// No ceiling: the budget service still counts, so the run's record carries the counts.
    Unlimited,
    /// A ceiling, in calls and tokens.
    Exact {
        /// Maximum LLM calls.
        max_calls: u32,
        /// Maximum tokens.
        max_tokens: u32,
    },
}

/// The dispatch policy an executor is built with — the knobs a host legitimately varies.
///
/// Nothing here decides *governance*: every seam is armed by an operator file and attached by the
/// assembly. These are concurrency, retry, and which optional machinery the host asks for.
#[derive(Debug, Clone)]
pub struct ExecutorPolicy {
    /// Maximum concurrently running steps.
    pub max_parallel_tasks: usize,
    /// Retries per step when a retry policy is not declared by the step.
    pub max_retries: u32,
    /// Abort the run after this many step failures; `0` means no limit.
    pub max_failures_before_abort: u32,
    /// The enforcement preset applied before each tool call (ADR-008).
    pub enforcement: Option<crate::enforcement::domain::config::EnforcementConfig>,
    /// Whether `.rigorix/hooks.toml` is loaded. A host that opted out says so here.
    pub hooks: bool,
    /// Whether recovery recipes are active. `RIGORIX_RECOVERY=1` is the operator's switch; a host
    /// that passes `true` still gets recipes only when the environment asks for them.
    pub recovery: bool,
}

impl Default for ExecutorPolicy {
    fn default() -> Self {
        Self {
            max_parallel_tasks: 4,
            max_retries: 3,
            max_failures_before_abort: 0,
            enforcement: Some(crate::enforcement::domain::config::EnforcementConfig::standard()),
            hooks: true,
            recovery: true,
        }
    }
}

/// What a host brings to the assembly, and what it may not delegate.
#[derive(Clone)]
pub struct GovernedAssemblyConfig {
    /// The repository root. Operator files are read from `<workspace>/.rigorix/`.
    pub workspace: PathBuf,
    /// Where the seal lives. Empty means "the default for this workspace", computed the same way
    /// `run_receipt` computes it; a non-empty value is used as given and refused if it resolves
    /// inside the workspace.
    pub state_dir: PathBuf,
    /// The attestation and audit signing key. `None` is an unsigned run.
    pub audit_key: Option<String>,
    /// The permission mode the enforcer is built for.
    pub permission_mode: PermissionMode,
    /// The host's name, recorded in the sandbox declaration and in log lines.
    pub host: &'static str,
    /// The orchestrator's own configuration (timeouts, buffer sizes, whether audit is enabled).
    pub orchestrator: OrchestratorConfig,
    /// Dispatch policy.
    pub executor: ExecutorPolicy,
    /// The LLM budget for the run.
    pub budget: LlmBudgetLimits,
    /// The planning pipeline. A receipt-driven host passes
    /// [`crate::planning::application::supplied_steps::SuppliedStepsPlanning`], which refuses every
    /// planning method with its reason: the steps were supplied, so planning was not asked for.
    pub planning: Arc<dyn PlanningPipelineService>,
    /// The plan→execute→verify loop, when the host wired one.
    ///
    /// `None` is the status quo for every host that runs a plan once; a host that supplies it is
    /// asking the orchestrator to iterate, and the *loop* is the host's to configure.
    pub validation_loop:
        Option<Arc<dyn crate::plan_validation::application::service::ValidationLoopService>>,
    /// The audit service the host built, when it built one.
    pub audit_service: Option<Arc<dyn AuditService>>,
    /// Quality-gate service, when the host wired one.
    pub quality_gate_service: Option<Arc<dyn QualityGateService>>,
    /// Scored-evaluation service, when the host wired one.
    pub scored_evaluation_service: Option<Arc<dyn ScoredEvaluationService>>,
    /// Policy engine, when the host wired one.
    pub policy_engine: Option<Arc<dyn PolicyEngineService>>,
    /// Code-graph service, when the host wired one.
    pub code_graph_service: Option<Arc<dyn CodeGraphService>>,
    /// Whether to wire the seal: the attestation store, the receipt store and the sealed state
    /// directory the orchestrator records into. A host that only plans does not need it.
    pub seal: bool,
}

impl std::fmt::Debug for GovernedAssemblyConfig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GovernedAssemblyConfig")
            .field("workspace", &self.workspace)
            .field("state_dir", &self.state_dir)
            .field("audit_key", &self.audit_key.as_ref().map(|_| "<redacted>"))
            .field("permission_mode", &self.permission_mode)
            .field("host", &self.host)
            .field("seal", &self.seal)
            .finish_non_exhaustive()
    }
}

/// Why an assembly could not be built.
#[derive(Debug)]
pub enum AssemblyError {
    /// The state directory resolved inside the workspace, so the seal could be rewritten by the run
    /// it is supposed to describe.
    SealInsideWorkspace {
        /// The state directory as resolved.
        state_dir: PathBuf,
        /// The repository root it was compared against.
        workspace: PathBuf,
    },
    /// The permission enforcer could not be built. This is an error rather than a warning because
    /// an engine with no enforcer dispatches everything it is asked to (#1487).
    PermissionEnforcer {
        /// The factory's own words.
        detail: String,
    },
    /// An operator file is malformed and the seam it would arm refuses to disappear. `seam` names
    /// the file, so an operator knows which one to fix.
    OperatorConfiguration {
        /// Which seam: `capacity`, `effect-commitment`, `approval`, `hooks`, or `state`.
        seam: &'static str,
        /// The parser's own words.
        detail: String,
    },
}

impl std::fmt::Display for AssemblyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SealInsideWorkspace {
                state_dir,
                workspace,
            } => write!(
                f,
                "the state directory {} is inside the workspace {} — the run could rewrite the \
                 record that describes it (ADR-024 §6)",
                state_dir.display(),
                workspace.display()
            ),
            Self::PermissionEnforcer { detail } => {
                write!(f, "the permission enforcer could not be built: {detail}")
            }
            Self::OperatorConfiguration { seam, detail } => {
                write!(f, "the {seam} configuration is invalid: {detail}")
            }
        }
    }
}

impl std::error::Error for AssemblyError {}

/// The services one assembly produced. Every field is something a host needs and none is a seam a
/// host may re-wire: the orchestrator already contains the gate, and the stores answer for the seal.
#[derive(Clone)]
pub struct GovernedAssembly {
    /// The orchestrator, with every seam attached.
    pub orchestrator: Arc<dyn OrchestratorService>,
    /// The bus the executor publishes into and the orchestrator drains — one bus, because a split
    /// one produces envelopes with no evidence in them.
    pub event_bus: Arc<dyn EventBusService>,
    /// The dispatch service, for a host that needs to ask it something directly.
    pub execution: Arc<dyn ParallelExecutionService>,
    /// The state manager the orchestrator persists through. Returned because a host's own services
    /// (a CLI's resumable sessions, say) read the same durable state, and a second manager over the
    /// same directory would be a second writer.
    pub state_manager: Arc<dyn StateManagerService>,
    /// What the operator's files armed, for a host that reports its posture.
    pub gate: GateSetup,
    /// Where the seal lives, as resolved by this assembly.
    pub state_dir: PathBuf,
    /// The attestation store, when `seal` was set.
    pub attestation_store: Option<FileAttestationStore>,
    /// The receipt store, when `seal` was set.
    pub receipt_store: Option<FileReceiptStore>,
}

impl std::fmt::Debug for GovernedAssembly {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GovernedAssembly")
            .field("state_dir", &self.state_dir)
            .field("gate_empty", &self.gate.is_empty())
            .field("sealed", &self.attestation_store.is_some())
            .finish_non_exhaustive()
    }
}

/// Read `.rigorix/hooks.toml` and build the hook runner, when the host asked for hooks.
///
/// The two hosts that had this each carried their own byte-identical copy. A hook config that cannot
/// be parsed is a warning and no hooks — the same semantics `SequencePolicySetup` and
/// `PreconditionSetup` use for an *absent* file, and different from a malformed preconditions file,
/// which arms a refusing gate. The difference is deliberate: a hook is an interceptor a host can run
/// without, while a gate is a governance decision that must not vanish. That asymmetry is stated
/// here because it is the kind of thing a reader assumes is uniform.
fn hook_runner(
    workspace: &Path,
) -> Result<Option<Arc<dyn crate::hooks::application::service::HookRunnerService>>, AssemblyError> {
    use crate::hooks::application::factory::HookRunnerFactory;
    use crate::hooks::application::runner_factory_impl::HookRunnerFactoryImpl;
    use crate::hooks::domain::config::HookConfig;

    let path = workspace.join(".rigorix/hooks.toml");
    let Ok(content) = std::fs::read_to_string(&path) else {
        return Ok(None);
    };
    let config: HookConfig =
        toml::from_str(&content).map_err(|error| AssemblyError::OperatorConfiguration {
            seam: "hooks",
            detail: format!("{}: {error}", path.display()),
        })?;
    match HookRunnerFactoryImpl.create(config) {
        Ok(runner) => {
            tracing::info!("hooks enabled from {}", path.display());
            Ok(Some(Arc::from(runner)))
        }
        Err(error) => Err(AssemblyError::OperatorConfiguration {
            seam: "hooks",
            detail: format!("{}: {error}", path.display()),
        }),
    }
}

/// Where the seal lives for `workspace`, from what the caller asked for.
///
/// One rule with two callers: [`assemble`] resolves the directory it will seal into, and a host that
/// needs the directory *before* it calls the assembly (to place a local audit trail, say) resolves it
/// with the same function. A host that recomputed the default itself would be a second answer to
/// "where is the seal", which is the shape #1628 removed one layer down.
#[must_use]
pub fn resolve_state_dir(workspace: impl AsRef<Path>, requested: &Path) -> PathBuf {
    if requested.as_os_str().is_empty() {
        default_state_dir(&workspace.as_ref().to_string_lossy())
    } else {
        requested.to_path_buf()
    }
}

/// Build the governed engine: every seam, in the order that makes them consulted, from the
/// operator's own files.
///
/// # Errors
/// [`AssemblyError`] when the seal would live inside the workspace, when the permission enforcer
/// cannot be built, or when an operator file is malformed in a way that must not be downgraded to
/// "no seam".
#[allow(clippy::too_many_lines)] // one seam per line group; splitting it would hide the order
pub async fn assemble(config: GovernedAssemblyConfig) -> Result<GovernedAssembly, AssemblyError> {
    let workspace = config.workspace.clone();
    // The default layout, computed by the module that owns it, so a host that does not choose a
    // state directory still gets the one every other part of the engine will look in.
    let state_dir = resolve_state_dir(&workspace, &config.state_dir);

    // ── The seams, in the order they are consulted ────────────────────────────
    // ADR-013 R3 + ADR-017 R1/R2: one composition root for both operator gates. Absent file → no
    // gate; malformed precondition file → an unarmed gate that refuses everything.
    let gate = GateSetup::from_env(&workspace);
    // ADR-018: operator-declared capacity. Absent → no seam; malformed → refuse the run.
    let capacity = CapacitySetup::from_env(&workspace).await.map_err(|error| {
        AssemblyError::OperatorConfiguration {
            seam: "capacity",
            detail: error.to_string(),
        }
    })?;
    // ADR-019: effect commitment. Absent → no seam; malformed → refuse the run. A record left
    // `Dispatched` by a crash has no live owner, so it is marked ambiguous before anything retries.
    let effect_commitment =
        match EffectCommitmentSetup::from_env(&workspace)
            .await
            .map_err(|error| AssemblyError::OperatorConfiguration {
                seam: "effect-commitment",
                detail: error.to_string(),
            })? {
            Some(setup) => {
                setup
                    .reconcile()
                    .await
                    .map_err(|error| AssemblyError::OperatorConfiguration {
                        seam: "effect-commitment",
                        detail: format!("reconciliation: {error}"),
                    })?;
                Some(setup.dispatch_seam())
            }
            None => None,
        };
    // ADR-011: the approval store, signed with the same key the evidence is signed with.
    let approval_binding =
        ApprovalBindingSetup::from_env_with_signing_key(&workspace, config.audit_key.as_deref())
            .map_err(|error| AssemblyError::OperatorConfiguration {
                seam: "approval",
                detail: error.to_string(),
            })?;
    // #1487: an enforcer that cannot be built is an error. The previous shape warned and continued
    // with `None`, which the dispatcher read as "skip the gate".
    let permission_enforcer: Option<Arc<dyn PermissionEnforcer>> = Some(Arc::from(
        PermissionEnforcerFactoryImpl
            .create_with_mode(config.permission_mode)
            .await
            .map_err(|error| AssemblyError::PermissionEnforcer {
                detail: error.to_string(),
            })?,
    ));
    // #1487: the isolation every spawned step runs under. Absent ⇒ the run still starts and each
    // `run_command` step is refused with `sandbox_undeclared`, which is evidence rather than a shell.
    let sandbox: Option<Arc<SandboxDeclaration>> = declaration_for_host(config.host);
    let hooks = if config.executor.hooks {
        hook_runner(&workspace)?
    } else {
        None
    };
    // Recovery is opt-in twice: the host says whether it wants the capability, and the operator's
    // `RIGORIX_RECOVERY` says whether recipes are active. `false` is "this host does not arm it",
    // never "recovery is off everywhere".
    let recovery = if config.executor.recovery {
        RecoverySetup::from_env()
    } else {
        None
    };

    // ── The services those seams feed ─────────────────────────────────────────
    let event_bus: Arc<dyn EventBusService> =
        Arc::new(EventBusServiceImpl::new(EventBusConfig::default()));
    let executor_policy = &config.executor;
    let exec_config = ParallelExecutionFactoryConfig {
        executor_config: ParallelExecutorConfig {
            max_concurrent_executions: executor_policy.max_parallel_tasks as u32,
            max_total_retries_per_session: executor_policy.max_retries,
            max_failures_before_abort: executor_policy.max_failures_before_abort,
            // ADR-011 R5: the effect-scope oracle needs a repository to diff. It stays inert unless
            // the approved intent declared a scope, so this wiring cannot invent a violation (#1099).
            approval_repo_path: Some(workspace.to_string_lossy().to_string()),
            enforcement: executor_policy.enforcement.clone(),
            ..ParallelExecutorConfig::default()
        },
        event_bus: Some(Arc::clone(&event_bus)),
        permission_enforcer,
        sandbox: sandbox.clone(),
        hook_runner: hooks,
        approval_binding,
        capacity,
        effect_commitment,
        recovery,
        // Both gate ports come from the one setup, so a caller cannot wire one and forget the other.
        ..gate.execution_config(ParallelExecutionFactoryConfig::default())
    };
    let execution: Arc<dyn ParallelExecutionService> = Arc::from(
        ParallelExecutionFactoryImpl
            .create(exec_config)
            .await
            .map_err(|error| AssemblyError::OperatorConfiguration {
                seam: "dispatch",
                detail: error.to_string(),
            })?,
    );

    let state_repository = FileSystemStateRepository::new(state_dir.clone())
        .await
        .map_err(|detail| AssemblyError::OperatorConfiguration {
            seam: "state",
            detail: format!("{}: {detail}", state_dir.display()),
        })?;
    let state_manager: Arc<dyn StateManagerService> =
        Arc::new(FileSystemStateManager::new(Box::new(state_repository)));
    let state_manager_for_orchestrator = Arc::clone(&state_manager);
    let cancellation: Arc<dyn CancellationService> = Arc::new(CancellationManagerImpl::default());
    let (max_calls, max_tokens) = match config.budget {
        LlmBudgetLimits::Unlimited => (u32::MAX, u32::MAX),
        LlmBudgetLimits::Exact {
            max_calls,
            max_tokens,
        } => (max_calls, max_tokens),
    };
    let budget: Arc<dyn LlmBudgetService> = Arc::new(LlmBudgetImpl::new(
        max_calls,
        max_tokens,
        config.host.to_string(),
    ));

    // ── The orchestrator, and the two halves of the seal ──────────────────────
    let mut orchestrator = OrchestratorServiceImpl::governed(
        crate::gate::domain::Governed::witness(),
        config.orchestrator.clone(),
        Arc::clone(&config.planning),
        Arc::clone(&execution),
        state_manager_for_orchestrator,
        cancellation,
        Arc::clone(&event_bus),
        config.audit_service.clone(),
        budget,
        config.code_graph_service.clone(),
    )
    // The builder-side seams: plan-time sequence policy (R2), the extras, and the seal.
    .with_attestation_key(config.audit_key.clone());
    if let Some(policy) = gate.sequence.clone() {
        orchestrator = orchestrator.with_sequence_policy(policy);
    }
    if let Some(service) = config.validation_loop.clone() {
        orchestrator = orchestrator.with_validation_loop(service);
    }
    if let Some(service) = config.quality_gate_service.clone() {
        orchestrator = orchestrator.with_quality_gate_service(service);
    }
    if let Some(service) = config.scored_evaluation_service.clone() {
        orchestrator = orchestrator.with_scored_evaluation_service(service);
    }
    if let Some(engine) = config.policy_engine.clone() {
        orchestrator = orchestrator.with_policy_engine(engine);
    }
    if let Some(declaration) = sandbox.clone() {
        orchestrator = orchestrator.with_sandbox_declaration(declaration);
    }
    let attestation_store = if config.seal {
        // The orchestrator records *where* the seal goes; the store refuses the same directory the
        // orchestrator was about to write into, which is why both are constructed from one value.
        orchestrator = orchestrator.with_sealed_state_dir(state_dir.clone());
        Some(FileAttestationStore::new(
            state_dir.clone(),
            workspace.clone(),
            config.audit_key.clone(),
        ))
    } else {
        None
    };
    let receipt_store = config
        .seal
        .then(|| FileReceiptStore::new(state_dir.clone(), workspace.clone()));

    Ok(GovernedAssembly {
        orchestrator: Arc::new(orchestrator),
        event_bus,
        execution,
        state_manager,
        gate,
        state_dir,
        attestation_store,
        receipt_store,
    })
}
