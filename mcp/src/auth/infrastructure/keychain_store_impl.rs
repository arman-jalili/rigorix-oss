//! KeychainStoreImpl — refresh-token custody (OS keychain + CI fallback).
//!
//! @canonical .pi/architecture/modules/auth.md#keychainstore-infrastructure
//! Implements: ISSUE-AUTH-3 — KeychainStore (Infrastructure)
//! Issue: #823
//! ADR-008: the refresh token lives in the OS keychain, never in readable files
//!
//! Concrete implementation of the frozen [`KeychainStore`] port:
//!
//! - [`KeychainStoreImpl::keychain`] — OS keychain via the `keyring` crate
//!   (macOS Keychain, Windows Credential Manager, Linux Secret Service)
//! - [`KeychainStoreImpl::plaintext`] — **explicit opt-in** degraded mode for
//!   CI environments without a keychain: credentials in a file restricted to
//!   the owning user (0600 on Unix), with a prominent warning at construction
//!
//! # Security Contract
//!
//! - The refresh token is the crown jewel: the default path never writes to
//!   `.rigorix/` or any agent-readable file
//! - The plaintext fallback is never automatic — only an explicit constructor
//!   choice (documented degraded mode, ADR-008)
//! - Entry identifiers (`service`/`account`) are sanitized before touching the
//!   filesystem (path-traversal guard in fallback mode)
//! - Credential material is never logged or echoed in errors

use std::fs;
use std::path::{Path, PathBuf};

use tracing::warn;

use crate::auth::domain::error::AuthError;
use crate::auth::domain::value::Secret;
use crate::auth::infrastructure::keychain_store::KeychainStore;

/// Concrete keychain store.
///
/// Either backs onto the OS keychain (`keyring`) or the explicit plaintext
/// fallback directory. See [`KeychainStoreImpl::keychain`] and
/// [`KeychainStoreImpl::plaintext`].
#[derive(Debug, Clone)]
pub struct KeychainStoreImpl {
    /// Fallback storage directory when in degraded (plaintext) mode.
    fallback_dir: Option<PathBuf>,
}

impl KeychainStoreImpl {
    /// Create an OS-keychain-backed store (production default).
    ///
    /// # Errors
    /// - `AuthError::Keychain` — the platform keychain is unavailable
    ///   (headless/CI without a secret service); use
    ///   [`KeychainStoreImpl::plaintext`] explicitly for CI
    pub fn keychain() -> Result<Self, AuthError> {
        // Probe the platform store up front so misconfiguration surfaces at
        // construction, not on first use.
        let entry = keyring::Entry::new(
            crate::auth::infrastructure::keychain_store::RIGORIX_KEYCHAIN_SERVICE,
            "probe",
        )
        .map_err(|e| AuthError::Keychain(format!("OS keychain unavailable: {e}")))?;
        // Best-effort probe cleanup (no credential exists yet).
        let _ = entry.delete_credential();
        Ok(Self { fallback_dir: None })
    }

    /// Create the explicit plaintext-file fallback (degraded mode).
    ///
    /// Only for CI environments without an OS keychain. Logs a prominent
    /// warning: the refresh token will be readable by the owning OS user —
    /// never rely on this on a shared/multi-agent machine.
    ///
    /// # Errors
    /// - `AuthError::Keychain` — the directory cannot be created/written
    pub fn plaintext(dir: impl Into<PathBuf>) -> Result<Self, AuthError> {
        let dir = dir.into();
        fs::create_dir_all(&dir).map_err(|e| {
            AuthError::Keychain(format!("cannot create fallback dir {}: {e}", dir.display()))
        })?;
        warn!(
            target: "rigorix::auth",
            path = %dir.display(),
            "KeychainStore is using the EXPLICIT PLAINTEXT FILE fallback (degraded mode, \
             ADR-008). The refresh token is readable by the owning OS user — intended only \
             for CI environments without a keychain."
        );
        Ok(Self {
            fallback_dir: Some(dir),
        })
    }

    /// Resolve the per-entry store when in fallback mode.
    fn fallback_path(&self, service: &str, account: &str) -> Option<PathBuf> {
        let dir = self.fallback_dir.as_ref()?;
        let name = format!("{}_{}.tok", sanitize(service), sanitize(account));
        Some(dir.join(name))
    }

