//! `verify` a receipt before anything runs, and `sign` what the engine observed (#1527).
//!
//! The order matters and is the same order the refusal list is built in: the whole declaration is
//! checked as a value — schema, digest, workspace, steps, cycle, sandbox, credential scan — and only
//! a `VerifiedReceipt` can be turned into a graph. A refusal is not a warning: nothing is
//! dispatched, so "the receipt was refused" and "nothing ran" are the same statement.
//!
//! # The two directions of trust
//!
//! [`verify`] answers "may this run as declared?". [`AttestationBuilder`] answers "what did it do?"
//! and cannot answer anything the engine did not observe: the receipt's requirements are echoed, and
//! the caller's quality claim is copied with `source: Caller` unless a step in this execution
//! produced that level.

use crate::quality_gates::domain::level::QualityLevel;
use crate::receipt::domain::attestation::{
    Attestation, BudgetEcho, ObservedEffect, ObservedStep, QualityRecord, QualitySource,
};
use crate::receipt::domain::digest::sha256_hex;
use crate::receipt::domain::receipt::{Receipt, Sandbox};
use crate::receipt::domain::refusal::ReceiptRefusal;

/// A receipt that passed every check, with the digest the checks computed. The newtype is the gate:
/// `verify` is the only function that returns one.
#[derive(Debug, Clone)]
pub struct VerifiedReceipt {
    receipt: Receipt,
    digest: String,
}

impl VerifiedReceipt {
    /// The receipt.
    #[must_use]
    pub fn receipt(&self) -> &Receipt {
        &self.receipt
    }

    /// The digest every downstream artifact pins.
    #[must_use]
    pub fn digest(&self) -> &str {
        &self.digest
    }
}

/// Verify a receipt against the workspace identity the engine computed.
///
/// `workspace_identity` is the engine's answer (for `CommitTree`, the tree OID of the tree it was
/// handed), never the receipt's own string: a receipt that names its workspace and is then compared
/// against that name proves nothing.
///
/// # Errors
/// Every refusal the receipt meets, not the first. An empty refusal list cannot occur: the error is
/// only returned when at least one check failed.
pub fn verify(
    receipt: &Receipt,
    workspace_identity: &str,
) -> Result<VerifiedReceipt, Vec<ReceiptRefusal>> {
    let mut refusals = Vec::new();

    // The digest is over the canonical form, so a receipt whose bytes changed since sealing is
    // refused before any of its fields are believed.
    let computed = match receipt.digest() {
        Ok(digest) => digest.as_str().to_string(),
        Err(e) => {
            return Err(vec![ReceiptRefusal::DigestMismatch {
                expected: "<uncomputable>".to_string(),
                actual: e,
            }]);
        }
    };
    if let Some(sealed) = &receipt.set_digest
        && sealed != &computed
    {
        refusals.push(ReceiptRefusal::DigestMismatch {
            expected: sealed.clone(),
            actual: computed.clone(),
        });
    }

    // §4: `none` is not a sandbox regime, so an empty image is the only way to express "no sandbox"
    // in a document — and it is refused rather than read as "unspecified".
    if matches!(receipt.sandbox, Sandbox { ref image_digest, .. } if image_digest.trim().is_empty())
    {
        refusals.push(ReceiptRefusal::SandboxMissing);
    }

    if receipt.workspace.value != workspace_identity {
        refusals.push(ReceiptRefusal::WorkspaceMismatch {
            receipt: receipt.workspace.value.clone(),
            workspace: workspace_identity.to_string(),
        });
    }
    if !crate::receipt::domain::receipt::SUPPORTED_RECEIPT_SCHEMA_VERSIONS
        .contains(&receipt.schema_version)
    {
        refusals.push(ReceiptRefusal::SchemaVersion {
            found: receipt.schema_version,
            supported: crate::receipt::domain::receipt::SUPPORTED_RECEIPT_SCHEMA_VERSIONS.to_vec(),
        });
    }
    refusals.extend(receipt.structure_refusals());

    if refusals.is_empty() {
        Ok(VerifiedReceipt {
            receipt: receipt.clone(),
            digest: computed,
        })
    } else {
        Err(refusals)
    }
}

