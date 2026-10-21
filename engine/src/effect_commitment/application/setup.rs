//! EffectCommitmentSetup — composition-root wiring for the commitment seam.
//!
//! @canonical .pi/architecture/modules/effect-commitment.md#compositionroots
//! Implements: ISSUE-EFFECT-COMMITMENT-20 — `from_env` arms the seam
//! Issue: #1133
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
use crate::effect_commitment::application::key_impl::Sha256IntentKeyDerivation;
use crate::effect_commitment::application::seam::DispatchSeam;
use crate::effect_commitment::application::seam_impl::EffectCommitmentSeam;
use crate::effect_commitment::application::service::EffectCommitmentService;
use crate::effect_commitment::application::service_impl::EffectCommitmentServiceImpl;
use crate::effect_commitment::domain::CommitmentError;
use crate::effect_commitment::infrastructure::factory_impl::DefaultEffectCommitmentFactory;
use crate::effect_commitment::interfaces::EffectCommitmentLedger;

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
    /// The factory constructs the ledger and, when declared, the L3 receipt
    /// reader. The service always uses the SHA-256 intent-derived key; a
    /// deployment cannot accidentally run with attempt-derived keys.
    pub async fn from_config(
        config: EffectCommitmentConfig,
        factory: &dyn EffectCommitmentFactory,
    ) -> Result<Self, EffectCommitmentSetupError> {
        let EffectCommitmentConfig {
            backend,
            receipt_reader,
        } = config;

        let ledger: Arc<dyn EffectCommitmentLedger> = Arc::from(factory.create(backend));
        let service: Arc<dyn EffectCommitmentService> = Arc::new(EffectCommitmentServiceImpl::new(
            ledger,
            Arc::new(Sha256IntentKeyDerivation::new()),
        ));
        let receipt_reader = receipt_reader.map(|config| {
            Arc::from(factory.create_receipt_reader(config)) as Arc<dyn ProviderReceiptReader>
        });

        Ok(Self {
            service,
            receipt_reader,
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
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!(
                    "effect-commitment: .rigorix/effect-commitment.toml absent — seam not armed \
                     (fail-open-absent)"
                );
                return Ok(None);
            }
            Err(error) => {
                return Err(EffectCommitmentSetupError::ConfigInvalid {
                    detail: format!(".rigorix/effect-commitment.toml could not be read: {error}"),
                });
            }
        };

        let config = EffectCommitmentConfig::parse(&text)?;
        let setup = Self::from_config(config, &DefaultEffectCommitmentFactory).await?;

        tracing::info!(
            can_prove_commitment = setup.can_prove_commitment(),
            "effect-commitment: dispatch seam ARMED from operator config (ADR-019)"
        );
        Ok(Some(setup))
    }

    /// The dispatch seam over the armed service.
    ///
    /// This is the value the execution-engine factory attaches; it is how the
    /// context stops being dead code.
    pub fn dispatch_seam(&self) -> Arc<dyn DispatchSeam> {
        Arc::new(EffectCommitmentSeam::new(self.service.clone()))
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