    /// True when running against the OS keychain.
    fn uses_keychain(&self) -> bool {
        self.fallback_dir.is_none()
    }
}

/// Make an identifier filesystem-safe (path-traversal guard).
fn sanitize(part: &str) -> String {
    let mut out = String::with_capacity(part.len());
    for ch in part.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else {
            out.push('_');
        }
    }
    out
}

/// Best-effort restriction to the owning user on Unix (0600).
fn restrict_permissions(path: &Path) {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let _ = fs::set_permissions(path, fs::Permissions::from_mode(0o600));
    }
    let _ = path;
}

#[async_trait::async_trait]
impl KeychainStore for KeychainStoreImpl {
    async fn store_refresh_token(
        &self,
        service: &str,
        account: &str,
        token: &Secret<String>,
    ) -> Result<(), AuthError> {
        if self.uses_keychain() {
            let entry = keyring::Entry::new(service, account).map_err(|e| {
                AuthError::Keychain(format!("keychain entry {service}/{account}: {e}"))
            })?;
            entry.set_password(token.expose()).map_err(|e| {
                AuthError::Keychain(format!("keychain write {service}/{account}: {e}"))
            })
        } else {
            let path = self
                .fallback_path(service, account)
                .ok_or_else(|| AuthError::Keychain("fallback path unavailable".into()))?;
            fs::write(&path, token.expose()).map_err(|e| {
                AuthError::Keychain(format!("fallback write {}: {e}", path.display()))
            })?;
            restrict_permissions(&path);
            Ok(())
        }
    }

    async fn get_refresh_token(
        &self,
        service: &str,
        account: &str,
    ) -> Result<Option<Secret<String>>, AuthError> {
        if self.uses_keychain() {
            let entry = keyring::Entry::new(service, account).map_err(|e| {
                AuthError::Keychain(format!("keychain entry {service}/{account}: {e}"))
            })?;
            match entry.get_password() {
                Ok(password) => Ok(Some(Secret::new(password))),
                Err(keyring::Error::NoEntry) => Ok(None),
                Err(e) => Err(AuthError::Keychain(format!(
                    "keychain read {service}/{account}: {e}"
                ))),
            }
        } else {
            let path = match self.fallback_path(service, account) {
                Some(path) => path,
                None => return Ok(None),
            };
            match fs::read_to_string(&path) {
                Ok(contents) => Ok(Some(Secret::new(contents))),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(AuthError::Keychain(format!(
                    "fallback read {}: {e}",
                    path.display()
                ))),
            }
        }
    }

    async fn delete_refresh_token(&self, service: &str, account: &str) -> Result<(), AuthError> {
        if self.uses_keychain() {
            let entry = keyring::Entry::new(service, account).map_err(|e| {
                AuthError::Keychain(format!("keychain entry {service}/{account}: {e}"))
            })?;
            match entry.delete_credential() {
                Ok(()) => Ok(()),
                // Deleting a missing credential is idempotent.
                Err(keyring::Error::NoEntry) => Ok(()),
                Err(e) => Err(AuthError::Keychain(format!(
                    "keychain delete {service}/{account}: {e}"
                ))),
            }
        } else {
            if let Some(path) = self.fallback_path(service, account) {
                match fs::remove_file(&path) {
                    Ok(()) => {}
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
                    Err(e) => {
                        return Err(AuthError::Keychain(format!(
                            "fallback delete {}: {e}",
                            path.display()
                        )));
                    }
                }
            }
            Ok(())
        }
    }

