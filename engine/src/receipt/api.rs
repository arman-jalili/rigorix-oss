//! The crate's documented **run** surface: submit a receipt, read the attestation (#1619).
//!
//! @canonical .pi/architecture/modules/receipt.md
//! Implements: ADR-024 §5 (the observation), §10 (the crate is the host boundary)
//! Issue: #1619
//!
//! # Why this module exists
//!
//! `rigorix-engine` publishes `Receipt`, `verify`, `Attestation` and `refuse_foreign_execution`, so a
//! consumer can **author** a declaration and **check** one. It could not **run** one: the only worked
//! composition in this workspace is `mcp::host::build_real_engine`, which is a host — four hundred
//! lines of audit queue, hook runner, LLM classifier, precondition and sequence-policy setup — and the
//! orchestrator's own constructor asks for eight collaborators. A host that links the crate therefore
//! wrote R0 by hand, verified it, and had no call to make. The harness is the first such host.
//!
//! [`run_receipt`] is that call. It is a **function, not a builder**, deliberately: a builder invites a
//! caller to assemble an engine missing its audit service and discover it at the first refusal, which
//! is the failure mode the receipt's "verify before the graph" rule exists to prevent, one layer up.
//! The three things a host must decide are constructor arguments with no defaults —
//!
//! - `state_dir`, because a state directory inside the workspace lets the agent rewrite the seal
//!   (ADR-024 §6), and a default would make that decision silently;
//! - `audit_key`, because `None` is an **unsigned trail** and the output says so rather than implying
//!   a signature nobody produced;
//! - `permission_mode`, because an enforcer that was never built is a skipped gate, not a lenient one
//!   (#1487).
//!
//! Everything else is the engine's: the event bus, the cancellation manager, the filesystem state
//! manager, the parallel execution service, the budget service, the audit service, the attestation
//! store. A host that wants hooks or sequence policies composes the orchestrator itself; a host that
//! wants to submit a receipt does not have to.
//!
//! # Example
//!
//! ```no_run
//! # async fn run() -> Result<(), Box<dyn std::error::Error>> {
//! use std::path::Path;
//! use rigorix_engine::orchestrator::dto::TemplateStepDef;
//! use rigorix_engine::permission::domain::PermissionMode;
//! use rigorix_engine::receipt::{RunReceiptConfig, Receipt, run_receipt};
//!
//! # let receipt: Receipt = unimplemented!();
//! # let steps: Vec<TemplateStepDef> = Vec::new();
//! let output = run_receipt(
//!     Path::new("/path/to/workspace"),
//!     &receipt,
//!     steps,
//!     RunReceiptConfig {
//!         state_dir: Path::new("/path/outside/the/workspace/.rigorix/state").to_path_buf(),
//!         audit_key: std::env::var("RIGORIX_HMAC_KEY").ok(),
//!         permission_mode: PermissionMode::WorkspaceWrite,
//!     },
//! )
//! .await?;
//!
//! // The observation is the receipt's answer: same execution, same declaration digest.
//! assert_eq!(output.receipt_digest, output.attestation.receipt_digest());
//! # Ok(())
//! # }
//! ```
//!
//! A refusal is the receipt's own and nothing ran ([`ReceiptRefusal`]); a store or execution failure is
//! [`RunReceiptError::Infrastructure`], because a state directory that cannot be written is not a
//! refusal of the receipt — the same distinction the store makes internally.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use crate::orchestrator::dto::RunFromTemplateInput;
use crate::receipt::PlanStep;
use crate::receipt::application::service::{
    refuse_foreign_execution, verify, verify_against_plan, workspace_identity,
};
use crate::receipt::domain::attestation::Attestation;
use crate::receipt::domain::receipt::Receipt;
use crate::receipt::domain::refusal::ReceiptRefusal;

/// What a host must decide before it submits a receipt, and nothing else.
///
/// No field has a default. Each of the three is a decision this crate may not make for a caller: where
/// the seal lives, whether the record is signed, and which permission mode the enforcer runs in.
#[derive(Debug, Clone)]
pub struct RunReceiptConfig {
    /// Where the run's state, the sealed attestation and the local audit trail are written.
    ///
    /// A directory **inside the workspace** is refused (`StateDirInsideWorkspace`) on the resolved
    /// path, because the agent can rewrite it (ADR-024 §6). Use
    /// [`default_state_dir`] for the sibling layout this workspace uses.
    pub state_dir: PathBuf,

