//! The receipt: the declaration of one execution, pinned by a digest (ADR-024 §4, §5, §8).
//!
//! Every field here is something a *host* says. Nothing in this type is an observation — the
//! observations live in [`super::attestation`], and the split is what makes "the execution did what
//! was asked" checkable: the declaration is fixed before anything runs, and the observation is
//! produced afterwards by the engine.
//!
//! The field set is ADR-024 §4's table and nothing else. A field added here without a decision in
//! the ADR is a field no verifier was designed for.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::quality_gates::domain::level::QualityLevel;

use super::digest::ReceiptDigest;
use super::refusal::{ReceiptRefusal, ReceiptVerdict};

/// The schema this module writes.
pub const RECEIPT_SCHEMA_VERSION: u16 = 1;

/// The receipt schema versions this engine reads.
pub const SUPPORTED_RECEIPT_SCHEMA_VERSIONS: &[u16] = &[RECEIPT_SCHEMA_VERSION];

/// What identifies a workspace (ADR-024 §5).
///
/// The identity carries its own `kind` so a decision that is not taken cannot be smuggled in as a
/// string: `CommitTree` ships, and a dirty-tree identity later is an additive vocabulary value that
/// an old reader refuses, rather than a schema break.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Workspace {
    /// Which identity this is.
    pub kind: WorkspaceKind,
    /// The identity itself — for `CommitTree`, `git rev-parse HEAD^{tree}`.
    pub value: String,
}

/// The workspace kinds this engine implements. Closed vocabulary, unknown = refuse.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum WorkspaceKind {
    /// A commit tree OID. The only kind 1.15.0 accepts.
    CommitTree,
}

impl WorkspaceKind {
    /// Every kind this engine implements, for a refusal that names them.
    #[must_use]
    pub fn supported() -> Vec<String> {
        vec!["commit_tree".to_string()]
    }

    /// Read a kind from a document, a CLI flag, or a tool argument.
    ///
    /// This is the door that produces `UnsupportedWorkspaceKind`, and it exists because the
    /// vocabulary's claim and the parser's behaviour were different things (#1548): the typed
    /// `Deserialize` refuses an unknown kind as `unknown variant ..., expected commit_tree` —
    /// serde's sentence, from serde's layer, never this type's refusal. A caller reading a
    /// document gets this type's answer from `Receipt::from_document`, which runs this door over
    /// the raw field before the typed parse; a caller that already has a string uses it directly.
    ///
    /// # Errors
    /// `UnsupportedWorkspaceKind`, quoting the kind it found and the kinds it supports.
    pub fn from_name(name: &str) -> Result<Self, ReceiptRefusal> {
        match name {
            "commit_tree" => Ok(Self::CommitTree),
            other => Err(ReceiptRefusal::UnsupportedWorkspaceKind {
                kind: other.to_string(),
                supported: Self::supported(),
            }),
        }
    }
}

impl Workspace {
    /// A workspace identity from an untyped kind name.
    ///
    /// # Errors
    /// Whatever `WorkspaceKind::from_name` refuses.
    pub fn from_name(kind: &str, value: impl Into<String>) -> Result<Self, ReceiptRefusal> {
        Ok(Self {
            kind: WorkspaceKind::from_name(kind)?,
            value: value.into(),
        })
    }
}

/// The sandbox a step runs in. `none` is deliberately not representable (ADR-024 §4): a receipt
/// that declares no isolation cannot be written, so a host cannot ask for an unisolated run by
/// omitting a field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Sandbox {
    /// The regime.
    pub regime: SandboxRegime,
    /// The image the regime resolves to.
    pub image_digest: String,
}

/// The sandbox regimes this engine implements.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxRegime {
    /// A container.
    Container,
    /// A virtual machine.
    Vm,
}

/// An effect a step declares it may produce.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeclaredEffect {
    /// The paths it may write.
    pub paths: Vec<String>,
    /// The digest the content at each path must have afterwards, parallel to `paths`.
    pub content_digests: Vec<String>,
}

