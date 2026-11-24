//! The sealed receipt: the declaration, written once, outside the workspace (ADR-024 §6 rule 2).
//!
//! @canonical .pi/architecture/modules/receipt.md#store
//! Implements: ADR-024 §6 rule 2 — the sealed copy lives where the executing agent cannot write it
//! Issue: #1570
//!
//! # Why the bytes and not the digest
//!
//! P2 verified the receipt in memory and carried its digest into the envelope. That digest named
//! bytes which existed only in the caller's request: a later reader could confirm a digest but not
//! recover what it was a digest *of*, so "the run was admitted under this declaration" was a claim
//! about a document nobody had kept. This store keeps the document.
//!
//! # Where
//!
//! `<state dir>/receipts/<execution_id>.json`, beside `attestations/` and for the same reason
//! (ADR-024 §6): `default_state_dir` resolves outside every workspace, and a state dir that resolves
//! inside one is refused on the resolved paths before anything is written.
//!
//! The file is the receipt's **canonical JSON** — the bytes the digest is taken over — so a reader
//! recovers the exact value that was sealed rather than a re-serialisation that happens to parse.
//!
//! # The write is read-before-write
//!
//! The seal is authoritative (rule 3, which P4 gave the attestation store): the same canonical body
//! is already sealed and nothing is written; a different body is `SealedDigestChanged`; a document
//! that no longer parses is `MalformedDocument` rather than an invitation to overwrite it.

use std::path::{Path, PathBuf};

use crate::receipt::domain::digest::canonical_json;
use crate::receipt::domain::receipt::Receipt;
use crate::receipt::{ReceiptRefusal, refuse_state_dir_inside_workspace};

/// Why a receipt could not be sealed or read back.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReceiptStoreError {
    /// A rule of the receipt vocabulary refused the record: the seal's location, a re-seal with a
    /// different digest, or a document that no longer parses. One vocabulary, so a caller branches on
    /// what it already reads.
    Refused(ReceiptRefusal),
    /// There is no sealed receipt for this execution.
    NotFound {
        /// The execution asked for.
        execution_id: String,
        /// Where the store looked.
        path: String,
    },
    /// The record could not be written or read. `stage` names which half — `create`, `read`, `write`,
    /// `rename` or `canonicalise` — because "persistence failed" without the stage is not actionable.
    Io {
        /// Which half failed.
        stage: &'static str,
        /// The path involved.
        path: String,
        /// The operating system's own words.
        detail: String,
    },
}

impl std::fmt::Display for ReceiptStoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Refused(refusal) => write!(f, "{refusal}"),
            Self::NotFound { execution_id, path } => write!(
                f,
                "no sealed receipt for execution {execution_id} at {path}"
            ),
            Self::Io {
                stage,
                path,
                detail,
            } => write!(f, "the receipt store failed at {stage} ({path}): {detail}"),
        }
    }
}

impl std::error::Error for ReceiptStoreError {}

/// What a successful seal returns.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredReceipt {
    /// sha256 over the receipt's canonical bytes — the same string the envelope carries.
    pub digest: String,
    /// The file the declaration lives in.
    pub path: PathBuf,
    /// Whether the record was already there with this digest, in which case nothing was written.
    pub already_sealed: bool,
}

/// The file-backed receipt store.
#[derive(Debug, Clone)]
pub struct FileReceiptStore {
    state_dir: PathBuf,
    repo_root: PathBuf,
}

impl FileReceiptStore {
    /// A store over `state_dir`, for `repo_root`.
    #[must_use]
    pub fn new(state_dir: impl Into<PathBuf>, repo_root: impl Into<PathBuf>) -> Self {
        Self {
            state_dir: state_dir.into(),
            repo_root: repo_root.into(),
        }
    }

    /// Where this store writes.
    #[must_use]
    pub fn state_dir(&self) -> &Path {
        &self.state_dir
    }

    fn path(&self, execution_id: &str) -> PathBuf {
        self.state_dir
            .join("receipts")
            .join(format!("{execution_id}.json"))
    }

