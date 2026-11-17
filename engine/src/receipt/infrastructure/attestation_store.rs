//! Writing a signed attestation into the state directory (ADR-024 §6, #1527).
//!
//! One record, one home, and one door each way (#1628). The store's job is small and its refusals are
//! the interesting part:
//!
//! - a state directory **inside the workspace** is refused before anything is written, on resolved
//!   paths, because a seal the executing agent can rewrite is not a seal;
//! - an attestation with **no signature** is refused, because the only reader the module has
//!   (`verify_attestation`) requires one — a file that looks like evidence and cannot be verified is
//!   worse than no file, and the module already refuses to simulate signing;
//! - the write is a temporary file in the destination directory plus a rename, so a reader never
//!   sees half an attestation and a crash leaves the previous record intact.
//!
//! **The location rule is the store's, not its callers'.** `put` and `get` compute the same path from
//! the same function, so a layout change is one edit; before #1628 `receipt::api::run_receipt` spelled
//! `<state dir>/attestations/<id>.json` itself, which meant the second copy of the rule was the one
//! that would be edited. `FileReceiptStore` already had `get` — the observation was the asymmetrical
//! half.

use std::path::{Path, PathBuf};

use crate::receipt::domain::attestation::Attestation;
use crate::receipt::{refuse_state_dir_inside_workspace, sign};