    fn uses_plaintext_fallback(&self) -> bool {
        self.fallback_dir.is_some()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::auth::infrastructure::keychain_store::{KeychainStore, REFRESH_TOKEN_ACCOUNT};

    const SERVICE: &str = "rigorix";

    #[test]
    fn plaintext_store_round_trip() {
        let dir = tempfile::tempdir().unwrap();
        let store = KeychainStoreImpl::plaintext(dir.path()).unwrap();
        assert!(store.uses_plaintext_fallback());

        let rt = tokio::runtime::Runtime::new().unwrap();
        // Missing credential reads as None.
        let none = rt
            .block_on(store.get_refresh_token(SERVICE, REFRESH_TOKEN_ACCOUNT))
            .unwrap();
        assert!(none.is_none());

        // Store → read back.
        rt.block_on(store.store_refresh_token(
            SERVICE,
            REFRESH_TOKEN_ACCOUNT,
            &Secret::new("crown-jewel-token".into()),
        ))
        .unwrap();
        let got = rt
            .block_on(store.get_refresh_token(SERVICE, REFRESH_TOKEN_ACCOUNT))
            .unwrap()
            .expect("credential present");
        assert_eq!(got.expose(), "crown-jewel-token");

        // Delete → gone (idempotent).
        rt.block_on(store.delete_refresh_token(SERVICE, REFRESH_TOKEN_ACCOUNT))
            .unwrap();
        assert!(
            rt.block_on(store.get_refresh_token(SERVICE, REFRESH_TOKEN_ACCOUNT))
                .unwrap()
                .is_none()
        );
        rt.block_on(store.delete_refresh_token(SERVICE, REFRESH_TOKEN_ACCOUNT))
            .unwrap();
    }

    #[test]
    fn accounts_are_stored_independently() {
        let dir = tempfile::tempdir().unwrap();
        let store = KeychainStoreImpl::plaintext(dir.path()).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(store.store_refresh_token(SERVICE, "issuer-a", &Secret::new("token-a".into())))
            .unwrap();
        rt.block_on(store.store_refresh_token(SERVICE, "issuer-b", &Secret::new("token-b".into())))
            .unwrap();
        assert_eq!(
            rt.block_on(store.get_refresh_token(SERVICE, "issuer-a"))
                .unwrap()
                .unwrap()
                .expose(),
            "token-a"
        );
        rt.block_on(store.delete_refresh_token(SERVICE, "issuer-a"))
            .unwrap();
        assert_eq!(
            rt.block_on(store.get_refresh_token(SERVICE, "issuer-b"))
                .unwrap()
                .unwrap()
                .expose(),
            "token-b",
            "deleting one account never touches another"
        );
    }

    #[test]
    fn sanitize_blocks_path_traversal() {
        // Account strings are URL-ish (issuers) and must never escape the
        // fallback directory via ../ or separators.
        let dir = tempfile::tempdir().unwrap();
        let store = KeychainStoreImpl::plaintext(dir.path()).unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();

        let evil = "../../etc/passwd";
        rt.block_on(store.store_refresh_token(SERVICE, evil, &Secret::new("token-x".into())))
            .unwrap();

        // Nothing was written outside the fallback dir.
        let etc = Path::new("/etc/passwd");
        assert!(!etc.exists() || !std::fs::read_to_string(etc).unwrap().contains("token-x"));

        // And the credential round-trips under its sanitized identity.
        assert_eq!(
            rt.block_on(store.get_refresh_token(SERVICE, evil))
                .unwrap()
                .unwrap()
                .expose(),
            "token-x"
        );
    }

    #[test]
    fn fallback_files_are_restricted_on_unix() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            let dir = tempfile::tempdir().unwrap();
            let store = KeychainStoreImpl::plaintext(dir.path()).unwrap();
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(store.store_refresh_token(
                SERVICE,
                REFRESH_TOKEN_ACCOUNT,
                &Secret::new("secret-value".into()),
            ))
            .unwrap();
            let path = store
                .fallback_path(SERVICE, REFRESH_TOKEN_ACCOUNT)
                .expect("fallback path");
            let mode = fs::metadata(&path).unwrap().permissions().mode();
            assert_eq!(mode & 0o077, 0, "no group/other access: mode {mode:o}");
        }
    }

    #[test]
    fn os_keychain_constructor_works_or_reports_unavailable() {
        // On a desktop with a keychain this succeeds; on headless CI it
        // reports AuthError::Keychain — never panics.
        match KeychainStoreImpl::keychain() {
            Ok(store) => assert!(!store.uses_plaintext_fallback()),
            Err(e) => assert!(matches!(e, AuthError::Keychain(_))),
        }
    }
}
