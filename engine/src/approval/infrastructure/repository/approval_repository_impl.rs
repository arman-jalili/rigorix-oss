//! ApprovalRepository implementations.
//!
//! @canonical .pi/architecture/modules/approval.md#infrastructure
//! Implements: ApprovalRepository — in-memory and file-backed stores
//! Issue: #792 (approval epic — approvalservice implementation)
//!
//! - `InMemoryApprovalRepository` — tests, ephemeral runs; the degradation
//!   path for unavailable state persistence (records held in memory, warning
//!   logged by the caller)
//! - `FileBackedApprovalRepository` — durable JSON store for cross-process
//!   resume: instance A persists, instance B hydrates the same file and can
//!   verify the approval against its own re-derived intents

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use uuid::Uuid;

use crate::approval::domain::{ApprovalError, ApprovalRecord};

/// In-memory approval store (node-scoped, single record per node).
#[derive(Default)]
pub struct InMemoryApprovalRepository {
    records: RwLock<HashMap<Uuid, ApprovalRecord>>,
    mac_key: Option<Vec<u8>>,
}

impl InMemoryApprovalRepository {
    /// Create an empty in-memory store without record authentication.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create an in-memory store that seals and verifies records (#1237).
    pub fn with_key(run_key: &[u8]) -> Self {
        Self {
            records: RwLock::new(HashMap::new()),
            mac_key: Some(run_key.to_vec()),
        }
    }
}

fn seal_if_keyed(
    record: &ApprovalRecord,
    mac_key: Option<&[u8]>,
) -> Result<ApprovalRecord, ApprovalError> {
    let mut sealed = record.clone();
    if let Some(key) = mac_key {
        sealed.seal(key)?;
    }
    Ok(sealed)
}

fn verify_if_keyed(
    record: ApprovalRecord,
    mac_key: Option<&[u8]>,
) -> Result<ApprovalRecord, ApprovalError> {
    if let Some(key) = mac_key
        && !record.verify_integrity(key)
    {
        return Err(ApprovalError::RecordIntegrity {
            node_id: record.node_id,
            step_name: record.step_name,
        });
    }
    Ok(record)
}

#[async_trait::async_trait]
impl crate::approval::infrastructure::repository::ApprovalRepository
    for InMemoryApprovalRepository
{
    async fn save(&self, record: &ApprovalRecord) -> Result<(), ApprovalError> {
        let mut map = self
            .records
            .write()
            .map_err(|_| ApprovalError::Internal("in-memory repo poisoned".into()))?;
        let sealed = seal_if_keyed(record, self.mac_key.as_deref())?;
        map.insert(record.node_id, sealed);
        Ok(())
    }

    async fn load(&self, node_id: Uuid) -> Result<Option<ApprovalRecord>, ApprovalError> {
        let map = self
            .records
            .read()
            .map_err(|_| ApprovalError::Internal("in-memory repo poisoned".into()))?;
        let record = map.get(&node_id).cloned();
        record
            .map(|record| verify_if_keyed(record, self.mac_key.as_deref()))
            .transpose()
    }

    async fn verify_integrity(&self) -> Result<usize, ApprovalError> {
        let Some(key) = self.mac_key.as_deref() else {
            return Ok(0);
        };
        let map = self
            .records
            .read()
            .map_err(|_| ApprovalError::Internal("in-memory repo poisoned".into()))?;
        for record in map.values() {
            if !record.verify_integrity(key) {
                return Err(ApprovalError::RecordIntegrity {
                    node_id: record.node_id,
                    step_name: record.step_name.clone(),
                });
            }
        }
        Ok(map.len())
    }

    async fn delete(&self, node_id: Uuid) -> Result<(), ApprovalError> {
        let mut map = self
            .records
            .write()
            .map_err(|_| ApprovalError::Internal("in-memory repo poisoned".into()))?;
        map.remove(&node_id);
        Ok(())
    }
}

