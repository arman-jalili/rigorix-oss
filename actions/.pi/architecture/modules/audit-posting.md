# Audit Posting Architecture

<!--
Canonical Reference: .pi/architecture/modules/audit-posting.md
Blueprint Source: Ported from original Rigorix docs/ARCHITECTURE_GITHUB_ACTIONS.md §2.5 + open-core boundary design (2026-06-20)
Rationale: Open-core audit trail — OSS ships with FilesystemAuditBackend; enterprise plugs in RigorixApiBackend via AuditBackend trait
-->

## Overview

The Audit Posting module posts HMAC-signed execution records to an audit backend. It defines the `AuditBackend` trait — the open-core boundary between the OSS Rigorix and enterprise backend services. The OSS ships with `FilesystemAuditBackend` (local `.rigorix/audit/` directory). The enterprise SaaS implements `RigorixApiBackend` (managed PostgreSQL + dashboard).

This module does NOT implement the backend itself — it provides the interface and the OSS default. Community members can implement `S3AuditBackend`, `PostgresAuditBackend`, or any other storage.

## Philosophy

The audit trail is a **public interface, not a proprietary product**. By opening the `AuditBackend` trait:

- OSS users get a complete local audit experience (`rigorix audit list`)
- Enterprise customers get cross-repo visibility + dashboards
- Community can build plugins for S3, MinIO, Datadog, etc.
- The HMAC signature format is open and verifiable by anyone

## Responsibilities

- Define `AuditBackend` trait (open-core boundary)
- Ship `FilesystemAuditBackend` as the OSS default
- Sign audit records with HMAC-SHA256 for integrity
- Retry posting with exponential backoff (circuit breaker integration)
- Queue audit records locally when backend is unavailable
- Format audit records as structured JSON with HMAC signature
- Support `NoopAuditBackend` for dry-run/testing

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| AuditBackend | `actions/src/audit_posting/backend.rs` | Trait: post_audit, query_audit, health_check | #backend |
| SignedAuditRecord | `actions/src/audit_posting/record.rs` | HMAC-signed audit record format | #record |
| AuditSigner | `actions/src/audit_posting/signer.rs` | Signs records with HMAC-SHA256 | #signer |
| FilesystemAuditBackend | `actions/src/audit_posting/filesystem_backend.rs` | OSS default: writes to .rigorix/audit/ | #filesystem |
| NoopAuditBackend | `actions/src/audit_posting/noop_backend.rs` | Dry-run backend (testing, disabled) | #noop |
| AuditRetryConfig | `actions/src/audit_posting/retry.rs` | Exponential backoff with jitter | #retry |
| AuditRecordQueue | `actions/src/audit_posting/queue.rs` | Local queue for offline resilience | #queue |
| AuditPoster | `actions/src/audit_posting/poster.rs` | Orchestrates posting with retry + circuit breaker | #poster |
| AuditError | `actions/src/audit_posting/error.rs` | Typed errors: BackendUnavailable, SigningFailed | #error |

---

## Component Details

### AuditBackend Trait (Open-Core Boundary)

```rust
/// Trait for posting audit records to an external backend.
///
/// This is the **open-core boundary**. The OSS ships with
/// `FilesystemAuditBackend` (local .rigorix/audit/). Enterprise
/// implements `RigorixApiBackend`. Community implements custom backends.
///
/// # Implementing a custom backend
///
/// 1. Implement this trait
/// 2. Register via `.rigorix/audit.toml` → `backend = "s3"` or plugin system
/// 3. The action will call `post_audit()` after each execution
#[async_trait]
pub trait AuditBackend: Send + Sync {
    /// Post a signed audit record. Returns the remote record ID.
    async fn post_audit(
        &self,
        record: &SignedAuditRecord,
    ) -> Result<String, AuditError>;

    /// Query audit records by repository and date range.
    async fn query_audit(
        &self,
        repo: &str,
        since: chrono::DateTime<chrono::Utc>,
        until: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<AuditRecord>, AuditError>;

    /// Health check — is the backend reachable?
    async fn health_check(&self) -> Result<(), AuditError>;

    /// Backend display name for CLI output.
    fn name(&self) -> &str;

    /// Backend version / capabilities string.
    fn version(&self) -> &str { "1.0.0" }
}
```