/// What a step must meet to count as having done its job.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Expectation {
    /// What is expected, in words the operator can read back.
    pub description: String,
    /// The quality level this step is expected to establish, when it establishes one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub quality_level: Option<QualityLevel>,
}

/// One step of a receipt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReceiptStep {
    /// The step's id, unique within the receipt.
    pub id: String,
    /// The tool it invokes.
    pub tool: String,
    /// The tool's arguments.
    pub parameters: serde_json::Value,
    /// The steps it depends on, by id.
    pub depends_on: Vec<String>,
    /// What it may write.
    pub declared_effects: DeclaredEffect,
    /// What it must meet.
    pub expectation: Expectation,
    /// Whether a human must approve it first.
    pub requires_approval: bool,
    /// Its timeout.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub timeout_secs: Option<u64>,
}

/// A role and the model that plays it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Role {
    /// The role's name.
    pub role: String,
    /// The model's id.
    pub model_id: String,
    /// Its context window.
    pub context_window: u32,
    /// Whether it produces structured output.
    pub structured_output: bool,
}

/// The budget a receipt declares (ADR-024 §8). Enforcement is a separate issue, and this type says
/// so: there is no `enforced` here to set to `true`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct BudgetDeclared {
    /// Iterations the execution may use.
    pub max_iterations: u32,
    /// Tokens it may use.
    pub max_tokens: u64,
}

/// The declaration of one execution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Receipt {
    /// The schema version, first in the canonical bytes.
    pub schema_version: u16,
    /// The execution this receipt is for — one receipt, one execution (§5's resume half is #1528's).
    pub execution_id: Uuid,
    /// The workspace it applies to.
    pub workspace: Workspace,
    /// The sandbox its steps run in.
    pub sandbox: Sandbox,
    /// Its steps.
    pub steps: Vec<ReceiptStep>,
    /// The roles and models.
    pub roles: Vec<Role>,
    /// What it may spend.
    pub budget: BudgetDeclared,
    /// The quality level the caller claims it needs, **as a claim**.
    pub quality_required: QualityClaim,
    /// The digest over the declared effect set, for a caller that pins one separately.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub set_digest: Option<String>,
    /// The digests of the fixtures the execution reads.
    #[serde(default)]
    pub fixture_digests: Vec<String>,
}

/// A quality level **and whose claim it is** (ADR-024 §2 D1).
///
/// On a receipt the source is always `Caller`: nothing has run yet. The field exists on the receipt
/// rather than being implied, so the attestation can echo it without inventing one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct QualityClaim {
    /// The level.
    pub level: QualityLevel,
    /// Always `Caller` here — a receipt is a declaration.
    pub source: ClaimSource,
}

/// Who made a claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ClaimSource {
    /// The caller.
    Caller,
}

impl Receipt {
    /// Build a receipt as the caller's declaration, refusing what cannot be a declaration at all.
    ///
    /// Construction is where the parameter scan happens (ADR-009, ADR-024 §4): a receipt that
    /// carries a credential is refused rather than stored quietly, because everything downstream
    /// would copy it into an attestation and a log.
    ///
    /// # Errors
    /// `SecretInParameters` for the first parameter carrying a credential.
    pub fn new(
        execution_id: Uuid,
        workspace: Workspace,
        sandbox: Sandbox,
        steps: Vec<ReceiptStep>,
        roles: Vec<Role>,
        budget: BudgetDeclared,
        quality_required: QualityLevel,
    ) -> Result<Self, ReceiptRefusal> {
        for step in &steps {
            if let Some(path) = find_secret(&step.parameters) {
                return Err(ReceiptRefusal::SecretInParameters {
                    step: step.id.clone(),
                    path,
                });
            }
        }
        Ok(Self {
            schema_version: RECEIPT_SCHEMA_VERSION,
            execution_id,
            workspace,
            sandbox,
            steps,
            roles,
            budget,
            quality_required: QualityClaim {
                level: quality_required,
                source: ClaimSource::Caller,
            },
            set_digest: None,
            fixture_digests: Vec::new(),
        })
    }

