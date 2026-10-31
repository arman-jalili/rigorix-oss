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

use std::collections::{BTreeMap, HashMap};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::approval::domain::{ApprovalError, ApprovalRecord, ApprovalStatus};
use crate::approval::infrastructure::repository::LedgerMigration;

/// One recorded terminal transition, kept outside the store file (#1241).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConsumptionMarker {
    node_id: Uuid,
    nonce: Uuid,
    status: ApprovalStatus,
    recorded_at: DateTime<Utc>,
}

fn marker_key(node_id: Uuid, nonce: Uuid) -> String {
    format!("{node_id}:{nonce}")
}

/// The ledger file. `mac` covers `version` + the whole `entries` map, so
/// removing one marker changes the authenticated bytes (#1244). A per-entry
/// MAC would be removed with its entry; the whole-set MAC is the point.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConsumptionLedgerFile {
    version: u8,
    entries: BTreeMap<String, ConsumptionMarker>,
    mac: String,
}

const CONSUMPTION_LEDGER_VERSION: u8 = 1;

fn ledger_mac(
    entries: &BTreeMap<String, ConsumptionMarker>,
    run_key: &[u8],
) -> Result<String, ApprovalError> {
    let bytes = serde_json::to_vec(&(CONSUMPTION_LEDGER_VERSION, entries))
        .map_err(|e| ApprovalError::Internal(format!("serialize consumption ledger MAC: {e}")))?;
    let mut mac = Hmac::<Sha256>::new_from_slice(run_key)
        .map_err(|e| ApprovalError::Internal(format!("consumption ledger MAC key: {e}")))?;
    mac.update(&bytes);
    Ok(hex::encode(mac.finalize().into_bytes()))
}

fn ledger_integrity(store: &Path, ledger: &Path, detail: impl Into<String>) -> ApprovalError {
    ApprovalError::ConsumptionLedgerIntegrity {
        store: store.display().to_string(),
        ledger: ledger.display().to_string(),
        detail: detail.into(),
    }
}

fn write_consumption_ledger(
    ledger: &Path,
    entries: &BTreeMap<String, ConsumptionMarker>,
    run_key: Option<&[u8]>,
) -> Result<(), ApprovalError> {
    if let Some(parent) = ledger.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let mac = match run_key {
        Some(key) => ledger_mac(entries, key)?,
        None => String::new(),
    };
    let contents = serde_json::to_string_pretty(&ConsumptionLedgerFile {
        version: CONSUMPTION_LEDGER_VERSION,
        entries: entries.clone(),
        mac,
    })
    .map_err(|e| ApprovalError::Internal(format!("serialize consumption ledger: {e}")))?;
    // Unique temp name: the ledger may be shared across processes, and a
    // fixed `.json.tmp` makes concurrent writers rename a file another writer
    // already moved.
    let tmp = ledger.with_file_name(format!(
        ".{}.{}.tmp",
        ledger
            .file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default(),
        Uuid::new_v4()
    ));
    std::fs::write(&tmp, contents).map_err(|e| {
        ApprovalError::Internal(format!(
            "write approval consumption ledger {}: {e}",
            tmp.display()
        ))
    })?;
    std::fs::rename(&tmp, ledger).map_err(|e| {
        ApprovalError::Internal(format!(
            "commit approval consumption ledger {}: {e}",
            ledger.display()
        ))
    })?;
    Ok(())
}

