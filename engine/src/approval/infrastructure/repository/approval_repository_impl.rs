//! ApprovalRepository implementations.
//!
//! @canonical .pi/architecture/modules/approval.md#infrastructure
//! Canonical Reference: .pi/architecture/modules/approval.md#infrastructure
//! Implements: ApprovalRepository — in-memory and file-backed stores
//! Issue: #792 (approval epic — approvalservice implementation)
//!
//! - `InMemoryApprovalRepository` — tests, ephemeral runs; the degradation
//!   path for unavailable state persistence (records held in memory, warning
//!   logged by the caller)
//! - `FileBackedApprovalRepository` — durable JSON store for cross-process
//!   resume: instance A persists, instance B hydrates the same file and can
//!   verify the approval against its own re-derived intents

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::RwLock;

use chrono::{DateTime, Utc};
use hmac::{Hmac, KeyInit, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use uuid::Uuid;

use crate::approval::domain::{
    ApprovalError, ApprovalRecord, ApprovalStatus, ConsumptionSettlement,
};
use crate::approval::infrastructure::repository::LedgerMigration;

/// One recorded terminal transition, kept outside the store file (#1241).
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ConsumptionMarker {
    node_id: Uuid,
    nonce: Uuid,
    status: ApprovalStatus,
    recorded_at: DateTime<Utc>,
    /// The run that consumed it. Omitted on legacy markers; omission keeps
    /// their canonical MAC bytes unchanged (#1267).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    dag_id: Option<Uuid>,
    /// The 1-indexed attempt that settled. Omitted on legacy markers.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    attempt: Option<u32>,
}

fn marker_key(node_id: Uuid, nonce: Uuid) -> String {
    format!("{node_id}:{nonce}")
}

fn lock_path(path: &Path) -> PathBuf {
    path.with_file_name(format!(
        ".{}.lock",
        path.file_name()
            .map(|name| name.to_string_lossy())
            .unwrap_or_default()
    ))
}

/// Serialise access across processes (#1254, #1261).
///
/// Writes take an exclusive lock for the whole read-modify-write; reads take a
/// shared lock while they re-read, so a live instance never serves its open-time
/// snapshot. The lock is a sibling file because the store and ledger may not
/// exist yet.
fn with_file_lock<T>(
    path: &Path,
    exclusive: bool,
    action: impl FnOnce() -> Result<T, ApprovalError>,
) -> Result<T, ApprovalError> {
    use std::os::unix::io::AsRawFd;
    let lock = lock_path(path);
    if let Some(parent) = lock.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .read(true)
        .write(true)
        .truncate(false)
        .open(&lock)
        .map_err(|e| {
            ApprovalError::Internal(format!("open approval lock {}: {e}", lock.display()))
        })?;
    let operation = if exclusive {
        libc::LOCK_EX
    } else {
        libc::LOCK_SH
    };
    loop {
        // SAFETY: `file` owns a valid descriptor for the duration of the call.
        let rc = unsafe { libc::flock(file.as_raw_fd(), operation) };
        if rc == 0 {
            break;
        }
        let error = std::io::Error::last_os_error();
        if error.kind() != std::io::ErrorKind::Interrupted {
            return Err(ApprovalError::Internal(format!(
                "lock approval state {}: {error}",
                lock.display()
            )));
        }
    }
    action()
}

fn with_exclusive_lock<T>(
    path: &Path,
    action: impl FnOnce() -> Result<T, ApprovalError>,
) -> Result<T, ApprovalError> {
    with_file_lock(path, true, action)
}

fn with_shared_lock<T>(
    path: &Path,
    action: impl FnOnce() -> Result<T, ApprovalError>,
) -> Result<T, ApprovalError> {
    with_file_lock(path, false, action)
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

/// Terminal records in the authenticated store that the ledger does not cover.
///
/// The store is the source of truth for which records reached a terminal
/// status; the ledger is supposed to remember every one. A bare map that names
/// fewer is a truncated map, not an exported one (#1249).
struct StoreComparison {
    store_present: bool,
    terminal_records: usize,
    missing: Vec<(Uuid, String)>,
    /// The same records with the fields a marker needs, so `--accept-missing`
    /// can adopt them into the ledger instead of leaving them uncovered.
    missing_markers: Vec<(String, ConsumptionMarker)>,
    /// Markers naming no record this store holds (#1253).
    unmatched_markers: usize,
}

fn unprotected_terminal_records(
    store: &Path,
    run_key: &[u8],
    entries: &BTreeMap<String, ConsumptionMarker>,
) -> Result<StoreComparison, ApprovalError> {
    let (store_present, records) = match std::fs::read_to_string(store) {
        Ok(contents) if !contents.trim().is_empty() => (
            true,
            serde_json::from_str::<HashMap<Uuid, ApprovalRecord>>(&contents).map_err(|e| {
                ApprovalError::Internal(format!("corrupt approval store {}: {e}", store.display()))
            })?,
        ),
        Ok(_) => (true, HashMap::new()),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => (false, HashMap::new()),
        Err(e) => {
            return Err(ApprovalError::Internal(format!(
                "read approval store {}: {e}",
                store.display()
            )));
        }
    };
    let record_keys: HashSet<String> = records
        .values()
        .map(|record| marker_key(record.node_id, record.nonce))
        .collect();
    let unmatched_markers = entries
        .keys()
        .filter(|key| !record_keys.contains(*key))
        .count();
    let mut terminal = 0usize;
    let mut missing = Vec::new();
    let mut missing_markers = Vec::new();
    for record in records.values() {
        let record = verify_if_keyed(record.clone(), Some(run_key))?;
        if matches!(
            record.status,
            ApprovalStatus::Consumed | ApprovalStatus::Expired | ApprovalStatus::Superseded
        ) {
            terminal += 1;
            let key = marker_key(record.node_id, record.nonce);
            if !entries.contains_key(&key) {
                missing.push((record.node_id, record.step_name.clone()));
                missing_markers.push((
                    key,
                    ConsumptionMarker {
                        node_id: record.node_id,
                        nonce: record.nonce,
                        status: record.status,
                        recorded_at: record.decided_at,
                        dag_id: None,
                        attempt: None,
                    },
                ));
            }
        }
    }
    Ok(StoreComparison {
        store_present,
        terminal_records: terminal,
        missing,
        missing_markers,
        unmatched_markers,
    })
}

/// Explicit migration for a legacy bare map. Never called by the loader.
fn migrate_consumption_ledger_at(
    store: &Path,
    ledger: &Path,
    run_key: Option<&[u8]>,
    accept_missing: bool,
) -> Result<LedgerMigration, ApprovalError> {
    // Absence and emptiness are no-ops, and neither needs a key: there is
    // nothing to authenticate. Damage below still requires one (#1256).
    let raw = match std::fs::read_to_string(ledger) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
            return Ok(LedgerMigration::NothingToMigrate {
                ledger_path: ledger.to_path_buf(),
                empty: false,
            });
        }
        Err(e) => {
            return Err(ledger_integrity(store, ledger, format!("read failed: {e}")));
        }
    };
    if raw.trim().is_empty() {
        return Ok(LedgerMigration::NothingToMigrate {
            ledger_path: ledger.to_path_buf(),
            empty: true,
        });
    }
    let Some(key) = run_key else {
        return Err(ledger_integrity(
            store,
            ledger,
            "migration requires the run key; set RIGORIX_HMAC_KEY",
        ));
    };
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
        let comparison = unprotected_terminal_records(store, key, &file.entries)?;
        if !accept_missing && !comparison.missing.is_empty() {
            return Err(migration_incomplete(store, ledger, &comparison.missing));
        }
        let mut entries = file.entries;
        if accept_missing && !comparison.missing_markers.is_empty() {
            entries.extend(comparison.missing_markers.iter().cloned());
            write_consumption_ledger(ledger, &entries, Some(key))?;
        }
        return Ok(LedgerMigration::AlreadySealed {
            markers: entries.len(),
            terminal_records: comparison.terminal_records,
            missing: comparison.missing,
            unmatched_markers: comparison.unmatched_markers,
            store_present: comparison.store_present,
            store_path: store.to_path_buf(),
        });
    }
    let mut entries: BTreeMap<String, ConsumptionMarker> = serde_json::from_value(value)
        .map_err(|e| ledger_integrity(store, ledger, format!("corrupt legacy map: {e}")))?;
    let comparison = unprotected_terminal_records(store, key, &entries)?;
    if !accept_missing && !comparison.missing.is_empty() {
        return Err(migration_incomplete(store, ledger, &comparison.missing));
    }
    if accept_missing {
        entries.extend(comparison.missing_markers.iter().cloned());
    }
    write_consumption_ledger(ledger, &entries, Some(key))?;
    Ok(LedgerMigration::Migrated {
        markers: entries.len(),
        terminal_records: comparison.terminal_records,
        missing: comparison.missing,
        unmatched_markers: comparison.unmatched_markers,
        store_present: comparison.store_present,
        store_path: store.to_path_buf(),
    })
}

