//! Integration tests for KeychainStoreImpl (Infrastructure) — ISSUE-AUTH-3.
//!
//! Exercises the REAL implementations:
//! - Plaintext fallback (explicit opt-in degraded mode) against the real
//!   filesystem — persistence across instances, Unix 0600 restriction
//! - OS keychain (`keyring`) when a platform store is available (skips
//!   gracefully on headless CI)
//!
//! @canonical .pi/architecture/modules/auth.md#keychainstore-infrastructure
//! Implements: ISSUE-AUTH-3 — custody contract (ADR-008)

use rigorix_mcp::auth::domain::value::Secret;
use rigorix_mcp::auth::infrastructure::KeychainStoreImpl;
use rigorix_mcp::auth::infrastructure::keychain_store::KeychainStore;

/// A unique service name per test run keeps the OS keychain clean.
fn unique_service(tag: &str) -> String {
    format!("rigorix-test-{tag}-{}", uuid::Uuid::new_v4())
}

/// Plaintext fallback: credentials persist across store instances and are
/// never readable by group/other on Unix.
#[test]
fn plaintext_fallback_persists_across_instances() {
    let dir = tempfile::tempdir().unwrap();
    let service = unique_service("plain");

    // Instance 1 writes.
    let store = KeychainStoreImpl::plaintext(dir.path()).unwrap();
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(store.store_refresh_token(
        &service,
        "refresh_token",
        &Secret::new("crown-jewel".into()),
    ))
    .unwrap();

    // Instance 2 (a fresh store over the same dir) reads it back.
    let reopened = KeychainStoreImpl::plaintext(dir.path()).unwrap();
    let got = rt
        .block_on(reopened.get_refresh_token(&service, "refresh_token"))
        .unwrap()
        .expect("credential persisted across instances");
    assert_eq!(got.expose(), "crown-jewel");
    assert!(reopened.uses_plaintext_fallback());

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let dir_path = dir.path();
        let name = format!("{}_refresh_token.tok", sanitize_service(&service));
        let path = dir_path.join(name);
        let mode = std::fs::metadata(path).unwrap().permissions().mode();
        assert_eq!(
            mode & 0o077,
            0,
            "fallback file must be user-only (mode {mode:o})"
        );
    }

    rt.block_on(reopened.delete_refresh_token(&service, "refresh_token"))
        .unwrap();
}

/// Mirror of the production sanitizer for the file-name assertion above.
fn sanitize_service(service: &str) -> String {
    service
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect()
}

/// OS keychain round trip when a platform store is available; headless CI
/// skips gracefully (constructor reports `AuthError::Keychain`).
#[tokio::test]
async fn os_keychain_round_trip_when_available() {
    let store = match KeychainStoreImpl::keychain() {
        Ok(store) => store,
        Err(_) => {
            // Headless CI (no Secret Service / Keychain): the fallback path is
            // covered by the plaintext tests above.
            eprintln!("skipping OS keychain test — platform store unavailable");
            return;
        }
    };
    let service = unique_service("os");

    store
        .store_refresh_token(&service, "refresh_token", &Secret::new("os-secret".into()))
        .await
        .unwrap();
    let got = store
        .get_refresh_token(&service, "refresh_token")
        .await
        .unwrap()
        .expect("credential stored in OS keychain");
    assert_eq!(got.expose(), "os-secret");
    assert!(!store.uses_plaintext_fallback());

    store
        .delete_refresh_token(&service, "refresh_token")
        .await
        .unwrap();
    assert!(
        store
            .get_refresh_token(&service, "refresh_token")
            .await
            .unwrap()
            .is_none()
    );
}
