//! `CapacitySetup` — composition-root wiring for the capacity-ledger seam.
//!
//! @canonical .pi/architecture/modules/capacity-ledger.md#enforcementseam
//! ADR-018: distributed capacity ledger
//! Implements: ISSUE-CAPACITY-LEDGER-SEAM — arm the seam from
//!   `.rigorix/capacity.toml` at composition time
//!
//! This module exists for the same reason `PreconditionSetup` does: the port,
//! the adapters and the execution-engine hook can all be present and correct
//! while **nothing ever constructs them**, in which case the capability is dead
//! code and the operator declaration is never read. The engine's own history has
//! this failure twice (`precondition-followups/02-activate-composition-roots`,
//! `03-activate-reachability-test`); this is the bridge that prevents a third.
//!
//! Loading semantics mirror the repository boundary exactly:
//! - **missing file** -> `Ok(None)` — fail-open-absent (status quo, no seam)
//! - **valid file** -> `Ok(Some(setup))` — an armed seam
//! - **malformed / over cap** -> `Err` — fail closed (the caller must refuse,
//!   never silently downgrade)

use std::path::Path;
use std::sync::Arc;

use thiserror::Error;

use crate::capacity_ledger::application::factory::CapacityLedgerFactory;
use crate::capacity_ledger::application::service::CapacityLedgerService;
use crate::capacity_ledger::application::service_impl::CapacityLedgerServiceImpl;
use crate::capacity_ledger::domain::LedgerError;
use crate::capacity_ledger::infrastructure::factory_impl::DefaultCapacityLedgerFactory;

use super::config::{CapacityGrantRule, CapacityLedgerConfig, parse};

/// Failures while arming the seam.
///
/// Deliberately a separate type from the frozen [`LedgerError`]: a declaration
/// that cannot be honoured is a *configuration* fault, and the frozen taxonomy
/// is the runtime contract rather than the setup one.
#[derive(Debug, Error)]
pub enum CapacitySetupError {
    /// The declaration is present but cannot be honoured.
    #[error("capacity config invalid: {detail}")]
    ConfigInvalid {
        /// What is wrong, without absolute paths.
        detail: String,
    },

    /// The declaration is valid but the adapter could not be made usable.
    #[error("capacity ledger unavailable: {detail}")]
    Unavailable {
        /// What went wrong.
        detail: String,
    },
}

impl From<LedgerError> for CapacitySetupError {
    fn from(error: LedgerError) -> Self {
        Self::Unavailable {
            detail: error.to_string(),
        }
    }
}

/// An armed capacity seam: the service plus the compiled step/claim rules.
///
/// `Clone` so it can travel inside `ParallelExecutionFactoryConfig`, matching
/// the precondition setup.
#[derive(Clone)]
pub struct CapacitySetup {
    /// The application service over the selected adapter.
    pub service: Arc<dyn CapacityLedgerService>,
    /// The step/claim rules, compiled from the declaration.
    pub rules: Arc<Vec<CapacityGrantRule>>,
}

impl CapacitySetup {
    /// Build a seam from an already-parsed config.
    ///
    /// Declares every resource on the adapter before returning, so a step can
    /// never be governed by a resource the ledger does not know about.
    pub async fn from_config(
        config: CapacityLedgerConfig,
        factory: &dyn CapacityLedgerFactory,
    ) -> Result<Self, CapacitySetupError> {
        let ledger = factory.create(config.backend);
        for spec in &config.resources {
            ledger.declare_resource(spec.clone()).await?;
        }
        let service: Arc<dyn CapacityLedgerService> =
            Arc::new(CapacityLedgerServiceImpl::new(Arc::from(ledger)));
        Ok(Self {
            service,
            rules: Arc::new(config.grants),
        })
    }

    /// Build the seam from the operator declaration in `.rigorix/capacity.toml`.
    ///
    /// * `Ok(None)` — no declaration: no seam (fail-open-absent, status quo).
    /// * `Ok(Some(setup))` — declaration valid: an armed seam.
    /// * `Err(CapacitySetupError)` — declaration present but unusable; the
    ///   caller must fail closed.
    pub async fn from_env(repo_root: &Path) -> Result<Option<Self>, CapacitySetupError> {
        let path = repo_root.join(".rigorix").join("capacity.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                tracing::debug!(
                    "capacity: .rigorix/capacity.toml absent — seam not armed (fail-open-absent)"
                );
                return Ok(None);
            }
            Err(error) => {
                // No absolute paths in any recorded artifact.
                return Err(CapacitySetupError::ConfigInvalid {
                    detail: format!(".rigorix/capacity.toml could not be read: {error}"),
                });
            }
        };

        let config = parse(&text)?;
        let setup = Self::from_config(config, &DefaultCapacityLedgerFactory).await?;

        tracing::info!(
            rules = setup.rules.len(),
            "capacity: dispatch seam ARMED from operator config (ADR-018)"
        );
        Ok(Some(setup))
    }

    /// The rule governing a dispatch, if any.
    pub fn rule_for(&self, step_name: &str, tool: &str) -> Option<&CapacityGrantRule> {
        self.rules.iter().find(|r| r.matches(step_name, tool))
    }

    /// Whether the seam will govern anything.
    pub fn is_armed(&self) -> bool {
        !self.rules.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capacity_ledger::application::dto::ReserveCapacityInput;
    use crate::capacity_ledger::domain::{IdempotencyKey, ResourceId};
    use std::path::PathBuf;

    /// A unique scratch directory. Avoids depending on a temp-file crate.
    fn scratch(tag: &str) -> PathBuf {
        let mut dir = std::env::temp_dir();
        dir.push(format!(
            "rigorix-capacity-setup-{tag}-{}-{}",
            std::process::id(),
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir_all(dir.join(".rigorix")).expect("scratch dir");
        dir
    }

    #[tokio::test]
    async fn missing_file_is_fail_open_absent() {
        let dir = scratch("absent");
        let setup = CapacitySetup::from_env(&dir).await.unwrap();
        assert!(setup.is_none(), "absent declaration must not arm anything");
    }

    #[tokio::test]
    async fn malformed_file_is_fail_closed() {
        let dir = scratch("malformed");
        std::fs::write(dir.join(".rigorix/capacity.toml"), "not = = toml").unwrap();
        let result = CapacitySetup::from_env(&dir).await;
        assert!(
            result.is_err(),
            "a malformed declaration must refuse to arm"
        );
    }

    #[tokio::test]
    async fn valid_file_arms_and_declares_resources() {
        let dir = scratch("valid");
        std::fs::write(
            dir.join(".rigorix/capacity.toml"),
            r#"
[[resources]]
id = "r"
hard_limit = 10

[[grants]]
step = "s"
resource = "r"
amount = 4
"#,
        )
        .unwrap();

        let setup = CapacitySetup::from_env(&dir).await.unwrap().expect("armed");
        assert!(setup.is_armed());
        assert!(setup.rule_for("s", "any-tool").is_some());

        // The resource was declared on the adapter, so a reserve can succeed.
        let out = setup
            .service
            .reserve(ReserveCapacityInput {
                resource: ResourceId::new("r"),
                amount: 4,
                idempotency_key: IdempotencyKey::new("k"),
                requested_by: None,
            })
            .await
            .expect("declared resource must be reservable");
        assert_eq!(out.reservation.amount, 4);
    }
}
