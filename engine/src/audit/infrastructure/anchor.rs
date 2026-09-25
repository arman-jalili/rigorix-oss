//! ADR-016 Phase C — anchor runtime: fetch + Ed25519-verify the signed
//! history projection, and report the active mode/head.
//!
//! @canonical .pi/architecture/decisions/ADR-016-audit-integrity-anchor.md
//! Issue: #899 (OSS-AUDIT-C)
//!
//! The engine owns the **port** ([`AnchorSliceSource`]) and the **verification**
//! ([`verify_slice`]). The concrete HTTP client lives here too so every
//! composition root (the MCP host and `rigorix-server`) shares one runtime; the
//! server's `anchor` module exposes this state for `rigorix.system.version`.
//!
//! Security invariant: a slice is handed to the matcher **only** after its
//! Ed25519 signature verifies against the configured public key. Any failure
//! (unreachable, unsigned, forged, scope mismatch) is an error, which the
//! `AnchoredHistoryAdapter` propagates as a fail-closed refusal.

use std::sync::{Arc, Mutex, OnceLock};
use std::time::Duration;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use ed25519_dalek::{Signature, Verifier, VerifyingKey};
use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::audit::domain::anchor::{AnchorMode, HistoryActionRef, HistorySlice};

/// Environment variable: anchor base URL. Set (with `RIGORIX_ANCHOR_PUBLIC_KEY`)
/// to enable `anchored` mode.
pub const ANCHOR_URL_ENV: &str = "RIGORIX_ANCHOR_URL";
/// Environment variable: hex-encoded 32-byte Ed25519 public key (out of band).
pub const ANCHOR_PUBLIC_KEY_ENV: &str = "RIGORIX_ANCHOR_PUBLIC_KEY";
/// Environment variable: history scope requested from the anchor
/// (defaults to `default`).
pub const ANCHOR_SCOPE_ENV: &str = "RIGORIX_ANCHOR_SCOPE";

/// Anchor read/verification failure. Every variant is fail-closed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum AnchorError {
    /// No anchor is configured — the anchored adapter refuses to fabricate
    /// history.
    #[error("anchor is not configured (set RIGORIX_ANCHOR_URL + RIGORIX_ANCHOR_PUBLIC_KEY)")]
    NotConfigured,
    /// The anchor could not be reached / returned a non-success status.
    #[error("anchor unreachable: {0}")]
    Unreachable(String),
    /// The response was not a well-formed signed slice, or covered the wrong
    /// scope.
    #[error("invalid anchor slice: {0}")]
    InvalidSlice(String),
    /// The slice's Ed25519 signature did not verify against the configured key.
    #[error("anchor signature verification failed: {0}")]
    SignatureInvalid(String),
}

/// The anchor's signed-history read port. Implemented by the HTTP client in
/// production; a fixture/mock in tests.
#[async_trait]
pub trait AnchorSliceSource: Send + Sync {
    /// Fetch the signed history projection for `scope` since `since`.
    ///
    /// # Errors
    /// [`AnchorError`] — the caller fails closed.
    async fn fetch_slice(
        &self,
        scope: &str,
        since: DateTime<Utc>,
    ) -> Result<HistorySlice, AnchorError>;
}

/// Verify a slice's Ed25519 signature over its canonical (`sig`-nulled) bytes.
///
/// # Errors
/// - [`AnchorError::SignatureInvalid`] when unsigned or the signature does not
///   verify.
/// - [`AnchorError::InvalidSlice`] when the hex/signature bytes are malformed.
pub fn verify_slice(slice: &HistorySlice, public_key_hex: &str) -> Result<(), AnchorError> {
    let sig_hex = slice
        .sig
        .as_deref()
        .ok_or_else(|| AnchorError::SignatureInvalid("slice is unsigned".into()))?;
    let bytes = slice.signing_bytes().map_err(AnchorError::InvalidSlice)?;

    let sig_bytes =
        hex::decode(sig_hex).map_err(|e| AnchorError::InvalidSlice(format!("sig hex: {e}")))?;
    let sig = Signature::from_slice(&sig_bytes)
        .map_err(|e| AnchorError::InvalidSlice(format!("sig bytes: {e}")))?;

    let key_bytes = hex::decode(public_key_hex)
        .map_err(|e| AnchorError::InvalidSlice(format!("key hex: {e}")))?;
    let key_arr: [u8; 32] = key_bytes
        .as_slice()
        .try_into()
        .map_err(|_| AnchorError::InvalidSlice("public key must be 32 bytes".into()))?;
    let key = VerifyingKey::from_bytes(&key_arr)
        .map_err(|e| AnchorError::InvalidSlice(format!("public key: {e}")))?;

    key.verify(&bytes, &sig)
        .map_err(|e| AnchorError::SignatureInvalid(e.to_string()))
}

