//! The refusals a receipt can meet, and the verdict shape (ADR-009, ADR-024 §4).
//!
//! The vocabulary is closed and every unknown value is refused — no `#[serde(other)]` catch-all,
//! because a catch-all turns "this engine does not know that refusal" into "some refusal happened",
//! and the caller cannot act on that. The shape copies `GateOutcome`'s
//! `Passed | Refused(..)` so an integrator reads one vocabulary, not two.
//!
//! Two rules about this enum, both learned the hard way (#1548):
//!
//! - **A variant exists only where it can be produced.** A refusal nothing constructs is a verdict
//!   a caller may branch on and never receive. `Receipt::from_document` is the door for the two
//!   closed vocabularies (`SchemaVersion`, `UnsupportedWorkspaceKind`), because serde would answer
//!   with its own sentence instead; a test scans for a variant with no construction site.
//! - **ADR-024 §6's re-seal rule has no variant here**, because nothing persists a seal yet. The
//!   rule is recorded in the ADR rather than advertised in this type.

use serde::{Deserialize, Serialize};

/// Why a receipt was refused.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "refusal", rename_all = "snake_case")]
pub enum ReceiptRefusal {
    /// The document names a `schema_version` this engine does not read.
    SchemaVersion {
        /// The version the document named.
        found: u16,
        /// The versions this engine reads.
        supported: Vec<u16>,
    },
    /// The stored digest and the receipt's canonical form disagree.
    DigestMismatch {
        /// What the seal says.
        expected: String,
        /// What the bytes say now.
        actual: String,
    },
    /// The receipt applies to a different workspace than the one presented.
    WorkspaceMismatch {
        /// The identity in the receipt.
        receipt: String,
        /// The identity of the workspace the engine was given.
        workspace: String,
    },
    /// A step names a dependency, or the receipt names a step, that does not exist.
    UnknownStep {
        /// The name that is not a step.
        step: String,
        /// The steps that do exist.
        known: Vec<String>,
    },
    /// The declared dependencies cannot be ordered.
    DependencyCycle {
        /// The steps that remain when nothing is ready.
        steps: Vec<String>,
    },
    /// The document declares no sandbox, and "no sandbox" is not a value this vocabulary has.
    SandboxMissing,
    /// A parameter value carries a credential. Refused, never stored quietly.
    SecretInParameters {
        /// The step carrying it.
        step: String,
        /// The parameter's JSON path, so the author can find it without reading the receipt out.
        path: String,
    },
    /// A workspace identity names a kind this engine does not implement (ADR-024 §5).
    UnsupportedWorkspaceKind {
        /// The kind the receipt named.
        kind: String,
        /// The kinds this engine implements.
        supported: Vec<String>,
    },
    /// A step wrote a path it did not declare.
    UndeclaredWrite {
        /// The step.
        step: String,
        /// The path it wrote.
        path: String,
        /// What it did declare.
        declared: Vec<String>,
    },
    /// A step wrote content whose digest differs from the declared digest.
    ContentDigestMismatch {
        /// The step.
        step: String,
        /// The path.
        path: String,
        /// What the receipt declared.
        declared: String,
        /// What the engine observed.
        observed: String,
    },
    /// The state directory is inside the workspace the agent can write (ADR-024 §6).
    StateDirInsideWorkspace {
        /// The canonical state directory.
        state_dir: String,
        /// The canonical workspace root it was found inside.
        repo_root: String,
    },
    /// The document is not readable JSON, or not the shape this module reads.
    ///
    /// Distinct from `SchemaVersion` and `UnsupportedWorkspaceKind`, which are fields this module
    /// refuses *in its own words* before the typed parse: everything else is the parser's answer,
    /// carried here so a caller reads one vocabulary.
    MalformedDocument {
        /// The parser's message, or the field that is missing.
        detail: String,
    },
}

impl ReceiptRefusal {
    /// A stable kind, for callers that branch without matching fields.
    #[must_use]
    pub const fn kind(&self) -> &'static str {
        match self {
            Self::SchemaVersion { .. } => "schema_version",
            Self::DigestMismatch { .. } => "digest_mismatch",
            Self::WorkspaceMismatch { .. } => "workspace_mismatch",
            Self::UnknownStep { .. } => "unknown_step",
            Self::DependencyCycle { .. } => "dependency_cycle",
            Self::SandboxMissing => "sandbox_missing",
            Self::SecretInParameters { .. } => "secret_in_parameters",
            Self::UnsupportedWorkspaceKind { .. } => "unsupported_workspace_kind",
            Self::UndeclaredWrite { .. } => "undeclared_write",
            Self::ContentDigestMismatch { .. } => "content_digest_mismatch",
            Self::StateDirInsideWorkspace { .. } => "state_dir_inside_workspace",
            Self::MalformedDocument { .. } => "malformed_document",
        }
    }
}

impl std::fmt::Display for ReceiptRefusal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SchemaVersion { found, supported } => write!(
                f,
                "receipt schema_version {found} is not one this engine reads (supported: {supported:?})"
            ),
            Self::DigestMismatch { expected, actual } => write!(
                f,
                "receipt digest does not match its bytes: sealed {expected}, computed {actual}"
            ),
            Self::WorkspaceMismatch { receipt, workspace } => write!(
                f,
                "receipt applies to workspace {receipt}, but the engine was given {workspace}"
            ),
            Self::UnknownStep { step, known } => {
                write!(
                    f,
                    "receipt names step '{step}', which is not one of {known:?}"
                )
            }
            Self::DependencyCycle { steps } => write!(
                f,
                "the receipt's dependencies form a cycle among {steps:?}: nothing can run first"
            ),
            Self::SandboxMissing => write!(
                f,
                "the receipt declares no sandbox, and this vocabulary has no 'none' regime"
            ),
            Self::SecretInParameters { step, path } => write!(
                f,
                "step '{step}' carries a credential in parameter '{path}'; a receipt stores no secrets"
            ),
            Self::UnsupportedWorkspaceKind { kind, supported } => write!(
                f,
                "workspace kind '{kind}' is not implemented (supported: {supported:?})"
            ),
            Self::UndeclaredWrite {
                step,
                path,
                declared,
            } => write!(
                f,
                "step '{step}' wrote '{path}', which it did not declare (declared: {declared:?})"
            ),
            Self::ContentDigestMismatch {
                step,
                path,
                declared,
                observed,
            } => write!(
                f,
                "step '{step}' wrote '{path}' with digest {observed}, not the declared {declared}"
            ),
            Self::StateDirInsideWorkspace {
                state_dir,
                repo_root,
            } => write!(
                f,
                "the state directory {state_dir} is inside the workspace {repo_root}, which the \
                 executing agent can write"
            ),
            Self::MalformedDocument { detail } => {
                write!(
                    f,
                    "the receipt document is not one this module reads: {detail}"
                )
            }
        }
    }
}

/// The verdict, in the shape `GateOutcome` already uses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptVerdict {
    /// The receipt may run as written.
    Verified,
    /// It may not, and these are the reasons — all of them, not the first.
    Refused(Vec<ReceiptRefusal>),
}

impl ReceiptVerdict {
    /// Whether the receipt may run.
    #[must_use]
    pub fn is_verified(&self) -> bool {
        matches!(self, Self::Verified)
    }

    /// The refusals, empty when verified.
    #[must_use]
    pub fn refusals(&self) -> &[ReceiptRefusal] {
        match self {
            Self::Verified => &[],
            Self::Refused(refusals) => refusals,
        }
    }
}