/// Load and verify the ledger. A file without the sealed wrapper is refused:
/// the wrapper is the only evidence that the markers were ever authenticated,
/// so accepting its absence would let an attacker choose the unprotected form
/// (#1247).
fn read_consumption_ledger(
    store: &Path,
    ledger: &Path,
    run_key: Option<&[u8]>,
) -> Result<BTreeMap<String, ConsumptionMarker>, ApprovalError> {
    let raw = std::fs::read_to_string(ledger)
        .map_err(|e| ledger_integrity(store, ledger, format!("read failed: {e}")))?;
    if raw.trim().is_empty() {
        return Err(ledger_integrity(
            store,
            ledger,
            "file is empty — an absent first-run ledger must be deleted, not truncated",
        ));
    }
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| ledger_integrity(store, ledger, format!("corrupt JSON: {e}")))?;
    if value.get("entries").is_some() {
        let file: ConsumptionLedgerFile = serde_json::from_value(value)
            .map_err(|e| ledger_integrity(store, ledger, format!("corrupt structure: {e}")))?;
        if file.version != CONSUMPTION_LEDGER_VERSION {
            return Err(ledger_integrity(
                store,
                ledger,
                format!("unsupported ledger version {}", file.version),
            ));
        }
        if let Some(key) = run_key {
            let expected = ledger_mac(&file.entries, key)?;
            if !constant_time_eq(expected.as_bytes(), file.mac.as_bytes()) {
                return Err(ledger_integrity(
                    store,
                    ledger,
                    "MAC mismatch — markers were removed or altered",
                ));
            }
        }
        Ok(file.entries)
    } else {
        Err(ledger_integrity(
            store,
            ledger,
            "unauthenticated ledger: no version/entries/mac wrapper; run \
             `rigorix approve ledger migrate` to adopt it explicitly",
        ))
    }
}

/// Explicit migration for a legacy bare map. Never called by the loader.
fn migrate_consumption_ledger_at(
    store: &Path,
    ledger: &Path,
    run_key: Option<&[u8]>,
) -> Result<LedgerMigration, ApprovalError> {
    let Some(key) = run_key else {
        return Err(ledger_integrity(
            store,
            ledger,
            "migration requires the run key; set RIGORIX_HMAC_KEY",
        ));
    };
    let raw = std::fs::read_to_string(ledger)
        .map_err(|e| ledger_integrity(store, ledger, format!("read failed: {e}")))?;
    if raw.trim().is_empty() {
        return Err(ledger_integrity(
            store,
            ledger,
            "file is empty — nothing to migrate",
        ));
    }
    let value: serde_json::Value = serde_json::from_str(&raw)
        .map_err(|e| ledger_integrity(store, ledger, format!("corrupt JSON: {e}")))?;
    if value.get("entries").is_some() {
        let file: ConsumptionLedgerFile = serde_json::from_value(value)
            .map_err(|e| ledger_integrity(store, ledger, format!("corrupt structure: {e}")))?;
        let expected = ledger_mac(&file.entries, key)?;
        if !constant_time_eq(expected.as_bytes(), file.mac.as_bytes()) {
            return Err(ledger_integrity(
                store,
                ledger,
                "MAC mismatch — the ledger is already sealed and was altered",
            ));
        }
        return Ok(LedgerMigration::AlreadySealed {
            markers: file.entries.len(),
        });
    }
    let entries: BTreeMap<String, ConsumptionMarker> = serde_json::from_value(value)
        .map_err(|e| ledger_integrity(store, ledger, format!("corrupt legacy map: {e}")))?;
    write_consumption_ledger(ledger, &entries, Some(key))?;
    Ok(LedgerMigration::Migrated {
        markers: entries.len(),
    })
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b).fold(0u8, |acc, (a, b)| acc | (a ^ b)) == 0
}

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
    /// Consumption markers live outside the store file, so restoring an older
    /// copy of the store does not restore an older copy of its progress.
    consumption_path: Option<PathBuf>,
    consumed: RwLock<BTreeMap<String, ConsumptionMarker>>,
}

impl FileBackedApprovalRepository {
    /// Explicitly migrate a legacy bare-map ledger without opening the store.
    ///
    /// The loader refuses a bare map (#1247); this is the only path that
    /// adopts one.
    pub fn migrate_consumption_ledger(
        store: impl AsRef<Path>,
        ledger: impl AsRef<Path>,
        run_key: &[u8],
    ) -> Result<LedgerMigration, ApprovalError> {
        migrate_consumption_ledger_at(store.as_ref(), ledger.as_ref(), Some(run_key))
    }