### SignedAuditRecord

```rust
/// An HMAC-signed audit record posted to the backend.
///
/// The signature covers the entire `AuditRecord` payload and is
/// verified by the backend before storage. This prevents spoofing
/// by unauthorized actors who might have PR comment access but
/// not the HMAC signing key.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SignedAuditRecord {
    /// The audit record payload.
    pub record: AuditRecord,

    /// HMAC-SHA256 signature of the serialized record.
    /// Format: hex-encoded 32-byte HMAC.
    pub signature: String,

    /// The key ID used for signing (for key rotation).
    pub key_id: String,

    /// ISO 8601 timestamp of signing.
    pub signed_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuditRecord {
    /// Unique execution ID.
    pub execution_id: String,

    /// Repository identifier (org/repo).
    pub repository: String,

    /// PR number (if triggered by PR).
    pub pr_number: Option<u64>,

    /// Commit SHA.
    pub commit_sha: String,

    /// Execution mode: governance (Mode A) or execution (Mode B).
    pub mode: String,

    /// Execution outcome: completed, failed, partial_failure.
    pub status: String,

    /// Policy evaluation results (Mode A only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub policy_result: Option<PolicyResult>,

    /// Validation outcome (Mode B only).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub validation_outcome: Option<String>,

    /// Number of validation iterations.
    pub iterations: u32,

    /// Cumulative LLM tokens used.
    pub cumulative_tokens: u64,

    /// Total execution duration in milliseconds.
    pub duration_ms: u64,

    /// Number of files changed (Mode A) or generated (Mode B).
    pub files_changed: usize,

    /// Quality level achieved.
    pub quality_level: String,

    /// ISO 8601 timestamps.
    pub started_at: String,
    pub completed_at: String,

    /// Policy version hash (for reproducibility).
    pub policy_version: String,
}
```

### FilesystemAuditBackend (OSS Default)

```rust
/// Ships with the OSS action. Stores audit records as JSON files
/// in `.rigorix/audit/{execution_id}.json`.
///
/// Each record is atomically written (write to .tmp, rename to final)
/// for crash safety. Records include the HMAC signature for integrity
/// verification, even locally.
pub struct FilesystemAuditBackend {
    root: PathBuf,
}

impl FilesystemAuditBackend {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let root = root.into();
        std::fs::create_dir_all(&root).ok();
        Self { root }
    }
}

#[async_trait]
impl AuditBackend for FilesystemAuditBackend {
    async fn post_audit(&self, record: &SignedAuditRecord) -> Result<String, AuditError> {
        let id = &record.record.execution_id;
        let path = self.root.join(format!("{}.json", id));
        let tmp = self.root.join(format!("{}.tmp", id));

        let json = serde_json::to_string_pretty(record)
            .map_err(|e| AuditError::Serialization(e.to_string()))?;

        // Atomic write
        std::fs::write(&tmp, &json)
            .map_err(|e| AuditError::Io(e))?;
        std::fs::rename(&tmp, &path)
            .map_err(|e| AuditError::Io(e))?;

        Ok(id.clone())
    }

    async fn query_audit(
        &self,
        repo: &str,
        since: chrono::DateTime<chrono::Utc>,
        until: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<AuditRecord>, AuditError> {
        let mut records = Vec::new();
        for entry in std::fs::read_dir(&self.root).map_err(|e| AuditError::Io(e))? {
            let entry = entry.map_err(|e| AuditError::Io(e))?;
            if entry.path().extension().map_or(true, |e| e != "json") {
                continue;
            }
            let content = std::fs::read_to_string(entry.path())
                .map_err(|e| AuditError::Io(e))?;
            if let Ok(signed) = serde_json::from_str::<SignedAuditRecord>(&content) {
                let ts = chrono::DateTime::parse_from_rfc3339(&signed.signed_at)
                    .unwrap_or_default()
                    .with_timezone(&chrono::Utc);
                if signed.record.repository == repo && ts >= since && ts <= until {
                    records.push(signed.record);
                }
            }
        }
        Ok(records)
    }

    async fn health_check(&self) -> Result<(), AuditError> {
        let test = self.root.join(".health_check");
        std::fs::write(&test, "ok").map_err(|e| AuditError::Io(e))?;
        std::fs::remove_file(&test).map_err(|e| AuditError::Io(e))?;
        Ok(())
    }

    fn name(&self) -> &str { "filesystem" }
}
```