    /// The digest over this receipt's canonical form.
    ///
    /// # Errors
    /// The canonical-form error — a defect, since every field serializes.
    pub fn digest(&self) -> Result<ReceiptDigest, String> {
        ReceiptDigest::of(self)
    }

    /// The declared step ids, in order.
    #[must_use]
    pub fn step_ids(&self) -> Vec<String> {
        self.steps.iter().map(|s| s.id.clone()).collect()
    }

    /// The digest-pinned verdict: this is what a caller checks before dispatching anything.
    ///
    /// `workspace_identity` is the identity the engine computed for the tree it was handed — the
    /// engine never asks the receipt which workspace it is in.
    #[must_use]
    pub fn verdict_against(&self, workspace_identity: &str) -> ReceiptVerdict {
        let mut refusals = Vec::new();
        if !SUPPORTED_RECEIPT_SCHEMA_VERSIONS.contains(&self.schema_version) {
            refusals.push(ReceiptRefusal::SchemaVersion {
                found: self.schema_version,
                supported: SUPPORTED_RECEIPT_SCHEMA_VERSIONS.to_vec(),
            });
        }
        if self.workspace.value != workspace_identity {
            refusals.push(ReceiptRefusal::WorkspaceMismatch {
                receipt: self.workspace.value.clone(),
                workspace: workspace_identity.to_string(),
            });
        }
        refusals.extend(self.structure_refusals());
        if refusals.is_empty() {
            ReceiptVerdict::Verified
        } else {
            ReceiptVerdict::Refused(refusals)
        }
    }

    /// Read a receipt document.
    ///
    /// Two fields are refused **in this module's words** before the typed parse, because they are
    /// closed vocabularies and serde would otherwise answer with its own sentence:
    ///
    /// - `schema_version` → `SchemaVersion`, naming the version and the ones this engine reads;
    /// - `workspace.kind` → `UnsupportedWorkspaceKind`, naming the kind it found and the ones it
    ///   supports (`SandboxRegime`'s own closed set is reached by the typed parse; this module has
    ///   no refusal for a regime, because `none` is not representable at all — see the type's doc).
    ///
    /// Everything else is the parser's answer, carried as `MalformedDocument`, so the caller reads
    /// one vocabulary instead of one vocabulary plus serde's.
    ///
    /// # Errors
    /// Every refusal the document meets, not the first: a document with a wrong version **and** an
    /// unimplemented kind is told both, in one pass.
    pub fn from_document(bytes: &str) -> Result<Self, Vec<ReceiptRefusal>> {
        let raw: serde_json::Value = match serde_json::from_str(bytes) {
            Ok(value) => value,
            Err(e) => {
                return Err(vec![ReceiptRefusal::MalformedDocument {
                    detail: e.to_string(),
                }]);
            }
        };

        let mut refusals = Vec::new();

        if let Some(found) = raw
            .get("schema_version")
            .and_then(serde_json::Value::as_u64)
        {
            let found = u16::try_from(found).unwrap_or(u16::MAX);
            if !SUPPORTED_RECEIPT_SCHEMA_VERSIONS.contains(&found) {
                refusals.push(ReceiptRefusal::SchemaVersion {
                    found,
                    supported: SUPPORTED_RECEIPT_SCHEMA_VERSIONS.to_vec(),
                });
            }
        }

        // The kind is read as a string first, so the refusal quotes what the document said rather
        // than what serde made of it. `from_name` is the only door that produces this refusal.
        if let Some(Err(refusal)) = raw
            .get("workspace")
            .and_then(|w| w.get("kind"))
            .and_then(serde_json::Value::as_str)
            .map(WorkspaceKind::from_name)
        {
            refusals.push(refusal);
        }

        if !refusals.is_empty() {
            return Err(refusals);
        }

        let receipt: Self = match serde_json::from_value(raw) {
            Ok(receipt) => receipt,
            Err(e) => {
                return Err(vec![ReceiptRefusal::MalformedDocument {
                    detail: e.to_string(),
                }]);
            }
        };

        let mut rest = receipt.structure_refusals();
        if rest.is_empty() {
            Ok(receipt)
        } else {
            rest.shrink_to_fit();
            Err(rest)
        }
    }