    /// Open (creating if needed) the store at `path` without record
    /// authentication (legacy/tests).
    pub fn open(path: impl AsRef<Path>) -> Result<Self, ApprovalError> {
        Self::open_with_key_and_ledger(path, None, None)
    }

    /// Open the store and authenticate every record with `run_key` (#1237).
    pub fn open_with_key(
        path: impl AsRef<Path>,
        run_key: Option<&[u8]>,
    ) -> Result<Self, ApprovalError> {
        Self::open_with_key_and_ledger(path, run_key, None)
    }

    /// Open the store with authentication and an anti-rollback consumption
    /// ledger (#1241). The ledger is a separate file; when it is absent, the
    /// store works and reports that it has no marker.
    pub fn open_with_key_and_ledger(
        path: impl AsRef<Path>,
        run_key: Option<&[u8]>,
        consumption_path: Option<&Path>,
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
        let consumed = match consumption_path {
            Some(ledger) if ledger.is_file() => read_consumption_ledger(&path, ledger, run_key)?,
            _ => BTreeMap::new(),
        };
        Ok(Self {
            path,
            records: RwLock::new(records),
            mac_key: run_key.map(<[u8]>::to_vec),
            consumption_path: consumption_path.map(Path::to_path_buf),
            consumed: RwLock::new(consumed),
        })
    }

    fn reject_rolled_back(&self, record: &ApprovalRecord) -> Result<(), ApprovalError> {
        if !matches!(record.status, ApprovalStatus::Pending) {
            return Ok(());
        }
        let consumed = self
            .consumed
            .read()
            .map_err(|_| ApprovalError::Internal("consumption ledger poisoned".into()))?;
        if consumed.contains_key(&marker_key(record.node_id, record.nonce)) {
            return Err(ApprovalError::RecordRollback {
                node_id: record.node_id,
                step_name: record.step_name.clone(),
            });
        }
        Ok(())
    }

    fn record_consumption(&self, record: &ApprovalRecord) -> Result<(), ApprovalError> {
        let Some(path) = self.consumption_path.as_ref() else {
            return Ok(());
        };
        if matches!(record.status, ApprovalStatus::Pending) {
            return Ok(());
        }
        let mut consumed = self
            .consumed
            .write()
            .map_err(|_| ApprovalError::Internal("consumption ledger poisoned".into()))?;
        consumed.insert(
            marker_key(record.node_id, record.nonce),
            ConsumptionMarker {
                node_id: record.node_id,
                nonce: record.nonce,
                status: record.status,
                recorded_at: Utc::now(),
            },
        );
        write_consumption_ledger(path, &consumed, self.mac_key.as_deref())
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
        self.reject_rolled_back(record)?;
        let sealed = seal_if_keyed(record, self.mac_key.as_deref())?;
        self.record_consumption(&sealed)?;
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
        let record = record
            .map(|record| verify_if_keyed(record, self.mac_key.as_deref()))
            .transpose()?;
        if let Some(record) = record.as_ref() {
            self.reject_rolled_back(record)?;
        }
        Ok(record)
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
            self.reject_rolled_back(record)?;
        }
        Ok(map.len())
    }

    async fn consumption_marker_count(&self) -> Result<usize, ApprovalError> {
        if self.consumption_path.is_none() {
            return Ok(0);
        }
        let consumed = self
            .consumed
            .read()
            .map_err(|_| ApprovalError::Internal("consumption ledger poisoned".into()))?;
        Ok(consumed.len())
    }

    async fn migrate_consumption_ledger(&self) -> Result<LedgerMigration, ApprovalError> {
        let ledger = self.consumption_path.as_ref().ok_or_else(|| {
            ApprovalError::Internal("no consumption ledger path is configured".into())
        })?;
        migrate_consumption_ledger_at(&self.path, ledger, self.mac_key.as_deref())
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