/// Why an attestation could not be stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AttestationStoreError {
    /// A rule of the receipt vocabulary refused the record — the seal's location, or a re-seal that
    /// would replace one. The refusal carries its own sentence and kind, so a caller branches on the
    /// vocabulary it already reads rather than on a second copy of the same rules.
    Refused(crate::receipt::ReceiptRefusal),
    /// The attestation carries no signature, so `verify_attestation` could only refuse it.
    Unsigned {
        /// The execution it was about, so the operator can find the run that produced it.
        execution_id: String,
    },
    /// No record exists for that execution. Typed, so a caller distinguishes "never sealed" from
    /// "cannot read": a verifier handed a digest from last week and a host displaying a run that just
    /// finished ask different questions, and only one of them is a store failure.
    UnknownExecution {
        /// The execution that was asked for.
        execution_id: String,
    },
    /// The execution id cannot name a file. Refused rather than sanitised: `get` takes a string,
    /// unlike `put`, which takes the id from a `Uuid`, so the read side is the one place a caller
    /// could hand in `../x` or a path separator. A store that silently rewrote the name would return
    /// a record for something the caller did not ask for.
    InvalidExecutionId {
        /// What the caller asked for.
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
            Self::Refused(refusal) => write!(f, "{refusal}"),
            Self::Unsigned { execution_id } => write!(
                f,
                "the attestation for execution {execution_id} is unsigned, and an unsigned record \
                 cannot be verified by the only reader this module has"
            ),
            Self::UnknownExecution { execution_id } => write!(
                f,
                "no attestation has been sealed for execution {execution_id}"
            ),
            Self::InvalidExecutionId { execution_id } => write!(
                f,
                "{execution_id:?} cannot name an attestation: an execution id is a file stem, not a path"
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
    /// The file the record lives in.
    pub path: PathBuf,
    /// Whether the record was already there with this digest, in which case nothing was written.
    pub already_sealed: bool,
    /// The bytes the store wrote, or read back — exactly what a verifier receives.
    ///
    /// Carried on the stored record rather than fetched by a second call, so "the bytes the store
    /// wrote" and "the bytes a reader gets" cannot diverge: a reader that re-serialised the value
    /// would make the sealed artifact unavailable to the one function that verifies it.
    pub bytes: Vec<u8>,
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

    /// Where a record for `execution_id` lives — the one place the layout is written.
    ///
    /// # Errors
    /// `InvalidExecutionId` when the id is not a plain file stem.
    fn record_path(&self, execution_id: &str) -> Result<PathBuf, AttestationStoreError> {
        let names_a_file = !execution_id.is_empty()
            && execution_id
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !names_a_file {
            return Err(AttestationStoreError::InvalidExecutionId {
                execution_id: execution_id.to_string(),
            });
        }
        Ok(self
            .state_dir
            .join("attestations")
            .join(format!("{execution_id}.json")))
    }

    /// Read a sealed attestation back, verified, with the bytes a verifier would receive.
    ///
    /// The location rule is enforced on this side too, on resolved paths: a state directory inside
    /// the workspace is refused both ways, because a reader that trusted a directory the writer
    /// refused would turn a refused write into a readable record.
    ///
    /// # Errors
    /// `Refused` when the state directory is inside the workspace, or when the sealed bytes do not
    /// verify; `UnknownExecution` when nothing has been sealed for that id; `InvalidExecutionId`;
    /// `WriteFailed` when the file exists and cannot be read.
    pub fn get(&self, execution_id: &str) -> Result<StoredAttestation, AttestationStoreError> {
        refuse_state_dir_inside_workspace(&self.state_dir, &self.repo_root)
            .map_err(AttestationStoreError::Refused)?;
        let path = self.record_path(execution_id)?;
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                return Err(AttestationStoreError::UnknownExecution {
                    execution_id: execution_id.to_string(),
                });
            }
            Err(error) => {
                return Err(AttestationStoreError::WriteFailed {
                    stage: "read",
                    path: path.to_string_lossy().to_string(),
                    detail: error.to_string(),
                });
            }
        };
        // The same door `put` uses to compare a re-seal: a record that does not verify is not a
        // record, whatever its name says.
        let key = self.signing_key.as_deref().unwrap_or_default();
        let sealed =
            crate::receipt::verify_attestation(&bytes, key.as_bytes()).map_err(|detail| {
                AttestationStoreError::Refused(crate::receipt::ReceiptRefusal::MalformedDocument {
                    detail: format!(
                        "the sealed attestation at {} does not verify: {detail}",
                        path.display()
                    ),
                })
            })?;
        let digest =
            sealed
                .attestation()
                .digest()
                .map_err(|detail| AttestationStoreError::WriteFailed {
                    stage: "canonicalise-sealed",
                    path: path.to_string_lossy().to_string(),
                    detail,
                })?;
        Ok(StoredAttestation {
            digest,
            path,
            already_sealed: true,
            bytes,
        })
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
        refuse_state_dir_inside_workspace(&self.state_dir, &self.repo_root)
            .map_err(AttestationStoreError::Refused)?;

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

        let path = self.record_path(&attestation.execution_id().to_string())?;
        let dir = path
            .parent()
            .map(std::path::Path::to_path_buf)
            .unwrap_or_else(|| self.state_dir.clone());
        // sink: receipt:attestation-file
        std::fs::create_dir_all(&dir).map_err(|e| AttestationStoreError::WriteFailed {
            stage: "create",
            path: dir.to_string_lossy().to_string(),
            detail: e.to_string(),
        })?;

        // A seal is authoritative (ADR-024 §6, rule 3): read what is there before replacing it.
        //
        // The comparison is made by the module's own reader — `verify_attestation` over the sealed
        // bytes — rather than by trusting a name or a timestamp, so the digest being compared is the
        // one the record actually has. The same body re-signed is the same record, and writing it
        // again would be a copy of the same fact: it is accepted and reported as already sealed.
        if path.exists() {
            let sealed_bytes =
                std::fs::read(&path).map_err(|e| AttestationStoreError::WriteFailed {
                    stage: "read",
                    path: path.to_string_lossy().to_string(),
                    detail: e.to_string(),
                })?;
            let key = self.signing_key.as_deref().unwrap_or_default();
            let sealed = crate::receipt::verify_attestation(&sealed_bytes, key.as_bytes())
                .map_err(|detail| {
                    AttestationStoreError::Refused(
                        crate::receipt::ReceiptRefusal::MalformedDocument {
                            detail: format!(
                                "the sealed attestation at {} does not verify: {detail}",
                                path.display()
                            ),
                        },
                    )
                })?;
            let sealed_digest = sealed.attestation().digest().map_err(|detail| {
                AttestationStoreError::WriteFailed {
                    stage: "canonicalise-sealed",
                    path: path.to_string_lossy().to_string(),
                    detail,
                }
            })?;
            if sealed_digest != digest {
                return Err(AttestationStoreError::Refused(
                    crate::receipt::ReceiptRefusal::SealedDigestChanged {
                        execution_id: attestation.execution_id().to_string(),
                        sealed: sealed_digest,
                        presented: digest,
                    },
                ));
            }
            return Ok(StoredAttestation {
                digest,
                path,
                already_sealed: true,
                bytes: sealed_bytes,
            });
        }

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

        Ok(StoredAttestation {
            digest,
            path,
            already_sealed: false,
            bytes: body,
        })
    }
}
