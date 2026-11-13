//! The settlement record, in the filesystem state repository — atomic tmp + rename, as every durable
//! record in this module is (#1528, ADR-024 §7).
//!
//! @canonical .pi/architecture/modules/state-persistence.md#infrastructure
//! Implements: `SettlementStore` over the same directory the state repository already owns
//!
//! The location is ADR-024 §6's: the state directory the host roots outside the workspace. A
//! settlement written inside the agent's write surface would be a record the agent can rewrite, and
//! "the run ended ambiguously" is exactly the sentence an interested party would like to change.

use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

use crate::effect_commitment::application::settlement_service::SettlementStore;
use crate::effect_commitment::domain::settlement::ExecutionSettlement;
use crate::state_persistence::domain::StateError;

/// Settlements as `{dir}/{execution_id}.settlement.json`.
pub struct FileSystemSettlementStore {
    dir: PathBuf,
}

impl FileSystemSettlementStore {
    /// Root the store at the state directory.
    ///
    /// # Errors
    /// `StateError::DirectoryError` when the directory cannot be created or is not a directory —
    /// the same refusals the other repositories in this module make, so a caller sees one behaviour.
    pub async fn new(dir: impl Into<PathBuf>) -> Result<Self, StateError> {
        let dir: PathBuf = dir.into();
        if !dir.exists() {
            // sink: state-persistence:settlements
            fs::create_dir_all(&dir)
                .await
                .map_err(|e| StateError::DirectoryError {
                    detail: format!("Failed to create settlement directory {dir:?}: {e}"),
                })?;
        }
        if !dir.is_dir() {
            return Err(StateError::DirectoryError {
                detail: format!("Settlement path {dir:?} exists but is not a directory"),
            });
        }
        Ok(Self { dir })
    }

    fn path(&self, execution_id: Uuid) -> PathBuf {
        self.dir.join(format!("{execution_id}.settlement.json"))
    }
}

#[async_trait]
impl SettlementStore for FileSystemSettlementStore {
    async fn put(&self, settlement: &ExecutionSettlement) -> Result<(), StateError> {
        let path = self.path(settlement.execution_id);
        let tmp = path.with_extension("json.tmp");
        let bytes =
            serde_json::to_vec_pretty(settlement).map_err(|e| StateError::SerialisationError {
                detail: format!(
                    "Failed to serialize settlement {:?}: {e}",
                    settlement.execution_id
                ),
            })?;
        // sink: state-persistence:settlement-tmp
        fs::write(&tmp, &bytes)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to write settlement tmp {tmp:?}: {e}"),
            })?;
        // sink: state-persistence:settlements
        fs::rename(&tmp, &path)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to rename settlement {tmp:?} -> {path:?}: {e}"),
            })?;
        Ok(())
    }

    async fn get(&self, execution_id: Uuid) -> Result<ExecutionSettlement, StateError> {
        let path = self.path(execution_id);
        let bytes = match fs::read(&path).await {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Err(StateError::StateNotFound {
                    execution_id: execution_id.to_string(),
                });
            }
            Err(e) => {
                return Err(StateError::IoError {
                    detail: format!("Failed to read settlement {path:?}: {e}"),
                });
            }
        };
        serde_json::from_slice(&bytes).map_err(|e| StateError::CorruptedState {
            path: path.to_string_lossy().to_string(),
            detail: format!("Settlement {path:?} is not readable: {e}"),
        })
    }

    async fn list(&self) -> Result<Vec<ExecutionSettlement>, StateError> {
        let mut out = Vec::new();
        let mut entries = fs::read_dir(&self.dir)
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to list settlements in {:?}: {e}", self.dir),
            })?;
        while let Some(entry) = entries
            .next_entry()
            .await
            .map_err(|e| StateError::IoError {
                detail: format!("Failed to read settlement directory entry: {e}"),
            })?
        {
            let path = entry.path();
            if path.extension().is_some_and(|e| e == "json")
                && path
                    .file_name()
                    .is_some_and(|n| n.to_string_lossy().ends_with(".settlement.json"))
            {
                let bytes = fs::read(&path).await.map_err(|e| StateError::IoError {
                    detail: format!("Failed to read settlement {path:?}: {e}"),
                })?;
                let settlement: ExecutionSettlement =
                    serde_json::from_slice(&bytes).map_err(|e| StateError::CorruptedState {
                        path: path.to_string_lossy().to_string(),
                        detail: format!("Settlement {path:?} is not readable: {e}"),
                    })?;
                out.push(settlement);
            }
        }
        Ok(out)
    }
}
