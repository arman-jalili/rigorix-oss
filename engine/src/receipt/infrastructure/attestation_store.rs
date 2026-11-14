//! Writing a signed attestation into the state directory (ADR-024 §6, #1527).
//!
//! One record, one home. The store's job is small and its refusals are the interesting part:
//!
//! - a state directory **inside the workspace** is refused before anything is written, on resolved
//!   paths, because a seal the executing agent can rewrite is not a seal;
//! - an attestation with **no signature** is refused, because the only reader the module has
//!   (`verify_attestation`) requires one — a file that looks like evidence and cannot be verified is
//!   worse than no file, and the module already refuses to simulate signing;
//! - the write is a temporary file in the destination directory plus a rename, so a reader never
//!   sees half an attestation and a crash leaves the previous record intact.

use std::path::{Path, PathBuf};

use crate::receipt::domain::attestation::Attestation;
use crate::receipt::{refuse_state_dir_inside_workspace, sign};

/// Why an attestation could not be stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestationStoreError {
    /// The state directory resolves inside the workspace the agent can write (ADR-024 §6, rule 1).
    StateDirInsideWorkspace {
        /// The canonical state directory.
        state_dir: String,
        /// The canonical workspace root it was found inside.
        repo_root: String,
    },
    /// The attestation carries no signature, so `verify_attestation` could only refuse it.
    Unsigned {
        /// The execution it was about, so the operator can find the run that produced it.
        execution_id: String,
    },
    /// The record could not be written. `stage` names which half — `create` the directory, `write`
    /// the temporary file, or `rename` the final one — because "persistence failed" without the
    /// stage is a finding an operator cannot act on.
    WriteFailed {
        /// Which half of the write failed.
        stage: &'static str,
        /// The path the store was working on.
        path: String,
        /// The operating system's own words.
        detail: String,
    },
}

impl std::fmt::Display for AttestationStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::StateDirInsideWorkspace {
                state_dir,
                repo_root,
            } => write!(
                f,
                "the attestation store {state_dir} is inside the workspace {repo_root}, which the \
                 executing agent can write"
            ),
            Self::Unsigned { execution_id } => write!(
                f,
                "the attestation for execution {execution_id} is unsigned, and an unsigned record \
                 cannot be verified by the only reader this module has"
            ),
            Self::WriteFailed {
                stage,
                path,
                detail,
            } => write!(
                f,
                "the attestation write failed at {stage} ({path}): {detail}"
            ),
        }
    }
}

impl std::error::Error for AttestationStoreError {}

/// What a successful write returns: the digest the envelope references, and where the record is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredAttestation {
    /// sha256 over the attestation's canonical body — the same string the envelope carries.
    pub digest: String,
    /// The file the record was written to.
    pub path: PathBuf,
}

/// The file-backed attestation store: `<state dir>/attestations/<execution_id>.json`.
#[derive(Debug, Clone)]
pub struct FileAttestationStore {
    state_dir: PathBuf,
    repo_root: PathBuf,
    signing_key: Option<String>,
}

impl FileAttestationStore {
    /// A store over `state_dir`, for `repo_root`, signing with `signing_key` when one is wired.
    ///
    /// The key is a constructor argument rather than an environment read here, so the store does not
    /// have a second opinion about where keys come from: the composition root decides
    /// (`RIGORIX_HMAC_KEY`, the same variable the audit envelope is signed with) and passes it.
    #[must_use]
    pub fn new(
        state_dir: impl Into<PathBuf>,
        repo_root: impl Into<PathBuf>,
        signing_key: Option<String>,
    ) -> Self {
        Self {
            state_dir: state_dir.into(),
            repo_root: repo_root.into(),
            signing_key: signing_key.filter(|k| !k.is_empty()),
        }
    }

    /// Where this store writes.
    #[must_use]
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    /// Sign the attestation in place with the wired key.
    ///
    /// Returns whether a signature was produced. `false` is a fact the caller reports, never a
    /// silent skip: `put` then refuses to write.
    pub fn sign_if_keyed(&self, attestation: &mut Attestation) -> bool {
        let Some(key) = &self.signing_key else {
            return false;
        };
        // A key that cannot key HMAC-SHA256 is an operator error, and `sign` says so; treating it as
        // "unsigned" would turn a misconfigured deployment into a run with no record at all.
        sign(attestation, key.as_bytes()).is_ok()
    }

    /// Write the attestation, returning the digest the envelope references.
    ///
    /// # Errors
    /// `StateDirInsideWorkspace` before any write; `Unsigned` when no signature was produced;
    /// `WriteFailed` naming the stage that failed.
    pub fn put(
        &self,
        attestation: &mut Attestation,
    ) -> Result<StoredAttestation, AttestationStoreError> {
        self.sign_if_keyed(attestation);

        // The rule is checked on resolved paths, so `<repo>/x/../.rigorix/state` is refused even
        // though the string does not start with the repository.
        refuse_state_dir_inside_workspace(&self.state_dir, &self.repo_root).map_err(|refusal| {
            match refusal {
                crate::receipt::ReceiptRefusal::StateDirInsideWorkspace {
                    state_dir,
                    repo_root,
                } => AttestationStoreError::StateDirInsideWorkspace {
                    state_dir,
                    repo_root,
                },
                other => AttestationStoreError::WriteFailed {
                    stage: "refuse_state_dir_inside_workspace",
                    path: self.state_dir.to_string_lossy().to_string(),
                    detail: other.to_string(),
                },
            }
        })?;

        if !attestation.is_signed() {
            return Err(AttestationStoreError::Unsigned {
                execution_id: attestation.execution_id().to_string(),
            });
        }

        let digest = attestation
            .digest()
            .map_err(|detail| AttestationStoreError::WriteFailed {
                stage: "canonicalise",
                path: self.state_dir.to_string_lossy().to_string(),
                detail,
            })?;

        let dir = self.state_dir.join("attestations");
        // sink: receipt:attestation-file
        std::fs::create_dir_all(&dir).map_err(|e| AttestationStoreError::WriteFailed {
            stage: "create",
            path: dir.to_string_lossy().to_string(),
            detail: e.to_string(),
        })?;
        let path = dir.join(format!("{}.json", attestation.execution_id()));

        // The module's own wire shape: `Attestation` has no `Deserialize`, and `verify_attestation`
        // is the only door from bytes back to a value. Serialising here is the write half of that
        // door, not a second one.
        let body = serde_json::to_vec_pretty(attestation).map_err(|e| {
            AttestationStoreError::WriteFailed {
                stage: "serialise",
                path: path.to_string_lossy().to_string(),
                detail: e.to_string(),
            }
        })?;
        // The temporary name carries the digest, so two writers cannot collide on one scratch file
        // and a leftover can be tied to the record it belonged to.
        let tmp = dir.join(format!(
            ".{}.{}.tmp",
            attestation.execution_id(),
            &digest[..16]
        ));
        // sink: receipt:attestation-file
        std::fs::write(&tmp, &body).map_err(|e| AttestationStoreError::WriteFailed {
            stage: "write",
            path: tmp.to_string_lossy().to_string(),
            detail: e.to_string(),
        })?;
        // sink: receipt:attestation-file
        std::fs::rename(&tmp, &path).map_err(|e| AttestationStoreError::WriteFailed {
            stage: "rename",
            path: path.to_string_lossy().to_string(),
            detail: e.to_string(),
        })?;

        Ok(StoredAttestation { digest, path })
    }
}