### AuditPoster

```rust
/// Orchestrates audit record posting with retry and circuit breaker.
///
/// Uses the engine's circuit breaker pattern (exponential backoff,
/// cooldown, half-open state) for backend resilience.
pub struct AuditPoster {
    backend: Arc<dyn AuditBackend>,
    signer: AuditSigner,
    retry_config: AuditRetryConfig,
    queue: AuditRecordQueue,
}

impl AuditPoster {
    /// Post an audit record with retry. If the backend is unavailable,
    /// the record is queued locally and the action continues.
    pub async fn post_with_retry(
        &self,
        record: AuditRecord,
    ) -> Result<String, AuditError> {
        let signed = self.signer.sign(&record)?;

        // Try posting with retry
        let mut attempt = 0;
        loop {
            match self.backend.post_audit(&signed).await {
                Ok(id) => return Ok(id),
                Err(AuditError::BackendUnavailable) if attempt < self.retry_config.max_retries => {
                    attempt += 1;
                    let delay = self.retry_config.backoff(attempt);
                    tokio::time::sleep(delay).await;
                }
                Err(e) => {
                    // Non-retriable error — queue and continue
                    self.queue.push(&signed)?;
                    return Err(e);
                }
            }
        }

        // Max retries exhausted — queue and continue
        self.queue.push(&signed)?;
        Ok("queued".to_string())
    }
}

/// Retry configuration with exponential backoff and jitter.
#[derive(Debug, Clone)]
pub struct AuditRetryConfig {
    pub max_retries: u8,
    pub base_delay_ms: u64,
    pub max_delay_ms: u64,
    pub jitter_factor: f64,
}

impl Default for AuditRetryConfig {
    fn default() -> Self {
        Self {
            max_retries: 3,
            base_delay_ms: 1000,
            max_delay_ms: 30000,
            jitter_factor: 0.25,  // ±25% jitter
        }
    }
}

impl AuditRetryConfig {
    fn backoff(&self, attempt: u8) -> std::time::Duration {
        let delay = self.base_delay_ms * 2u64.pow(attempt as u32 - 1);
        let capped = delay.min(self.max_delay_ms);
        let jitter = (capped as f64 * self.jitter_factor * (rand::random::<f64>() * 2.0 - 1.0)) as u64;
        std::time::Duration::from_millis(capped + jitter)
    }
}
```

### AuditRecordQueue

```rust
/// Local queue for audit records when the backend is unavailable.
///
/// Records are stored in `.rigorix/audit/queue/` as individual JSON files.
/// A separate process (or next action run) replays queued records.
pub struct AuditRecordQueue {
    queue_dir: PathBuf,
}

impl AuditRecordQueue {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        let queue_dir = root.into().join("queue");
        std::fs::create_dir_all(&queue_dir).ok();
        Self { queue_dir }
    }

    /// Push a signed audit record to the queue.
    pub fn push(&self, record: &SignedAuditRecord) -> Result<(), AuditError> {
        let id = &record.record.execution_id;
        let path = self.queue_dir.join(format!("{}.json", id));
        let json = serde_json::to_string_pretty(record)
            .map_err(|e| AuditError::Serialization(e.to_string()))?;
        std::fs::write(&path, json)
            .map_err(|e| AuditError::Io(e))?;
        Ok(())
    }

    /// Get all queued records (for replay).
    pub fn drain(&self) -> Result<Vec<SignedAuditRecord>, AuditError> {
        let mut records = Vec::new();
        for entry in std::fs::read_dir(&self.queue_dir).map_err(|e| AuditError::Io(e))? {
            let entry = entry.map_err(|e| AuditError::Io(e))?;
            let content = std::fs::read_to_string(entry.path())
                .map_err(|e| AuditError::Io(e))?;
            if let Ok(record) = serde_json::from_str::<SignedAuditRecord>(&content) {
                records.push(record);
            }
            std::fs::remove_file(entry.path()).ok();
        }
        Ok(records)
    }
}
```