    /// The HMAC-SHA256 key the attestation and the audit envelope are signed with.
    ///
    /// `None` means an **unsigned** run: the attestation store refuses to write (`Unsigned`), so a
    /// receipt-gated execution with no key fails loudly instead of producing a record nobody can
    /// verify. The caller that passes `None` gets that refusal, not an unsealed "success".
    pub audit_key: Option<String>,

    /// The permission mode every spawned step is checked against.
    ///
    /// A required argument because an absent enforcer is a skipped gate (#1487), and a default mode
    /// would be this crate choosing a security posture on a host's behalf.
    pub permission_mode: crate::permission::domain::PermissionMode,
}

/// What a submitted receipt produced.
#[derive(Debug, Clone)]
pub struct RunReceiptOutput {
    /// The observation, read back from the sealed file through the module's own verifier.
    pub attestation: Attestation,

    /// The attestation's digest, as the audit envelope references it.
    pub attestation_digest: String,

    /// The declaration's digest, which equals [`Attestation::receipt_digest`] for an accepted run.
    pub receipt_digest: String,

    /// The execution this run carried out.
    pub execution_id: String,
}

/// Why a submitted receipt did not produce an attestation.
///
/// The first variant is the receipt's own answer and nothing ran; the rest are the engine's, and they
/// are kept apart because a store that cannot write is not a declaration that was refused (the
/// distinction `FileAttestationStore` already makes internally).
#[derive(Debug)]
pub enum RunReceiptError {
    /// The declaration does not match the workspace, its plan, the execution, or the schema.
    ///
    /// Every refusal, in the order [`run_receipt`] checked them, so a caller matches on the variants
    /// rather than parsing a sentence. Nothing was dispatched: these are the same three checks the
    /// orchestrator runs before it builds a graph, run here first so the typed values reach the caller.
    Refused(Vec<ReceiptRefusal>),

    /// A step the declaration names was refused **before dispatch** — the consequence gate said no,
    /// so the run did not do what the receipt described. Distinct from `Refused`: the declaration was
    /// sound and was admitted; this is the engine declining to perform it.
    ///
    /// `code` is the dispatcher's own (`precondition_denied`, `sandbox_undeclared`), carried rather
    /// than re-spelled, because a caller branches on the code and a sentence is not a code.
    RefusedAtGate {
        /// The step that was refused.
        step: String,
        /// The dispatcher's failure code for this refusal.
        code: String,
        /// The gate's own sentence, for a human.
        detail: String,
    },

    /// The run happened, and the engine could not write or read back the observation.
    Infrastructure {
        /// The stage that failed, and the engine's own words.
        detail: String,
    },
}

impl std::fmt::Display for RunReceiptError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(refusals) => write!(
                f,
                "the receipt was refused: {}",
                refusals
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
                    .join("; ")
            ),
            Self::RefusedAtGate { step, code, detail } => write!(
                f,
                "the step {step} was refused before dispatch ({code}): {detail}"
            ),
            Self::Infrastructure { detail } => write!(f, "the run could not be recorded: {detail}"),
        }
    }
}

impl std::error::Error for RunReceiptError {}

/// The local, signed audit trail this surface can keep without a backend.
///
/// A host that has an audit backend wires its own service and passes it to the assembly; this door has
/// none, so it records the trail under the state directory and claims no delivery
/// (`audit_enabled: false` at the sink) rather than saying `true` about a sender nobody runs.
fn build_local_audit_service(
    state_dir: &Path,
    audit_key: Option<&String>,
) -> Option<Arc<dyn crate::audit::application::service::AuditService>> {
    let key = audit_key?;
    let mut service = crate::audit::application::audit_service_impl::AuditServiceImpl::new(
        Box::new(
            crate::audit::application::envelope_factory_impl::AuditEnvelopeFactoryImpl::new(Some(
                key.clone(),
            )),
        ),
        Arc::new(crate::audit::application::audit_sender_impl::AuditSenderImpl::new(None, None)),
        Box::new(crate::audit::application::audit_queue_impl::AuditQueueImpl::default()),
        // No backend URL: the trail is local until a host wires a sender. `true` here would claim
        // delivery nobody performs.
        false,
    );
    // The trail goes under the state directory, not the workspace: the same rule the attestation
    // follows, for the same reason (ADR-024 §6 rule 1).
    // sink: receipt:audit-trail-dir
    if std::fs::create_dir_all(state_dir.join("audit")).is_ok() {
        service = service.with_local_repository(Arc::new(
            crate::audit::infrastructure::LocalAuditEnvelopeRepository::new(
                state_dir.join("audit"),
            ),
        ));
    }
    Some(Arc::new(service))
}

