//! `AnchoredHistoryAdapter` — ADR-016 Phase C history over the anchor's signed
//! projection.
//!
//! @canonical .pi/architecture/decisions/ADR-016-audit-integrity-anchor.md
//! Issue: #899 (OSS-AUDIT-C)
//!
//! This is an **adapter swap behind the `ExecutionHistory` port** — the matcher
//! is unchanged. It fetches the anchor-signed history slice and verifies the
//! Ed25519 signature *before* returning any action, so the matcher never sees
//! unverified evidence. It reports [`ExecutionHistory::is_anchored`] as `true`
//! only because every read is signature-gated; an unreachable anchor or an
//! invalid slice is an error, which the `SequencePolicyServiceImpl` fail-closes
//! on for consequential (deny-class / history-dependent) runs.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use crate::audit::domain::anchor::HistoryActionRef;
use crate::audit::infrastructure::anchor::AnchorRuntime;
use crate::sequence_policy::domain::{HistoryAction, SequencePolicyError};

use super::history::ExecutionHistory;

/// History adapter that reads the anchor-signed projection.
pub struct AnchoredHistoryAdapter {
    runtime: Arc<AnchorRuntime>,
}

impl AnchoredHistoryAdapter {
    /// Create the adapter over the shared anchor runtime.
    pub fn new(runtime: Arc<AnchorRuntime>) -> Self {
        Self { runtime }
    }

    /// The shared runtime (for the composition root / `system.version`).
    pub fn runtime(&self) -> &Arc<AnchorRuntime> {
        &self.runtime
    }
}

impl From<HistoryActionRef> for HistoryAction {
    fn from(reference: HistoryActionRef) -> Self {
        Self {
            node: reference.node,
            principal: reference.principal,
            at: reference.at,
            effect_key: reference.effect_key,
        }
    }
}

#[async_trait]
impl ExecutionHistory for AnchoredHistoryAdapter {
    async fn prior_actions(
        &self,
        since: DateTime<Utc>,
    ) -> Result<Vec<HistoryAction>, SequencePolicyError> {
        // Verify-before-match: any failure is an error, which the caller
        // fail-closes on (ADR-016 Phase C — never hand unverified facts to the
        // matcher).
        let actions = self
            .runtime
            .fetch_verified(since)
            .await
            .map_err(|e| SequencePolicyError::Internal(format!("anchored history: {e}")))?;
        Ok(actions.into_iter().map(HistoryAction::from).collect())
    }

    fn is_anchored(&self) -> bool {
        // Every read that yields actions was signature-verified; an
        // unverifiable read returns `Err` rather than empty history.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audit::domain::anchor::HistorySlice;
    use ed25519_dalek::{Signer, SigningKey};

    /// Test source: returns a pre-signed slice, or a configured failure.
    struct MockSource {
        slice: Option<HistorySlice>,
    }

    #[async_trait]
    impl crate::audit::infrastructure::anchor::AnchorSliceSource for MockSource {
        async fn fetch_slice(
            &self,
            _scope: &str,
            _since: DateTime<Utc>,
        ) -> Result<HistorySlice, crate::audit::infrastructure::anchor::AnchorError> {
            self.slice.clone().ok_or_else(|| {
                crate::audit::infrastructure::anchor::AnchorError::Unreachable(
                    "connection refused".into(),
                )
            })
        }
    }

    fn key() -> SigningKey {
        SigningKey::from_bytes(&[7u8; 32])
    }

    fn signed_runtime(actions: Vec<HistoryActionRef>, head: &str) -> Arc<AnchorRuntime> {
        let signing = key();
        let mut slice = HistorySlice {
            scope: "local".into(),
            since: Utc::now(),
            actions,
            head_hash: head.into(),
            sig: None,
        };
        slice.sig = Some(hex::encode(
            signing.sign(&slice.signing_bytes().unwrap()).to_bytes(),
        ));
        AnchorRuntime::anchored(
            Arc::new(MockSource { slice: Some(slice) }),
            hex::encode(signing.verifying_key().to_bytes()),
            "test-anchor",
            "local",
        )
    }

    #[tokio::test]
    async fn valid_slice_yields_actions_and_records_head() {
        let runtime = signed_runtime(
            vec![HistoryActionRef {
                node: "remove".into(),
                principal: Some("alice".into()),
                at: Utc::now(),
                effect_key: None,
            }],
            &"ab".repeat(32),
        );
        let adapter = AnchoredHistoryAdapter::new(runtime.clone());
        assert!(adapter.is_anchored());
        let actions = adapter.prior_actions(Utc::now()).await.unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].node, "remove");
        assert_eq!(runtime.head(), Some("ab".repeat(32)));
    }

    #[tokio::test]
    async fn unreachable_anchor_fails_closed() {
        let signing = key();
        let runtime = AnchorRuntime::anchored(
            Arc::new(MockSource { slice: None }),
            hex::encode(signing.verifying_key().to_bytes()),
            "test-anchor",
            "local",
        );
        let adapter = AnchoredHistoryAdapter::new(runtime);
        assert!(adapter.prior_actions(Utc::now()).await.is_err());
    }

    #[tokio::test]
    async fn forged_slice_fails_closed() {
        let signing = key();
        let mut slice = HistorySlice {
            scope: "local".into(),
            since: Utc::now(),
            actions: vec![],
            head_hash: "ab".repeat(32),
            sig: None,
        };
        slice.sig = Some(hex::encode(
            signing.sign(&slice.signing_bytes().unwrap()).to_bytes(),
        ));
        // Tamper after signing.
        slice.head_hash = "ff".repeat(32);
        let runtime = AnchorRuntime::anchored(
            Arc::new(MockSource { slice: Some(slice) }),
            hex::encode(signing.verifying_key().to_bytes()),
            "test-anchor",
            "local",
        );
        let adapter = AnchoredHistoryAdapter::new(runtime);
        assert!(adapter.prior_actions(Utc::now()).await.is_err());
    }
}
