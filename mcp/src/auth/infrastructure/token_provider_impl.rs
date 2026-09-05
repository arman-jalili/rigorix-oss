//! InMemoryTokenProvider — short-TTL access-token custody.
//!
//! @canonical .pi/architecture/modules/auth.md#tokenprovider-infrastructure
//! Implements: ISSUE-AUTH-4 — TokenProvider (Infrastructure)
//! Issue: #824
//! ADR-008: access tokens are short-TTL (5–15 min) and in-memory only
//!
//! Concrete implementation of the frozen [`TokenProvider`] port: a
//! thread-safe in-memory cell holding the current short-TTL access token
//! plus its absolute expiry.
//!
//! # Contract (Frozen)
//!
//! - Never persists access tokens to disk
//! - A token past its recorded expiry is **hidden** (`current_access_token`
//!   returns `None`) — the service silently refreshes via the keychain
//!   refresh token instead of serving a dead token
//! - Secrets stay wrapped in `Secret<String>` — redacted everywhere
//! - Locks are never held across `.await` (std Mutex suffices)

use chrono::{DateTime, Utc};

use crate::auth::domain::error::AuthError;
use crate::auth::domain::value::Secret;
use crate::auth::infrastructure::token_provider::TokenProvider;

/// Thread-safe in-memory access-token store.
#[derive(Debug, Default)]
pub struct InMemoryTokenProvider {
    /// Cached access token and its absolute expiry (`None` = empty).
    state: std::sync::Mutex<Option<(Secret<String>, DateTime<Utc>)>>,
}

impl InMemoryTokenProvider {
    /// Create an empty provider.
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait::async_trait]
impl TokenProvider for InMemoryTokenProvider {
    async fn current_access_token(&self) -> Option<Secret<String>> {
        let guard = self.state.lock().ok()?;
        let (token, expires_at) = guard.as_ref()?;
        // Expired tokens are never served as usable (TokenProvider contract).
        if *expires_at <= Utc::now() {
            return None;
        }
        Some(token.clone())
    }

    async fn set_access_token(
        &self,
        token: Secret<String>,
        expires_at: DateTime<Utc>,
    ) -> Result<(), AuthError> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| AuthError::Internal("token provider lock poisoned".into()))?;
        *guard = Some((token, expires_at));
        Ok(())
    }

    async fn access_token_expires_at(&self) -> Option<DateTime<Utc>> {
        self.state
            .lock()
            .ok()
            .and_then(|guard| guard.as_ref().map(|(_, at)| *at))
    }

    async fn clear(&self) {
        if let Ok(mut guard) = self.state.lock() {
            *guard = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration as ChronoDuration;

    fn future(secs: i64) -> DateTime<Utc> {
        Utc::now() + ChronoDuration::seconds(secs)
    }

    fn past(secs: i64) -> DateTime<Utc> {
        Utc::now() - ChronoDuration::seconds(secs)
    }

    #[tokio::test]
    async fn starts_empty() {
        let provider = InMemoryTokenProvider::new();
        assert!(provider.current_access_token().await.is_none());
        assert!(provider.access_token_expires_at().await.is_none());
    }

    #[tokio::test]
    async fn set_then_get_round_trip() {
        let provider = InMemoryTokenProvider::new();
        provider
            .set_access_token(Secret::new("access-token".into()), future(900))
            .await
            .unwrap();
        let token = provider.current_access_token().await.unwrap();
        assert_eq!(token.expose(), "access-token");
        assert!(provider.access_token_expires_at().await.unwrap() > Utc::now());
        // No token material renders.
        assert!(!format!("{token:?}").contains("access-token"));
    }

    #[tokio::test]
    async fn expired_token_is_hidden_not_served() {
        let provider = InMemoryTokenProvider::new();
        // Contract: a token past its recorded expiry is not usable — the
        // AuthService silently refreshes instead.
        provider
            .set_access_token(Secret::new("dead-token".into()), past(30))
            .await
            .unwrap();
        assert!(provider.current_access_token().await.is_none());
        // …but the expiry metadata is still visible so status() can report
        // TokenStatus::Expired.
        let expires_at = provider.access_token_expires_at().await.unwrap();
        assert!(expires_at <= Utc::now());
    }

    #[tokio::test]
    async fn overwrite_replaces_token() {
        let provider = InMemoryTokenProvider::new();
        provider
            .set_access_token(Secret::new("first".into()), future(900))
            .await
            .unwrap();
        provider
            .set_access_token(Secret::new("second".into()), future(900))
            .await
            .unwrap();
        let token = provider.current_access_token().await.unwrap();
        assert_eq!(token.expose(), "second");
    }

    #[tokio::test]
    async fn clear_removes_token_and_expiry() {
        let provider = InMemoryTokenProvider::new();
        provider
            .set_access_token(Secret::new("access-token".into()), future(900))
            .await
            .unwrap();
        provider.clear().await;
        assert!(provider.current_access_token().await.is_none());
        assert!(provider.access_token_expires_at().await.is_none());
    }

    #[test]
    fn provider_is_send_sync_object_safe() {
        fn _assert<T: Send + Sync + ?Sized>() {}
        _assert::<InMemoryTokenProvider>();
        _assert::<dyn TokenProvider>();
    }
}