fn migration_incomplete(store: &Path, ledger: &Path, missing: &[(Uuid, String)]) -> ApprovalError {
    ApprovalError::MigrationIncomplete {
        store: store.display().to_string(),
        ledger: ledger.display().to_string(),
        missing_count: missing.len(),
        missing: missing
            .iter()
            .map(|(node, step)| format!("{node} ({step})"))
            .collect::<Vec<_>>()
            .join(", "),
    }
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
/// The whole node-scoped map is persisted as JSON on every mutation, but the
/// in-memory map is not the source of truth: every write re-reads the file
/// under an exclusive lock before merging (#1254). A unique temp file per write
/// makes the publish atomic and collision-free.
pub struct FileBackedApprovalRepository {
    path: PathBuf,
    mac_key: Option<Vec<u8>>,
    /// Consumption markers live outside the store file, so restoring an older
    /// copy of the store does not restore an older copy of its progress.
    consumption_path: Option<PathBuf>,
    /// Serialises this instance's saves with its own writes; the file lock
    /// serialises across instances.
    write_lock: std::sync::Mutex<()>,
}

fn terminal_record_count(records: &HashMap<Uuid, ApprovalRecord>) -> usize {
    records
        .values()
        .filter(|record| {
            matches!(
                record.status,
                ApprovalStatus::Consumed | ApprovalStatus::Expired | ApprovalStatus::Superseded
            )
        })
        .count()
}

struct ReadSnapshot {
    records: HashMap<Uuid, ApprovalRecord>,
    consumed: BTreeMap<String, ConsumptionMarker>,
    ledger_present: bool,
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
        accept_missing: bool,
    ) -> Result<LedgerMigration, ApprovalError> {
        migrate_consumption_ledger_at(
            store.as_ref(),
            ledger.as_ref(),
            Some(run_key),
            accept_missing,
        )
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
        let ledger_present = consumption_path.is_some_and(Path::is_file);
        let consumed = match consumption_path {
            Some(ledger) if ledger_present => read_consumption_ledger(&path, ledger, run_key)?,
            _ => BTreeMap::new(),
        };
        // The open validates both files; reads re-read them under a shared
        // lock, because another instance may write after this returns (#1261).
        let _ = (records, consumed, ledger_present);
        Ok(Self {
            path,
            mac_key: run_key.map(<[u8]>::to_vec),
            consumption_path: consumption_path.map(Path::to_path_buf),
            write_lock: std::sync::Mutex::new(()),
        })
    }

    /// Read both files under shared locks. The store is locked first; writers
    /// take the ledger lock and the store lock separately, never nested, so
    /// this cannot deadlock (#1261).
    fn read_snapshot(&self) -> Result<ReadSnapshot, ApprovalError> {
        with_shared_lock(&self.path, || {
            let records = Self::read_store(&self.path)?;
            let (consumed, ledger_present) = match self.consumption_path.as_ref() {
                Some(path) => with_shared_lock(path, || {
                    if path.is_file() {
                        Ok((
                            read_consumption_ledger(&self.path, path, self.mac_key.as_deref())?,
                            true,
                        ))
                    } else {
                        Ok((BTreeMap::new(), false))
                    }
                })?,
                None => (BTreeMap::new(), false),
            };
            Ok(ReadSnapshot {
                records,
                consumed,
                ledger_present,
            })
        })
    }

    /// The ledger is checked in both directions (#1255). A pending record with
    /// a marker was restored after being spent; a terminal record without one
    /// means the marker was dropped, and the anti-rollback guard would have
    /// nothing to fire on. Without a ledger file there is no second witness, so
    /// only the pending direction is meaningful.
    fn reject_rolled_back(
        record: &ApprovalRecord,
        consumed: &BTreeMap<String, ConsumptionMarker>,
        ledger_present: bool,
        ledger_path: Option<&Path>,
        terminal_records: usize,
    ) -> Result<(), ApprovalError> {
        let pending = matches!(record.status, ApprovalStatus::Pending);
        let key = marker_key(record.node_id, record.nonce);
        if pending {
            if consumed.contains_key(&key) {
                return Err(ApprovalError::RecordRollback {
                    node_id: record.node_id,
                    step_name: record.step_name.clone(),
                });
            }
            return Ok(());
        }

        // A store with no ledger configured has no second witness; that is a
        // different state from a configured path whose file is missing. The
        // latter cannot prove nothing was spent, so it must refuse (#1264).
        let Some(path) = ledger_path else {
            return Ok(());
        };
        if !ledger_present {
            return Err(ApprovalError::ConsumptionLedgerMissing {
                ledger: path.display().to_string(),
                terminal_records,
                node_id: record.node_id,
                step_name: record.step_name.clone(),
            });
        }
        if !consumed.contains_key(&key) {
            return Err(ApprovalError::UncoveredTerminalRecord {
                node_id: record.node_id,
                step_name: record.step_name.clone(),
                ledger: path.display().to_string(),
            });
        }
        Ok(())
    }

    /// Record terminal consumption under the ledger lock, re-reading the file
    /// first so a marker another writer added is preserved (#1254).
    fn record_consumption(
        &self,
        record: &ApprovalRecord,
        settlement: Option<ConsumptionSettlement>,
    ) -> Result<(), ApprovalError> {
        let Some(path) = self.consumption_path.as_ref() else {
            return Ok(());
        };
        with_exclusive_lock(path, || {
            let mut consumed = if path.is_file() {
                read_consumption_ledger(&self.path, path, self.mac_key.as_deref())?
            } else {
                BTreeMap::new()
            };
            let key = marker_key(record.node_id, record.nonce);
            if consumed.contains_key(&key) {
                return if matches!(record.status, ApprovalStatus::Pending) {
                    Err(ApprovalError::RecordRollback {
                        node_id: record.node_id,
                        step_name: record.step_name.clone(),
                    })
                } else {
                    // Insert-if-absent, never upsert: the same key is the same
                    // approval, so a second terminal write is a replay (#1261).
                    Err(ApprovalError::ConsumptionReplay {
                        node_id: record.node_id,
                        step_name: record.step_name.clone(),
                        ledger: path.display().to_string(),
                    })
                };
            }
            if matches!(record.status, ApprovalStatus::Pending) {
                return Ok(());
            }
            consumed.insert(
                key,
                ConsumptionMarker {
                    node_id: record.node_id,
                    nonce: record.nonce,
                    status: record.status,
                    recorded_at: Utc::now(),
                    dag_id: settlement.map(|s| s.dag_id),
                    attempt: settlement.map(|s| s.attempt),
                },
            );
            write_consumption_ledger(path, &consumed, self.mac_key.as_deref())?;
            Ok(())
        })
    }

    fn read_store(path: &Path) -> Result<HashMap<Uuid, ApprovalRecord>, ApprovalError> {
        match std::fs::read_to_string(path) {
            Ok(contents) if !contents.trim().is_empty() => {
                serde_json::from_str(&contents).map_err(|e| {
                    ApprovalError::Internal(format!(
                        "corrupt approval store {}: {e}",
                        path.display()
                    ))
                })
            }
            Ok(_) => Ok(HashMap::new()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(HashMap::new()),
            Err(e) => Err(ApprovalError::Internal(format!(
                "read approval store {}: {e}",
                path.display()
            ))),
        }
    }

    /// Read-modify-write the whole store under the file lock, merging into the
    /// file at write time rather than publishing this instance's snapshot.
    fn update_store(
        &self,
        mutate: impl FnOnce(&mut HashMap<Uuid, ApprovalRecord>),
    ) -> Result<(), ApprovalError> {
        with_exclusive_lock(&self.path, || {
            let mut map = Self::read_store(&self.path)?;
            mutate(&mut map);
            let contents = serde_json::to_string_pretty(&map)
                .map_err(|e| ApprovalError::Internal(format!("serialize store: {e}")))?;
            // Unique per write: a fixed `.json.tmp` lets two writers rename the
            // same path and lose one record (#1254).
            let tmp = self.path.with_file_name(format!(
                ".{}.{}.tmp",
                self.path
                    .file_name()
                    .map(|name| name.to_string_lossy())
                    .unwrap_or_default(),
                Uuid::new_v4()
            ));
            std::fs::write(&tmp, contents)
                .map_err(|e| ApprovalError::Internal(format!("write store: {e}")))?;
            std::fs::rename(&tmp, &self.path)
                .map_err(|e| ApprovalError::Internal(format!("commit store: {e}")))?;
            Ok(())
        })
    }
}

#[async_trait::async_trait]
impl crate::approval::infrastructure::repository::ApprovalRepository
    for FileBackedApprovalRepository
{
    async fn save(&self, record: &ApprovalRecord) -> Result<(), ApprovalError> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| ApprovalError::Internal("file repo write lock poisoned".into()))?;
        let sealed = seal_if_keyed(record, self.mac_key.as_deref())?;
        // Marker first: a marker without a record is visible and safe, a
        // terminal record without a marker is the invisible failure (#1254).
        self.record_consumption(&sealed, None)?;
        self.update_store(|map| {
            map.insert(record.node_id, sealed);
        })
    }

    async fn save_with_settlement(
        &self,
        record: &ApprovalRecord,
        settlement: ConsumptionSettlement,
    ) -> Result<(), ApprovalError> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| ApprovalError::Internal("file repo write lock poisoned".into()))?;
        let sealed = seal_if_keyed(record, self.mac_key.as_deref())?;
        self.record_consumption(&sealed, Some(settlement))?;
        self.update_store(|map| {
            map.insert(record.node_id, sealed);
        })
    }

    async fn consumption_settlement(
        &self,
        node_id: Uuid,
    ) -> Result<Option<ConsumptionSettlement>, ApprovalError> {
        if self.consumption_path.is_none() {
            return Ok(None);
        }
        let snapshot = self.read_snapshot()?;
        Ok(snapshot
            .consumed
            .values()
            .filter(|marker| marker.node_id == node_id)
            .max_by_key(|marker| marker.recorded_at)
            .and_then(|marker| match (marker.dag_id, marker.attempt) {
                (Some(dag_id), Some(attempt)) => Some(ConsumptionSettlement {
                    dag_id,
                    node_id: marker.node_id,
                    attempt,
                }),
                _ => None,
            }))
    }

    async fn load(&self, node_id: Uuid) -> Result<Option<ApprovalRecord>, ApprovalError> {
        let snapshot = self.read_snapshot()?;
        let terminal_records = terminal_record_count(&snapshot.records);
        let record = snapshot
            .records
            .get(&node_id)
            .cloned()
            .map(|record| verify_if_keyed(record, self.mac_key.as_deref()))
            .transpose()?;
        if let Some(record) = record.as_ref() {
            Self::reject_rolled_back(
                record,
                &snapshot.consumed,
                snapshot.ledger_present,
                self.consumption_path.as_deref(),
                terminal_records,
            )?;
        }
        Ok(record)
    }

    async fn verify_integrity(&self) -> Result<usize, ApprovalError> {
        let Some(key) = self.mac_key.as_deref() else {
            return Ok(0);
        };
        let snapshot = self.read_snapshot()?;
        let terminal_records = terminal_record_count(&snapshot.records);
        for record in snapshot.records.values() {
            if !record.verify_integrity(key) {
                return Err(ApprovalError::RecordIntegrity {
                    node_id: record.node_id,
                    step_name: record.step_name.clone(),
                });
            }
            Self::reject_rolled_back(
                record,
                &snapshot.consumed,
                snapshot.ledger_present,
                self.consumption_path.as_deref(),
                terminal_records,
            )?;
        }
        Ok(snapshot.records.len())
    }

    async fn consumption_marker_count(&self) -> Result<usize, ApprovalError> {
        if self.consumption_path.is_none() {
            return Ok(0);
        }
        Ok(self.read_snapshot()?.consumed.len())
    }

    async fn migrate_consumption_ledger(&self) -> Result<LedgerMigration, ApprovalError> {
        let ledger = self.consumption_path.as_ref().ok_or_else(|| {
            ApprovalError::Internal("no consumption ledger path is configured".into())
        })?;
        migrate_consumption_ledger_at(&self.path, ledger, self.mac_key.as_deref(), false)
    }

    async fn delete(&self, node_id: Uuid) -> Result<(), ApprovalError> {
        let _guard = self
            .write_lock
            .lock()
            .map_err(|_| ApprovalError::Internal("file repo write lock poisoned".into()))?;
        self.update_store(|map| {
            map.remove(&node_id);
        })
    }
}