/// Compare what a step declared against what the engine observed.
///
/// The observed set is the engine's; it is never taken from the host. An undeclared path, or a
/// content digest that differs from the declared one, is a refusal naming the path — a receipt that
/// overruns its declaration is exactly the case this comparison exists for.
#[must_use]
pub fn compare_declared_observed(
    step_id: &str,
    declared: &crate::receipt::domain::receipt::DeclaredEffect,
    observed: &[ObservedEffect],
) -> Vec<ReceiptRefusal> {
    let mut refusals = Vec::new();
    for effect in observed {
        match declared.paths.iter().position(|p| p == &effect.path) {
            None => refusals.push(ReceiptRefusal::UndeclaredWrite {
                step: step_id.to_string(),
                path: effect.path.clone(),
                declared: declared.paths.clone(),
            }),
            Some(index) => {
                let expected = declared.content_digests.get(index);
                if let Some(expected) = expected
                    && expected != &effect.content_digest
                    && !expected.is_empty()
                {
                    refusals.push(ReceiptRefusal::ContentDigestMismatch {
                        step: step_id.to_string(),
                        path: effect.path.clone(),
                        declared: expected.clone(),
                        observed: effect.content_digest.clone(),
                    });
                }
            }
        }
    }
    refusals
}

/// Build the attestation from what the execution observed.
///
/// The builder is the engine's: it takes the verified receipt, the observations, the settled effect
/// ids and the budget counters, and it refuses to record a quality level the execution did not
/// establish.
#[derive(Debug)]
pub struct AttestationBuilder {
    receipt_digest: String,
    execution_id: uuid::Uuid,
    steps: Vec<ObservedStep>,
    settled_effects: Vec<String>,
    identity: Option<String>,
    declared_iterations: u32,
    declared_tokens: u64,
    quality: QualityRecord,
    /// The level the caller declared, kept separately: a step can raise the record above the claim,
    /// but never by silently rewriting what the host asked for.
    caller_level: QualityLevel,
}

impl AttestationBuilder {
    /// Start from a verified receipt. A builder cannot be started from an unverified one, which is
    /// the type-level half of "a refused receipt dispatches nothing".
    #[must_use]
    pub fn from_verified(verified: &VerifiedReceipt) -> Self {
        let receipt = verified.receipt();
        Self {
            receipt_digest: verified.digest().to_string(),
            execution_id: receipt.execution_id,
            steps: Vec::new(),
            settled_effects: Vec::new(),
            identity: None,
            declared_iterations: receipt.budget.max_iterations,
            declared_tokens: receipt.budget.max_tokens,
            // The caller's claim, recorded as a claim. A builder that started from `None` here would
            // have to invent a level at `build()` time, which is how "unknown" becomes "merge-ready".
            quality: QualityRecord {
                level: receipt.quality_required.level,
                source: QualitySource::Caller,
            },
            caller_level: receipt.quality_required.level,
        }
    }

    /// Record what one step did.
    pub fn observe_step(&mut self, step: ObservedStep) -> &mut Self {
        self.steps.push(step);
        self
    }

    /// Record a settled effect id.
    pub fn settle_effect(&mut self, effect_id: impl Into<String>) -> &mut Self {
        self.settled_effects.push(effect_id.into());
        self
    }

    /// Record the identity, already redacted by the caller.
    pub fn identity(&mut self, identity: Option<String>) -> &mut Self {
        self.identity = identity;
        self
    }

    /// Record that a step in **this** execution established a quality level.
    ///
    /// This is the only way a level becomes `source: Engine`, and it needs the step id that
    /// establishes it. Ascending levels replace the caller's claim; a lower one does not, because a
    /// later `cargo test -p x` must not overwrite a recorded workspace run.
    pub fn quality_from_step(
        &mut self,
        step_id: impl Into<String>,
        level: QualityLevel,
    ) -> &mut Self {
        // A step establishes a level only by exceeding the caller's claim. Below it, the record stays
        // the claim it was: the engine proved less than the host asked for, and saying otherwise
        // would upgrade a claim into a pass — the failure this whole module exists to prevent.
        if level > self.caller_level {
            self.quality = QualityRecord {
                level,
                source: QualitySource::Engine {
                    step_id: step_id.into(),
                },
            };
        }
        self
    }

    /// Finish, echoing the budget and the observations.
    #[must_use]
    pub fn build(self, observed_iterations: u32, observed_tokens: u64) -> Attestation {
        Attestation::assemble(
            self.receipt_digest,
            self.execution_id,
            self.steps,
            self.quality,
            self.settled_effects,
            self.identity,
            BudgetEcho {
                declared_iterations: self.declared_iterations,
                declared_tokens: self.declared_tokens,
                observed_iterations,
                observed_tokens,
                // Nothing in this change refuses a dispatch on budget; the field says so rather
                // than leaving a reader to infer that the numbers were limits.
                enforced: false,
            },
        )
    }
}