---

## Data Flow

```
Execution completes (Mode A or Mode B)
        │
        ▼
AuditRecord built from:
  - ExecutionRecord (engine)
  - PolicyResult (Mode A) or ValidationOutcome (Mode B)
  - Repository context (GitHub env)
        │
        ▼
AuditSigner::sign(&record)
  - HMAC-SHA256 of serialized record
  - key_id for key rotation tracking
        │
        ▼
SignedAuditRecord { record, signature, key_id, signed_at }
        │
        ▼
AuditPoster::post_with_retry(signed_record)
        │
        ├─ Backend reachable
        │     → backend.post_audit(&signed) → Ok(record_id)
        │
        ├─ Backend transient error (5xx, timeout)
        │     → retry with exponential backoff (max 3 attempts)
        │
        └─ Backend unavailable after retries
              → AuditRecordQueue::push(&signed)
              → continue (fail-open)
```

---

## Open-Core Plugin Architecture

```rust
// Community member registers a custom backend:
// .rigorix/audit.toml

[audit]
backend = "s3"               # or "postgres", "datadog", etc.

[audit.s3]
bucket = "my-rigorix-audit"
region = "us-east-1"
prefix = "audit/"
```

```rust
// The action resolves the backend at startup:
fn resolve_backend(config: &AuditConfig) -> Arc<dyn AuditBackend> {
    match config.backend.as_str() {
        "filesystem" => Arc::new(FilesystemAuditBackend::new(".rigorix/audit")),
        "noop" => Arc::new(NoopAuditBackend),
        "api" => {
            // Enterprise: RigorixApiBackend (from rigorix-enterprise crate)
            #[cfg(feature = "enterprise")]
            {
                let api = RigorixApiBackend::new(&config.api_url, &config.api_key);
                Arc::new(api)
            }
            #[cfg(not(feature = "enterprise"))]
            {
                panic!("Enterprise backend requires rigorix-enterprise crate")
            }
        }
        _ => {
            // Try plugin system
            PluginRegistry::load_backend(&config.backend)
                .unwrap_or_else(|| Arc::new(FilesystemAuditBackend::new(".rigorix/audit")))
        }
    }
}
```

---

## Dependencies

### Depends On
- **security-config**: `HmacSigner` for record signing
- **hmac + sha2**: HMAC-SHA256 (engine dependencies)
- **serde_json**: Record serialization
- **reqwest**: HTTP backends (plugin/enterprise)

### Used By
- **action-entrypoint**: Posts audit records after every execution
- **ci-integration**: Audit record IDs included in PR comments for traceability
- **CLI**: `rigorix audit list` queries the local backend

---

## Related ADRs

- **Actions ADR-101** (`actions/.pi/architecture/decisions/ADR-101-actions-as-thin-adapter.md`): AuditBackend trait is the open-core boundary
- **Actions ADR-103** (`actions/.pi/architecture/decisions/ADR-103-ci-permission-mode.md`): Audit posting is fail-open (doesn't block PRs)

---

*Last updated: 2026-06-20*
*Module version: 1.0.0 (Planned)*
*Ported from: original Rigorix docs/ARCHITECTURE_GITHUB_ACTIONS.md §2.5 + open-core boundary design*

---

**Status:** Planned
**Engine modules reused:** audit (record format, HMAC), configuration (audit.toml format)