/// Active anchor mode + identity + last verified head — the auditable facts
/// `rigorix.system.version` reports.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AnchorStatus {
    /// The active history mode.
    pub mode: AnchorMode,
    /// The configured anchor identity (base URL), when anchored.
    pub anchor_id: Option<String>,
    /// The most recent **verified** ledger head, when anchored.
    pub head: Option<String>,
}

/// Shared anchor runtime held by the composition root.
///
/// In `anchored` mode it fans out to a [`AnchorSliceSource`], verifies every
/// projection, and records the verified head. In `local_unanchored` mode it has
/// no source and any read is [`AnchorError::NotConfigured`] (fail closed).
pub struct AnchorRuntime {
    mode: AnchorMode,
    anchor_id: Option<String>,
    public_key_hex: Option<String>,
    scope: String,
    source: Option<Arc<dyn AnchorSliceSource>>,
    head: Mutex<Option<String>>,
}

impl AnchorRuntime {
    /// A `local_unanchored` runtime (no anchor configured — Phase A behavior).
    pub fn disabled() -> Arc<Self> {
        Arc::new(Self {
            mode: AnchorMode::LocalUnanchored,
            anchor_id: None,
            public_key_hex: None,
            scope: "default".into(),
            source: None,
            head: Mutex::new(None),
        })
    }

    /// An `anchored` runtime over the given source + public key.
    pub fn anchored(
        source: Arc<dyn AnchorSliceSource>,
        public_key_hex: impl Into<String>,
        anchor_id: impl Into<String>,
        scope: impl Into<String>,
    ) -> Arc<Self> {
        Arc::new(Self {
            mode: AnchorMode::Anchored,
            anchor_id: Some(anchor_id.into()),
            public_key_hex: Some(public_key_hex.into()),
            scope: scope.into(),
            source: Some(source),
            head: Mutex::new(None),
        })
    }

    /// Build the runtime from `RIGORIX_ANCHOR_URL` / `RIGORIX_ANCHOR_PUBLIC_KEY`.
    ///
    /// Anchored iff **both** are set and non-empty; otherwise `local_unanchored`.
    pub fn from_env() -> Arc<Self> {
        let url = std::env::var(ANCHOR_URL_ENV)
            .ok()
            .filter(|v| !v.trim().is_empty());
        let key = std::env::var(ANCHOR_PUBLIC_KEY_ENV)
            .ok()
            .filter(|v| !v.trim().is_empty());
        let (Some(url), Some(key)) = (url, key) else {
            return Self::disabled();
        };
        let scope = std::env::var(ANCHOR_SCOPE_ENV)
            .ok()
            .filter(|v| !v.trim().is_empty())
            .unwrap_or_else(|| "default".into());
        Self::anchored(
            Arc::new(HttpAnchorClient::new(url.clone())),
            key,
            url,
            scope,
        )
    }

    /// The active mode.
    pub fn mode(&self) -> AnchorMode {
        self.mode
    }

    /// The configured anchor identity (base URL), when anchored.
    pub fn anchor_id(&self) -> Option<&str> {
        self.anchor_id.as_deref()
    }

    /// The configured scope.
    pub fn scope(&self) -> &str {
        &self.scope
    }

    /// The most recent verified head, when anchored + read.
    pub fn head(&self) -> Option<String> {
        self.head.lock().ok().and_then(|g| g.clone())
    }

    /// Record a verified head (called after signature verification).
    pub fn set_head(&self, head: &str) {
        if let Ok(mut guard) = self.head.lock() {
            *guard = Some(head.to_string());
        }
    }

    /// A snapshot for `rigorix.system.version`.
    pub fn status(&self) -> AnchorStatus {
        AnchorStatus {
            mode: self.mode,
            anchor_id: self.anchor_id.clone(),
            head: self.head(),
        }
    }

