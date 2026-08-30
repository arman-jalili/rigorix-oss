//! FileSystemIdentityRepository — durable identity records alongside execution state.
//!
//! @canonical .pi/architecture/modules/identity.md#identityrepository
//! Implements: ISSUE-IDENTITY-5 — concrete persistence for IdentityRepository
//! Issue: #705 (identity epic)
//!
//! Filesystem-backed implementation of `IdentityRepository`: stores identity
//! claims as JSON files using atomic write-rename for crash safety, following
//! the state-persistence pattern. Files are stored as
//! `{state_dir}/{execution_id}.identity.json`.
//!
//! # Atomic Write-Rename Pattern
//! 1. Serialize the claim to `{execution_id}.identity.json.tmp`
//! 2. `fs::rename` to `{execution_id}.identity.json`
//!
//! On POSIX, `rename(2)` is atomic — a power failure during write leaves the
//! original file intact.
//!
//! # Contract
//! - Raw tokens are stored **by reference** (`token_ref`), never embedded in
//!   the serialized record
//! - `load` returns `Ok(None)` when no identity was recorded for the execution
//! - `delete` is idempotent

use async_trait::async_trait;
use std::path::PathBuf;
use tokio::fs;
use uuid::Uuid;

use crate::identity::domain::{IdentityClaim, IdentityError};
use crate::identity::infrastructure::repository::identity_repository::IdentityRepository;

/// Filesystem-backed implementation of `IdentityRepository`.
///
/// Stores identity claims as JSON files in a configurable directory (the same
/// convention as state persistence). Uses atomic write-rename for crash safety.
pub struct FileSystemIdentityRepository {
    /// Directory where identity record files are stored.
    state_dir: PathBuf,
}

impl FileSystemIdentityRepository {
    /// Create a new `FileSystemIdentityRepository`.
    ///
    /// The state directory is created if it does not exist.
    /// Returns `IdentityError::Internal` if the directory cannot be created.
    pub async fn new(state_dir: impl Into<PathBuf>) -> Result<Self, IdentityError> {
        let state_dir: PathBuf = state_dir.into();

        if !state_dir.exists() {
            fs::create_dir_all(&state_dir)
                .await
                .map_err(|e| IdentityError::Internal(format!("create state dir: {e}")))?;
        }

        if !state_dir.is_dir() {
            return Err(IdentityError::Internal(format!(
                "state path {:?} exists but is not a directory",
                state_dir
            )));
        }

        Ok(Self { state_dir })
    }

    /// Path to the identity record file for an execution.
    fn record_path(&self, execution_id: Uuid) -> PathBuf {
        self.state_dir.join(format!("{execution_id}.identity.json"))
    }

    /// Path to the temporary identity record file for an execution.
    fn temp_path(&self, execution_id: Uuid) -> PathBuf {
        self.state_dir
            .join(format!("{execution_id}.identity.json.tmp"))
    }
}

#[async_trait]
impl IdentityRepository for FileSystemIdentityRepository {
    async fn save(&self, execution_id: Uuid, claim: &IdentityClaim) -> Result<(), IdentityError> {
        let path = self.record_path(execution_id);
        let temp = self.temp_path(execution_id);

        // Serialize first to catch serialization errors before touching disk.
        let json = serde_json::to_string_pretty(claim)
            .map_err(|e| IdentityError::Internal(format!("serialize claim: {e}")))?;

        // Write to temp file — the original is intact if we fail here.
        fs::write(&temp, json)
            .await
            .map_err(|e| IdentityError::Internal(format!("write temp record: {e}")))?;

        // Atomic rename (on POSIX, rename(2) is atomic).
        fs::rename(&temp, &path)
            .await
            .map_err(|e| IdentityError::Internal(format!("rename record: {e}")))?;

        Ok(())
    }