    /// The structural refusals: unknown steps, cycles. Split out because a host may want them
    /// without a workspace, and because the workspace is the engine's to compute, not the host's.
    #[must_use]
    pub fn structure_refusals(&self) -> Vec<ReceiptRefusal> {
        let mut refusals = Vec::new();
        let known = self.step_ids();
        for step in &self.steps {
            for dependency in &step.depends_on {
                if !known.contains(dependency) {
                    refusals.push(ReceiptRefusal::UnknownStep {
                        step: dependency.clone(),
                        known: known.clone(),
                    });
                }
            }
            if step.declared_effects.paths.len() != step.declared_effects.content_digests.len() {
                // A paths/digests pair that is not a pair cannot be compared later, and a
                // comparison that silently truncates is how an undeclared write passes.
                refusals.push(ReceiptRefusal::ContentDigestMismatch {
                    step: step.id.clone(),
                    path: "<declared_effects>".to_string(),
                    declared: format!("{} path(s)", step.declared_effects.paths.len()),
                    observed: format!("{} digest(s)", step.declared_effects.content_digests.len()),
                });
            }
        }
        if let Some(cycle) = self.cycle() {
            refusals.push(ReceiptRefusal::DependencyCycle { steps: cycle });
        }
        refusals
    }

    /// The steps that no order can satisfy, if there are any.
    fn cycle(&self) -> Option<Vec<String>> {
        let mut remaining: Vec<&str> = self.steps.iter().map(|s| s.id.as_str()).collect();
        let mut resolved: Vec<&str> = Vec::new();
        loop {
            let ready: Vec<&str> = remaining
                .iter()
                .copied()
                .filter(|id| {
                    let Some(step) = self.steps.iter().find(|s| s.id == *id) else {
                        return true;
                    };
                    step.depends_on
                        .iter()
                        .all(|dep| resolved.contains(&dep.as_str()))
                })
                .collect();
            if ready.is_empty() {
                break;
            }
            for id in ready {
                resolved.push(id);
                remaining.retain(|r| *r != id);
            }
        }
        if remaining.is_empty() {
            None
        } else {
            Some(remaining.into_iter().map(str::to_string).collect())
        }
    }
}

/// Find a credential in a parameter tree, returning its JSON path rather than its value.
///
/// The value is never returned: a refusal that quotes the secret puts it in the log the refusal was
/// meant to keep it out of (ADR-009's redaction rule, applied at construction).
fn find_secret(value: &serde_json::Value) -> Option<String> {
    const SUSPECT: &[&str] = &[
        "api_key",
        "apikey",
        "token",
        "secret",
        "password",
        "passwd",
        "credential",
        "private_key",
        "access_key",
        "authorization",
    ];
    match value {
        serde_json::Value::Object(map) => {
            for (key, child) in map {
                let lowered = key.to_ascii_lowercase();
                if SUSPECT.iter().any(|s| lowered.contains(s)) {
                    return Some(key.clone());
                }
                if let Some(path) = find_secret(child) {
                    return Some(format!("{key}.{path}"));
                }
            }
            None
        }
        serde_json::Value::Array(items) => {
            for (index, item) in items.iter().enumerate() {
                if let Some(path) = find_secret(item) {
                    return Some(format!("[{index}].{path}"));
                }
            }
            None
        }
        _ => None,
    }
}