/// Sign an attestation in place with HMAC-SHA256 over its canonical body (ADR-024 §3 D2).
///
/// The mechanism is the audit envelope's: the same HMAC over the same kind of canonical bytes, with
/// the claim ceiling stated in the ADR — tamper evidence for a verifier that holds the key, not
/// third-party proof.
///
/// # Errors
/// The canonical-form error, or a key that cannot key HMAC-SHA256.
pub fn sign(attestation: &mut Attestation, key: &[u8]) -> Result<(), String> {
    use hmac::{Hmac, KeyInit, Mac};
    use sha2::Sha256;
    let body = attestation.canonical_body()?;
    let mut mac = <Hmac<Sha256> as KeyInit>::new_from_slice(key)
        .map_err(|_| "the key cannot key HMAC-SHA256".to_string())?;
    mac.update(&body);
    let signature: String = mac
        .finalize()
        .into_bytes()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    attestation.set_signature(signature);
    Ok(())
}

/// Where the state directory is for a repository (ADR-024 §6 E4-a/E4-b).
///
/// `RIGORIX_STATE_DIR` when set; otherwise a **sibling** of the repository, never inside it. The
/// sibling default is what makes the rule hold without configuration: the agent's write tools are
/// contained to `workspace_root`, so anything under it is reachable and anything outside it is not.
/// It is tamper-*evidence*, not tamper-proofing — a different user with the same path can still read
/// it — and the ADR says so rather than implying a permission model this does not have.
#[must_use]
pub fn default_state_dir(repo_root: &str) -> std::path::PathBuf {
    // sink: receipt:state-dir-env
    if let Ok(declared) = std::env::var("RIGORIX_STATE_DIR")
        && !declared.trim().is_empty()
    {
        return std::path::PathBuf::from(declared);
    }
    let root = std::path::Path::new(repo_root);
    let name = root
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "workspace".to_string());
    root.parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .join(".execution-state")
        .join(name)
}

/// Refuse a state directory that is inside the workspace (ADR-024 §6, rule 1).
///
/// Both paths are canonicalised before comparison, so the rule is about where the store *is*, not
/// about how it was spelled: `<repo>/x/../.rigorix/state` is inside the repository even though the
/// string does not start with it.
///
/// # Errors
/// `StateDirInsideWorkspace`, naming both canonical paths.
pub fn refuse_state_dir_inside_workspace(
    state_dir: &std::path::Path,
    repo_root: &std::path::Path,
) -> Result<(), ReceiptRefusal> {
    // A path that does not exist yet cannot be canonicalised, so it is first normalised lexically
    // (`.`, `..`) and then resolved through its nearest existing ancestor. The lexical pass is what
    // makes `sub/../.rigorix/state` and `.rigorix/state` the same location; `Path::file_name` is
    // `None` for a trailing `..`, which is exactly how a "canonicaliser" that forgot this hands back
    // the path it was given. When the path does exist, `canonicalize` is used and symlinks are
    // resolved like anywhere else.
    fn normalize(path: &std::path::Path) -> std::path::PathBuf {
        let mut out = std::path::PathBuf::new();
        for component in path.components() {
            match component {
                std::path::Component::CurDir => {}
                std::path::Component::ParentDir => {
                    out.pop();
                }
                other => out.push(other.as_os_str()),
            }
        }
        out
    }

    fn canonical(path: &std::path::Path) -> std::path::PathBuf {
        let normalized = normalize(path);
        if let Ok(canonical) = normalized.canonicalize() {
            return canonical;
        }
        let mut existing = normalized.clone();
        let mut tail = Vec::new();
        while !existing.exists() {
            match (existing.parent(), existing.file_name()) {
                (Some(parent), Some(name)) => {
                    tail.push(name.to_os_string());
                    existing = parent.to_path_buf();
                }
                _ => return normalized,
            }
        }
        let mut out = existing
            .canonicalize()
            .unwrap_or_else(|_| existing.to_path_buf());
        for component in tail.into_iter().rev() {
            out.push(component);
        }
        out
    }

    let state = canonical(state_dir);
    let root = canonical(repo_root);
    if state.starts_with(&root) {
        return Err(ReceiptRefusal::StateDirInsideWorkspace {
            state_dir: state.to_string_lossy().to_string(),
            repo_root: root.to_string_lossy().to_string(),
        });
    }
    Ok(())
}

/// The digest of a sandbox, for a receipt that pins its image without naming a registry.
#[must_use]
pub fn sandbox_digest(sandbox: &Sandbox) -> String {
    sha256_hex(format!("{:?}:{}", sandbox.regime, sandbox.image_digest).as_bytes())
}
