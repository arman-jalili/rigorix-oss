//! EffectCommitmentSetup — composition-root wiring for the commitment seam.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#compositionroots
//! Implements: ISSUE-CONTRACT-FREEZE — `from_env` composition contract
//! Issue: #1113
//! ADR-019: arm the seam from `.rigorix/effect-commitment.toml`
//!
//! The capacity ledger was once dead code because no composition root
//! constructed an adapter. This type exists so that failure class is visible in
//! the architecture rather than discovered in a demo: `cli`, `mcp`, and
//! `actions` each call [`EffectCommitmentSetup::from_env`] exactly where
//! `CapacitySetup::from_env` is wired today.
//!
//! Loading semantics mirror the repository boundary:
//! - **missing file** → `Ok(None)` — fail-open-absent (status quo, no seam)
//! - **valid file** → `Ok(Some(setup))` — an armed seam
//! - **malformed / present but unhonourable** → `Err` — fail closed; the caller
//!   must refuse, never silently downgrade

use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use thiserror::Error;

use crate::effect_commitment::application::factory::EffectCommitmentFactory;
use crate::effect_commitment::application::service::EffectCommitmentService;
use crate::effect_commitment::domain::CommitmentError;

use super::config::EffectCommitmentConfig;
use super::receipt::ProviderReceiptReader;

/// Failures while arming the seam.
///
/// Deliberately a separate type from the frozen [`CommitmentError`]: a
/// declaration that cannot be honoured is a *configuration* fault, while the
/// frozen taxonomy is the runtime contract.
#[derive(Debug, Error)]
pub enum EffectCommitmentSetupError {
    /// The declaration is present but cannot be honoured.
    #[error("effect-commitment config invalid: {detail}")]
    ConfigInvalid {
        /// What is wrong, without absolute paths.
        detail: String,
    },

    /// The declaration is valid but the adapter could not be made usable.
    #[error("effect-commitment ledger unavailable: {detail}")]
    Unavailable {
        /// What went wrong.
        detail: String,
    },
}

impl From<CommitmentError> for EffectCommitmentSetupError {
    fn from(error: CommitmentError) -> Self {
        Self::Unavailable {
            detail: error.to_string(),
        }
    }
}

/// An armed effect-commitment seam: the service plus the optional receipt reader.
///
/// `Clone` so it can travel inside the execution factory config, matching the
/// capacity setup.
#[derive(Clone)]
pub struct EffectCommitmentSetup {
    /// The application service over the selected adapter.
    pub service: Arc<dyn EffectCommitmentService>,

    /// The optional L3 provider read-back adapter.
    pub receipt_reader: Option<Arc<dyn ProviderReceiptReader>>,
}

impl EffectCommitmentSetup {
    /// Build a seam from an already-parsed config.
    ///
    /// The signature is frozen; ISSUE-EFFECT-COMMITMENT-14 supplies the concrete
    /// construction path. Until then a declaration fails closed rather than
    /// yielding a seam that does not enforce anything.
    pub async fn from_config(
        _config: EffectCommitmentConfig,
        _factory: &dyn EffectCommitmentFactory,
    ) -> Result<Self, EffectCommitmentSetupError> {
        Err(EffectCommitmentSetupError::Unavailable {
            detail:
                "ISSUE-EFFECT-COMMITMENT-14: effect-commitment composition is not implemented yet"
                    .to_string(),
        })
    }

    /// Build the seam from `.rigorix/effect-commitment.toml`.
    ///
    /// * `Ok(None)` — no declaration: no seam (fail-open-absent, status quo).
    /// * `Ok(Some(setup))` — declaration valid: an armed seam.
    /// * `Err(EffectCommitmentSetupError)` — declaration present but unusable;
    ///   the caller must fail closed.
    pub async fn from_env(repo_root: &Path) -> Result<Option<Self>, EffectCommitmentSetupError> {
        let path = repo_root.join(".rigorix").join("effect-commitment.toml");
        match std::fs::read_to_string(&path) {
            Ok(_text) => {
                // A declaration exists, so the operator asked for the guarantee.
                // Refuse to pretend it is armed until the concrete adapters and
                // parser land.
                Err(EffectCommitmentSetupError::Unavailable {
                    detail: "ISSUE-EFFECT-COMMITMENT-13/14: a declaration is present but the \
                             composition path is not implemented yet"
                        .to_string(),
                })
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!(
                    "effect-commitment: .rigorix/effect-commitment.toml absent — seam not armed \
                     (fail-open-absent)"
                );
                Ok(None)
            }
            Err(error) => Err(EffectCommitmentSetupError::ConfigInvalid {
                detail: format!(".rigorix/effect-commitment.toml could not be read: {error}"),
            }),
        }
    }

    /// Whether provider evidence is available (L3).
    pub fn can_prove_commitment(&self) -> bool {
        self.receipt_reader.is_some()
    }
}

/// The composition-root contract implemented by `cli`, `mcp`, and `actions`.
///
/// Each root calls this at startup, exactly where `CapacitySetup::from_env` is
/// wired. A root that does not implement it cannot claim the guarantee.
#[async_trait]
pub trait EffectCommitmentCompositionRoot: Send + Sync {
    /// Arm the seam for `repo_root`, if the operator declared one.
    async fn effect_commitment_setup(
        &self,
        repo_root: &Path,
    ) -> Result<Option<EffectCommitmentSetup>, EffectCommitmentSetupError>;
}