    /// Seal the declaration, or confirm the one already sealed.
    ///
    /// `digest` is the digest `verify` computed over this receipt; it is compared rather than
    /// recomputed so the store writes under the identity the run was admitted with.
    ///
    /// # Errors
    /// `StateDirInsideWorkspace`, `SealedDigestChanged` or `MalformedDocument` as refusals; `Io`
    /// naming the stage that failed.
    pub fn put(&self, receipt: &Receipt, digest: &str) -> Result<StoredReceipt, ReceiptStoreError> {
        refuse_state_dir_inside_workspace(&self.state_dir, &self.repo_root)
            .map_err(ReceiptStoreError::Refused)?;

        let execution_id = receipt.execution_id.to_string();
        let path = self.path(&execution_id);

        // read-before-write: the seal is authoritative.
        if path.exists() {
            let sealed = self.read(&path)?;
            if sealed.1 != digest {
                return Err(ReceiptStoreError::Refused(
                    ReceiptRefusal::SealedDigestChanged {
                        execution_id,
                        sealed: sealed.1,
                        presented: digest.to_string(),
                    },
                ));
            }
            return Ok(StoredReceipt {
                digest: sealed.1,
                path,
                already_sealed: true,
            });
        }

        let body = canonical_json(&receipt.clone()).map_err(|detail| ReceiptStoreError::Io {
            stage: "canonicalise",
            path: path.to_string_lossy().to_string(),
            detail,
        })?;

        let dir = self.state_dir.join("receipts");
        // sink: receipt:receipt-file
        crate::common::private_fs::create_dir_all(&dir).map_err(|e| ReceiptStoreError::Io {
            stage: "create",
            path: dir.to_string_lossy().to_string(),
            detail: e.to_string(),
        })?;
        let tmp = dir.join(format!(".{execution_id}.{}.tmp", &digest[..16]));
        // sink: receipt:receipt-file
        // #1658 item 2: the temp file is created owner-only and *is* what the rename makes the
        // destination. Measured before the change: `std::fs::write` under `umask 022` gives 0644, so
        // every receipt was readable by every account on the host.
        {
            use std::io::Write;
            let mut file = crate::common::private_fs::create_private(&tmp).map_err(|e| {
                ReceiptStoreError::Io {
                    stage: "write",
                    path: tmp.to_string_lossy().to_string(),
                    detail: e.to_string(),
                }
            })?;
            file.write_all(&body)
                .and_then(|()| file.sync_all())
                .map_err(|e| ReceiptStoreError::Io {
                    stage: "write",
                    path: tmp.to_string_lossy().to_string(),
                    detail: e.to_string(),
                })?;
        }
        // sink: receipt:receipt-file
        std::fs::rename(&tmp, &path).map_err(|e| ReceiptStoreError::Io {
            stage: "rename",
            path: path.to_string_lossy().to_string(),
            detail: e.to_string(),
        })?;

        Ok(StoredReceipt {
            digest: digest.to_string(),
            path,
            already_sealed: false,
        })
    }

    /// Read the sealed declaration back, with the digest its bytes have now.
    ///
    /// # Errors
    /// `NotFound` when nothing is sealed; `MalformedDocument` when the file no longer parses; `Io`
    /// when it cannot be read.
    pub fn get(&self, execution_id: uuid::Uuid) -> Result<(Receipt, String), ReceiptStoreError> {
        let id = execution_id.to_string();
        let path = self.path(&id);
        if !path.exists() {
            return Err(ReceiptStoreError::NotFound {
                execution_id: id,
                path: path.to_string_lossy().to_string(),
            });
        }
        let (receipt, digest) = self.read(&path)?;
        Ok((receipt, digest))
    }

    /// Parse the file and recompute its digest from the bytes on disk.
    fn read(&self, path: &Path) -> Result<(Receipt, String), ReceiptStoreError> {
        // sink: receipt:receipt-file
        let bytes = std::fs::read(path).map_err(|e| ReceiptStoreError::Io {
            stage: "read",
            path: path.to_string_lossy().to_string(),
            detail: e.to_string(),
        })?;
        let text = String::from_utf8(bytes).map_err(|e| ReceiptStoreError::Io {
            stage: "read",
            path: path.to_string_lossy().to_string(),
            detail: e.to_string(),
        })?;
        // `from_document`, not `serde_json::from_str`: the closed vocabularies (`schema_version`,
        // `workspace.kind`) must be refused in this module's words, which is the door that does it.
        let receipt = Receipt::from_document(&text).map_err(|refusals| {
            ReceiptStoreError::Refused(ReceiptRefusal::MalformedDocument {
                detail: format!(
                    "the sealed receipt at {} does not read back: {}",
                    path.display(),
                    refusals
                        .iter()
                        .map(std::string::ToString::to_string)
                        .collect::<Vec<_>>()
                        .join("; ")
                ),
            })
        })?;
        let digest = receipt.digest().map_err(|detail| ReceiptStoreError::Io {
            stage: "canonicalise-sealed",
            path: path.to_string_lossy().to_string(),
            detail,
        })?;
        Ok((receipt, digest.as_str().to_string()))
    }
}