/// File-backed approval store — durable across processes.
///
/// The whole node-scoped map is persisted as JSON on every mutation. Writes
/// are small (approvals are rare), so full-file rewrite is acceptable; the
/// write is atomic (temp file + rename) to survive interruption.
pub struct FileBackedApprovalRepository {
    path: PathBuf,
    records: RwLock<HashMap<Uuid, ApprovalRecord>>,
    mac_key: Option<Vec<u8>>,
}

impl FileBackedApprovalRepository {
    /// Open (creating if needed) the store at `path` without record
    /// authentication (legacy/tests).
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ApprovalError> {
        Self::open_with_key(path, None)
    }

    /// Open the store and authenticate every record with `run_key` (#1237).
    pub fn open_with_key(
        path: impl AsRef<Path>,
        run_key: Option<&[u8]>,
    ) -> Result<Self, ApprovalError> {
        let path = path.as_ref().to_path_buf();
        let records = match std::fs::read_to_string(&path) {
            Ok(contents) if !contents.trim().is_empty() => serde_json::from_str::<
                HashMap<Uuid, ApprovalRecord>,
            >(&contents)
            .map_err(|e| {
                ApprovalError::Internal(format!("corrupt approval store {}: {e}", path.display()))
            })?,
            Ok(_) | Err(_) => HashMap::new(),
        };
        Ok(Self {
            path,
            records: RwLock::new(records),
            mac_key: run_key.map(<[u8]>::to_vec),
        })
    }

    fn persist(&self) -> Result<(), ApprovalError> {
        let map = self
            .records
            .read()
            .map_err(|_| ApprovalError::Internal("file repo poisoned".into()))?;
        let contents = serde_json::to_string_pretty(&*map)
            .map_err(|e| ApprovalError::Internal(format!("serialize store: {e}")))?;
        let tmp = self.path.with_extension("json.tmp");
        std::fs::write(&tmp, contents)
            .map_err(|e| ApprovalError::Internal(format!("write store: {e}")))?;
        std::fs::rename(&tmp, &self.path)
            .map_err(|e| ApprovalError::Internal(format!("commit store: {e}")))?;
        Ok(())
    }
}

#[async_trait::async_trait]
impl crate::approval::infrastructure::repository::ApprovalRepository
    for FileBackedApprovalRepository
{
    async fn save(&self, record: &ApprovalRecord) -> Result<(), ApprovalError> {
        let mut map = self
            .records
            .write()
            .map_err(|_| ApprovalError::Internal("file repo poisoned".into()))?;
        let sealed = seal_if_keyed(record, self.mac_key.as_deref())?;
        map.insert(record.node_id, sealed);
        drop(map);
        self.persist()
    }

    async fn load(&self, node_id: Uuid) -> Result<Option<ApprovalRecord>, ApprovalError> {
        let map = self
            .records
            .read()
            .map_err(|_| ApprovalError::Internal("file repo poisoned".into()))?;
        let record = map.get(&node_id).cloned();
        record
            .map(|record| verify_if_keyed(record, self.mac_key.as_deref()))
            .transpose()
    }

    async fn verify_integrity(&self) -> Result<usize, ApprovalError> {
        let Some(key) = self.mac_key.as_deref() else {
            return Ok(0);
        };
        let map = self
            .records
            .read()
            .map_err(|_| ApprovalError::Internal("file repo poisoned".into()))?;
        for record in map.values() {
            if !record.verify_integrity(key) {
                return Err(ApprovalError::RecordIntegrity {
                    node_id: record.node_id,
                    step_name: record.step_name.clone(),
                });
            }
        }
        Ok(map.len())
    }

    async fn delete(&self, node_id: Uuid) -> Result<(), ApprovalError> {
        let mut map = self
            .records
            .write()
            .map_err(|_| ApprovalError::Internal("file repo poisoned".into()))?;
        map.remove(&node_id);
        drop(map);
        self.persist()
    }
}
