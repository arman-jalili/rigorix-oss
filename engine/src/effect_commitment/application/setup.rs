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
use crate::effect_commitment::application::reconcile::EffectReconciler;
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

/// The guarantee this setup can actually prove.
///
/// The ceiling is derived from what was constructed, never declared by an
/// operator (ADR-019). In particular, L3 requires a provider receipt reader:
/// without one the context can prove **not-settled**, never **settled**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectCommitmentCeiling {
    /// In-process ledger: exact within this process, lost on restart.
    L1,
    /// Shared ledger: survives process death; no provider read-back.
    L2,
    /// Provider receipt read-back is armed: can prove committed.
    L3,
}

impl EffectCommitmentCeiling {
    /// Derive the ceiling from the constructed adapters.
    ///
    /// Provider evidence is what upgrades L2 to L3; the absence of a shared
    /// ledger caps the claim at L1 even when a receipt reader is configured,
    /// because the record itself does not survive the process.
    pub fn derive(shared_ledger: bool, provider_evidence: bool) -> Self {
        match (shared_ledger, provider_evidence) {
            (_, true) => Self::L3,
            (true, false) => Self::L2,
            (false, false) => Self::L1,
        }
    }

    /// The short label used by `rigorix doctor` and the runbook.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::L1 => "L1",
            Self::L2 => "L2",
            Self::L3 => "L3",
        }
    }

    /// One sentence an operator can act on.
    pub fn description(self) -> &'static str {
        match self {
            Self::L1 => {
                "in-process record only; survives neither restart nor a second process, \
                 and cannot prove a commitment"
            }
            Self::L2 => {
                "shared ledger survives process death; without a receipt reader it can prove \
                 not-settled only"
            }
            Self::L3 => {
                "provider receipt read-back is armed; the only ceiling that can prove committed"
            }
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

    /// The startup sweep for records orphaned by a crash (#1164).
    reconciler: Arc<EffectReconciler>,

    /// The guarantee this setup can actually prove, derived at construction.
    ceiling: EffectCommitmentCeiling,
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

        let shared_ledger = matches!(
            backend,
            crate::effect_commitment::application::factory::EffectCommitmentBackendConfig::Http { .. }
        );
        let provider_evidence = receipt_reader.is_some();
        let ceiling = EffectCommitmentCeiling::derive(shared_ledger, provider_evidence);

        let ledger: Arc<dyn EffectCommitmentLedger> = Arc::from(factory.create(backend));
        let reconciler = Arc::new(EffectReconciler::with_default_lease(ledger.clone()));
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
            reconciler,
            ceiling,
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
        // #1166: fail at startup, not at first use. A config pointing at a
        // service that does not exist is a declaration that cannot be honoured.
        validate_backend_reachable(&config.backend).await?;
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

    /// The guarantee ceiling this setup was actually built to provide.
    ///
    /// Derived from the constructed adapters (#1176), so a report cannot claim
    /// more than the code can do.
    pub fn ceiling(&self) -> EffectCommitmentCeiling {
        self.ceiling
    }

    /// Whether this process owns the `Dispatched` sweep (L1) or the service does (L2).
    ///
    /// At L1 the record and the clock live in this process, so the startup sweep
    /// is the only thing that closes the crash window. At L2 the store is shared,
    /// so a client sweep would compare the writer's timestamps against its own
    /// clock; the service owns it (#1173).
    pub fn sweeps_dispatched(&self) -> bool {
        self.reconciler.can_sweep()
    }

    /// Sweep `Dispatched` records orphaned by a crash to `Ambiguous` (#1164).
    ///
    /// Composition roots call this exactly once at startup, after
    /// [`EffectCommitmentSetup::from_env`] arms the seam. A failure is
    /// fail-closed: starting with an unknown backlog would let a resumed run
    /// retry an effect that may already have happened.
    pub async fn reconcile(&self) -> Result<usize, EffectCommitmentSetupError> {
        let swept = self.reconciler.reconcile().await?;
        if swept > 0 {
            tracing::warn!(
                swept,
                lease_secs = self.reconciler.lease().as_secs(),
                "effect-commitment: startup sweep moved expired Dispatched records to Ambiguous"
            );
        } else {
            tracing::debug!("effect-commitment: startup sweep found no expired Dispatched records");
        }
        Ok(swept)
    }
}

/// Probe the declared backend before the run starts (#1166).
///
/// The OSS repository ships the HTTP **client** but not the ledger service, so
/// a `[backend] kind = "http"` declaration that points at nothing would fail
/// hours into a run. A TCP connect is enough to distinguish "nothing is
/// listening" from "a service is there"; the first use still fails closed on
/// any protocol error.
async fn validate_backend_reachable(
    backend: &crate::effect_commitment::application::factory::EffectCommitmentBackendConfig,
) -> Result<(), EffectCommitmentSetupError> {
    use crate::effect_commitment::application::factory::EffectCommitmentBackendConfig;

    let EffectCommitmentBackendConfig::Http {
        base_url,
        timeout_ms,
        ..
    } = backend
    else {
        return Ok(());
    };

    let url = reqwest::Url::parse(base_url).map_err(|error| {
        EffectCommitmentSetupError::ConfigInvalid {
            detail: format!("effect-commitment backend base_url is not a URL: {error}"),
        }
    })?;
    let host = url
        .host_str()
        .ok_or_else(|| EffectCommitmentSetupError::ConfigInvalid {
            detail: "effect-commitment backend base_url has no host".to_string(),
        })?;
    let port =
        url.port_or_known_default()
            .ok_or_else(|| EffectCommitmentSetupError::ConfigInvalid {
                detail: "effect-commitment backend base_url has no port".to_string(),
            })?;
    let probe_timeout = std::time::Duration::from_millis((*timeout_ms).min(5_000));

    match tokio::time::timeout(probe_timeout, tokio::net::TcpStream::connect((host, port))).await {
        Ok(Ok(_)) => Ok(()),
        Ok(Err(error)) => Err(EffectCommitmentSetupError::Unavailable {
            detail: format!(
                "effect-commitment backend at {base_url} is unreachable: {error}.                  OSS ships only the in-process L1 adapter; L2 requires a service implementing                  the frozen /v1/effects contract (enterprise ADR-011). See                  docs/runbook-effect-commitment.md."
            ),
        }),
        Err(_) => Err(EffectCommitmentSetupError::Unavailable {
            detail: format!(
                "effect-commitment backend at {base_url} did not accept a connection within                  {}ms. OSS ships only the in-process L1 adapter; L2 requires the enterprise                  ledger service (ADR-011).",
                probe_timeout.as_millis()
            ),
        }),
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

#[cfg(test)]
mod tests {
    use super::EffectCommitmentCeiling;

    #[test]
    fn the_ceiling_is_derived_from_the_constructed_adapters() {
        // No shared ledger, no provider evidence: the in-process default.
        assert_eq!(
            EffectCommitmentCeiling::derive(false, false),
            EffectCommitmentCeiling::L1
        );
        // A shared ledger upgrades durability, not provability.
        assert_eq!(
            EffectCommitmentCeiling::derive(true, false),
            EffectCommitmentCeiling::L2
        );
        // Provider evidence is what lets the context say "committed".
        assert_eq!(
            EffectCommitmentCeiling::derive(false, true),
            EffectCommitmentCeiling::L3
        );
        assert_eq!(
            EffectCommitmentCeiling::derive(true, true),
            EffectCommitmentCeiling::L3
        );
    }
}