    /// Fetch a projection and verify it **before** returning any action.
    ///
    /// # Errors
    /// - [`AnchorError::NotConfigured`] in `local_unanchored` mode.
    /// - [`AnchorError::Unreachable`] when the anchor cannot be read.
    /// - [`AnchorError::SignatureInvalid`] / [`AnchorError::InvalidSlice`] on a
    ///   forged, unsigned, malformed, or out-of-scope slice.
    pub async fn fetch_verified(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<HistoryActionRef>, AnchorError> {
        let (Some(source), Some(key)) = (&self.source, &self.public_key_hex) else {
            return Err(AnchorError::NotConfigured);
        };
        let slice = source.fetch_slice(&self.scope, since).await?;
        verify_slice(&slice, key)?;
        if slice.scope != self.scope {
            return Err(AnchorError::InvalidSlice(format!(
                "scope mismatch: expected '{}', got '{}'",
                self.scope, slice.scope
            )));
        }
        self.set_head(&slice.head_hash);
        Ok(slice.actions)
    }
}

/// Production anchor client: `GET {base}/v1/history?scope=..&since=..`
/// returning a signed [`HistorySlice`].
pub struct HttpAnchorClient {
    base_url: String,
    http: reqwest::Client,
}

impl HttpAnchorClient {
    /// Create a client for `base_url` with a bounded request timeout.
    pub fn new(base_url: impl Into<String>) -> Self {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .build()
            .unwrap_or_default();
        Self {
            base_url: base_url.into(),
            http,
        }
    }
}

#[async_trait]
impl AnchorSliceSource for HttpAnchorClient {
    async fn fetch_slice(
        &self,
        scope: &str,
        since: DateTime<Utc>,
    ) -> Result<HistorySlice, AnchorError> {
        let url = format!("{}/v1/history", self.base_url.trim_end_matches('/'));
        let since = since.to_rfc3339();
        let response = self
            .http
            .get(url)
            .query(&[("scope", scope), ("since", since.as_str())])
            .send()
            .await
            .map_err(|e| AnchorError::Unreachable(e.to_string()))?;
        if !response.status().is_success() {
            return Err(AnchorError::Unreachable(format!(
                "anchor returned {}",
                response.status()
            )));
        }
        response
            .json::<HistorySlice>()
            .await
            .map_err(|e| AnchorError::InvalidSlice(e.to_string()))
    }
}

/// Process-global runtime (the composition root's anchor choice).
static RUNTIME: OnceLock<Arc<AnchorRuntime>> = OnceLock::new();

/// The shared runtime, lazily initialised from the environment on first use.
pub fn runtime() -> Arc<AnchorRuntime> {
    RUNTIME.get_or_init(AnchorRuntime::from_env).clone()
}

/// The shared runtime if it has already been initialised.
pub fn runtime_opt() -> Option<Arc<AnchorRuntime>> {
    RUNTIME.get().cloned()
}

/// Install the shared runtime (composition root / tests). Returns the existing
/// runtime if one is already installed.
///
/// # Errors
/// The already-installed runtime.
pub fn install_runtime(runtime: Arc<AnchorRuntime>) -> Result<(), Arc<AnchorRuntime>> {
    RUNTIME.set(runtime)
}

/// The active anchor status (initialises the runtime from env if needed).
pub fn runtime_status() -> AnchorStatus {
    runtime().status()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::domain::anchor::HistoryActionRef;
    use chrono::TimeZone;
    use ed25519_dalek::Signer;

    fn signing_key() -> ed25519_dalek::SigningKey {
        ed25519_dalek::SigningKey::from_bytes(&[7u8; 32])
    }

    fn signed_slice(head: &str) -> (HistorySlice, String) {
        let key = signing_key();
        let mut slice = HistorySlice {
            scope: "local".into(),
            since: Utc.timestamp_opt(0, 0).unwrap(),
            actions: vec![HistoryActionRef {
                node: "remove".into(),
                principal: Some("alice".into()),
                at: Utc.timestamp_opt(1, 0).unwrap(),
                effect_key: None,
            }],
            head_hash: head.into(),
            sig: None,
        };
        let sig = key.sign(&slice.signing_bytes().unwrap());
        slice.sig = Some(hex::encode(sig.to_bytes()));
        (slice, hex::encode(key.verifying_key().to_bytes()))
    }

    #[test]
    fn verifies_a_correctly_signed_slice() {
        let (slice, key) = signed_slice(&"ab".repeat(32));
        assert!(verify_slice(&slice, &key).is_ok());
    }

    #[test]
    fn rejects_a_forged_slice() {
        let (mut slice, key) = signed_slice(&"ab".repeat(32));
        // Tamper AFTER signing — head binding is part of the signed bytes.
        slice.head_hash = "ff".repeat(32);
        assert!(matches!(
            verify_slice(&slice, &key),
            Err(AnchorError::SignatureInvalid(_))
        ));
    }

    #[test]
    fn rejects_an_unsigned_slice() {
        let (mut slice, key) = signed_slice(&"ab".repeat(32));
        slice.sig = None;
        assert!(matches!(
            verify_slice(&slice, &key),
            Err(AnchorError::SignatureInvalid(_))
        ));
    }
}