    async fn load(&self, execution_id: Uuid) -> Result<Option<IdentityClaim>, IdentityError> {
        let path = self.record_path(execution_id);

        if !path.exists() {
            return Ok(None);
        }

        let bytes = fs::read(&path)
            .await
            .map_err(|e| IdentityError::Internal(format!("read record: {e}")))?;

        let claim = serde_json::from_slice(&bytes)
            .map_err(|e| IdentityError::Internal(format!("deserialize claim: {e}")))?;

        Ok(Some(claim))
    }

    async fn delete(&self, execution_id: Uuid) -> Result<(), IdentityError> {
        let path = self.record_path(execution_id);

        // Idempotent — removing a missing record is a no-op success.
        match fs::remove_file(&path).await {
            Ok(()) => Ok(()),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(IdentityError::Internal(format!("delete record: {e}"))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::identity::domain::IdentitySource;

    fn sample_claim() -> IdentityClaim {
        IdentityClaim {
            subject: "user@org".to_string(),
            issuer: "https://idp.example.com".to_string(),
            authority: Some("admin".to_string()),
            source: IdentitySource::IdpToken,
            auth_method: Some("device_code".to_string()),
            issued_at: chrono::Utc::now(),
            expires_at: Some(chrono::Utc::now() + chrono::Duration::minutes(15)),
            // Reference only — never the raw token value.
            token_ref: Some("keychain://default/rigorix/idp-token".to_string()),
        }
    }

    async fn repo() -> FileSystemIdentityRepository {
        let dir = tempfile::tempdir().expect("tempdir");
        let dir_path = dir.keep();
        FileSystemIdentityRepository::new(&dir_path)
            .await
            .expect("repo")
    }

    #[tokio::test]
    async fn save_load_round_trip_preserves_all_fields() {
        let repo = repo().await;
        let execution_id = uuid::Uuid::new_v4();
        let claim = sample_claim();

        repo.save(execution_id, &claim).await.expect("save");

        let loaded = repo.load(execution_id).await.expect("load");
        assert_eq!(loaded, Some(claim), "round-trip preserves every field");
    }

    #[tokio::test]
    async fn load_missing_record_returns_none() {
        let repo = repo().await;
        let loaded = repo.load(uuid::Uuid::new_v4()).await.expect("load missing");
        assert_eq!(
            loaded, None,
            "no identity recorded => None (identity optional)"
        );
    }

    #[tokio::test]
    async fn delete_removes_record_and_is_idempotent() {
        let repo = repo().await;
        let execution_id = uuid::Uuid::new_v4();
        repo.save(execution_id, &sample_claim())
            .await
            .expect("save");

        repo.delete(execution_id).await.expect("delete");
        assert_eq!(repo.load(execution_id).await.expect("load"), None);

        // Second delete is a no-op success.
        repo.delete(execution_id).await.expect("idempotent delete");
    }

    #[tokio::test]
    async fn save_overwrites_previous_record() {
        let repo = repo().await;
        let execution_id = uuid::Uuid::new_v4();

        repo.save(execution_id, &sample_claim())
            .await
            .expect("save");

        let updated = IdentityClaim {
            subject: "updated@org".to_string(),
            ..sample_claim()
        };
        repo.save(execution_id, &updated).await.expect("save again");

        let loaded = repo.load(execution_id).await.expect("load");
        assert_eq!(loaded, Some(updated), "latest write wins");
    }

    #[tokio::test]
    async fn serialized_record_never_embeds_raw_token_value() {
        let repo = repo().await;
        let execution_id = uuid::Uuid::new_v4();
        let claim = sample_claim();
        repo.save(execution_id, &claim).await.expect("save");

        // The record references the token by locator — the raw token value
        // (e.g. a JWT payload marker) must not appear in the file.
        let record_path = repo.record_path(execution_id);
        let contents = std::fs::read_to_string(record_path).expect("read record");
        assert!(
            !contents.contains("eyJhbGciOiJSUzI1NiJ9"),
            "raw token payload leaked into the serialized record"
        );
        assert!(contents.contains("keychain://default/rigorix/idp-token"));
    }
}