/// Submit a receipt against `workspace` and return the sealed observation.
///
/// The verification, the graph, the identity gate, the seal and the settlement are the orchestrator's;
/// this function supplies the composition and nothing else. It does not plan: `steps` are what the
/// receipt declares and what the caller resolved, and the planning pipeline it wires
/// ([`crate::planning::application::SuppliedStepsPlanning`]) refuses every planning method with the
/// reason, so a run that reaches planning fails with a sentence rather than a fabricated plan.
///
/// # Errors
/// [`RunReceiptError::Refused`] when `verify` refuses the declaration — before the graph, before any
/// state write, before anything is dispatched. [`RunReceiptError::Infrastructure`] when the run could
/// not be recorded: no signing key, a state directory inside the workspace, a write failure, or an
/// attestation that does not read back.
pub async fn run_receipt(
    workspace: &Path,
    receipt: &Receipt,
    steps: Vec<crate::orchestrator::dto::TemplateStepDef>,
    config: RunReceiptConfig,
) -> Result<RunReceiptOutput, RunReceiptError> {
    let repo_root = workspace.to_string_lossy().to_string();
    let execution_id = receipt.execution_id;

    // ── The three checks, before anything is built ──
    // The orchestrator runs these before the graph (#1527, ADR-024 §4) and returns `kinds`/`detail`;
    // running them here first means the caller receives the *values* — `ReceiptRefusal` is what this
    // crate publishes, and a caller that matches on a reconstructed message would be matching on
    // prose. The orchestrator verifies again, which is idempotent and costs no dispatch.
    let mut refusals = Vec::new();
    if let Err(refusal) = refuse_foreign_execution(receipt, execution_id) {
        refusals.push(refusal);
    }
    match workspace_identity(&repo_root) {
        Ok(identity) => {
            if let Err(more) = verify(receipt, &identity) {
                refusals.extend(more);
            }
        }
        Err(refusal) => refusals.push(refusal),
    }
    let plan: Vec<PlanStep> = steps
        .iter()
        .map(|step| PlanStep {
            name: step.name.clone(),
            requires_approval: step.requires_approval,
        })
        .collect();
    if let Err(more) = verify_against_plan(receipt, &plan) {
        refusals.extend(more);
    }
    if !refusals.is_empty() {
        return Err(RunReceiptError::Refused(refusals));
    }
    // ── The governed assembly (ADR-025 D1) ──
    // Every seam, attached in one function in the engine, from the operator's own files. Before
    // #1625 this function built its own engine and attached no consequence gate at all, so a run
    // under a denying `.rigorix/preconditions.toml` dispatched anyway and the output looked exactly
    // like a run whose gate found nothing.
    let infrastructure = |detail: String| RunReceiptError::Infrastructure { detail };
    let state_dir =
        crate::gate::infrastructure::assembly::resolve_state_dir(&repo_root, &config.state_dir);
    let audit_service = build_local_audit_service(&state_dir, config.audit_key.as_ref());
    let planning: Arc<dyn crate::planning::application::service::PlanningPipelineService> =
        Arc::new(
            crate::planning::application::supplied_steps::SuppliedStepsPlanning::new(execution_id),
        );
    let assembly = crate::gate::infrastructure::assembly::assemble(
        crate::gate::infrastructure::assembly::GovernedAssemblyConfig {
            workspace: PathBuf::from(&repo_root),
            state_dir: state_dir.clone(),
            audit_key: config.audit_key.clone(),
            permission_mode: config.permission_mode,
            host: "run-receipt",
            orchestrator: crate::orchestrator::domain::OrchestratorConfig::default(),
            executor: crate::gate::infrastructure::assembly::ExecutorPolicy::default(),
            budget: crate::gate::infrastructure::assembly::LlmBudgetLimits::Unlimited,
            planning,
            audit_service,
            validation_loop: None,
            quality_gate_service: None,
            scored_evaluation_service: None,
            policy_engine: None,
            code_graph_service: None,
            seal: true,
        },
    )
    .await
    .map_err(|error| infrastructure(error.to_string()))?;

    let input = RunFromTemplateInput {
        steps,
        repo_root: repo_root.clone(),
        execution_id: Some(execution_id),
        template_name: format!("receipt-{execution_id}"),
        repository: None,
        author: None,
        identity: None,
        identity_verification_failure: None,
        enforcement_preset: None,
        receipt: Some(receipt.clone()),
    };

    let run = assembly
        .orchestrator
        .run_from_template(input)
        .await
        .map_err(|error| match error {
            // Rare and deliberate: the checks above passed and the orchestrator's own verification did
            // not. That is not a refusal of the declaration a caller can act on — it is a disagreement
            // between two producers of the same answer, so it is reported as the infrastructure fault
            // it is, with the engine's own sentence.
            crate::orchestrator::domain::error::OrchestratorError::ReceiptRefused {
                kinds,
                detail,
            } => infrastructure(format!(
                "the orchestrator refused a receipt this entry point accepted ({kinds}): {detail}"
            )),
            other => infrastructure(other.to_string()),
        })?;

    // A step refused before dispatch is not a step that ran: the declared work did not happen. The
    // code is read from the dispatcher's own record (#1625 added `failure_type` to the task result),
    // never from the error sentence, because a caller branches on the code.
    if let Some(refused) = run.record.task_results.iter().find_map(|task| {
        let code = task.failure_type.clone()?;
        // Only the codes that mean "this was refused, not attempted". A failure *inside* a tool is a
        // different fact: the effect may exist, and the caller decides what to do about it.
        (code == "precondition_denied" || code == "sandbox_undeclared").then_some((task, code))
    }) {
        let (task, code) = refused;
        return Err(RunReceiptError::RefusedAtGate {
            step: task.node_name.clone(),
            code,
            detail: task
                .error
                .clone()
                .unwrap_or_else(|| "the dispatcher recorded no detail".to_string()),
        });
    }

    let receipt_digest = run
        .receipt_digest
        .clone()
        .ok_or_else(|| infrastructure("the run produced no receipt digest".to_string()))?;
    let attestation_digest = run
        .attestation_digest
        .clone()
        .ok_or_else(|| infrastructure("the run produced no attestation digest".to_string()))?;

    // Read the observation back the way any verifier would: from the sealed bytes, through the
    // module's own two doors — the store's `get` for the location, `verify_attestation` for the
    // meaning. Returning the value the builder produced would be a second, unverified copy, and
    // composing the path here would be a second copy of the layout (#1628).
    let store = assembly.attestation_store.clone().ok_or_else(|| {
        infrastructure("the assembly was asked for a seal and returned none".into())
    })?;
    let stored = store
        .get(&execution_id.to_string())
        .map_err(|e| infrastructure(format!("the sealed attestation could not be read: {e}")))?;
    // The declaration the record is *about*, read from the record: what it names is what the run was
    // admitted under, or it is not the record of this run.
    let key = config.audit_key.as_deref().unwrap_or_default();
    let attestation = crate::receipt::verify_attestation(&stored.bytes, key.as_bytes())
        .map_err(|detail| {
            infrastructure(format!("the sealed attestation does not verify: {detail}"))
        })?
        .attestation()
        .clone();
    // The two producers must agree about the declaration, and the agreement is checked rather than
    // assumed: a digest mismatch here means the receipt the run was admitted under is not the receipt
    // it answered.
    if attestation.receipt_digest() != receipt_digest.as_str() {
        return Err(infrastructure(format!(
            "the attestation answers receipt {}, but the run was admitted under {receipt_digest}",
            attestation.receipt_digest()
        )));
    }

    Ok(RunReceiptOutput {
        attestation,
        attestation_digest,
        receipt_digest,
        execution_id: execution_id.to_string(),
    })
}
