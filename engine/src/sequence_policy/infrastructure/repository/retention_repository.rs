//! RetentionCoupledSequencePolicyRepository — enforce ADR-014's retention
//! coupling on the real config-load path.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#configuration
//! @canonical .pi/architecture/decisions/ADR-014-effect-identity-matching.md
//! Implements: GAP-A-29 (issue #895) — call
//!   `SequencePolicyConfig::validate_retention` **fail-closed** when an audit
//!   retention window is configured, so an effect-keyed rule whose window
//!   outlives it refuses evaluation rather than silently degrading.
//!
//! ADR-014's retention coupling is load-bearing: pruning the signed trail
//! below the longest effect-keyed rule window drops the effect-key evidence
//! those rules depend on, silently disabling them. This decorator **reuses**
//! the existing domain validator ([`SequencePolicyConfig::validate_retention`])
//! — it adds no parallel check — and applies it uniformly to every wrapped
//! source (local TOML, enterprise bundle, or their precedence composition),
//! because it wraps the composed repository the service reads through.
//!
//! # Configuration
//! The retention window is `RIGORIX_AUDIT_RETENTION_SECS` (seconds):
//!
//! | Value | Meaning |
//! |-------|---------|
//! | unset / empty | unlimited retention → every window valid (status quo) |
//! | a non-negative integer | that many seconds; an effect-keyed window longer than it fails closed |
//! | anything else | fail closed — a configured control must not silently degrade to unlimited |
//!
//! `None` (unlimited) is always valid; the check only ever refuses when a
//! retention window is configured and an effect-keyed rule outlives it.

use async_trait::async_trait;

use crate::sequence_policy::domain::{SequencePolicyConfig, SequencePolicyError};

use super::SequencePolicyRepository;

/// Environment variable carrying the audit retention window, in seconds.
///
/// Unset/empty means unlimited retention (status quo). Reading is
/// composition-time and explicit; the value is passed to the reused
/// [`SequencePolicyConfig::validate_retention`] on every load.
pub const AUDIT_RETENTION_ENV: &str = "RIGORIX_AUDIT_RETENTION_SECS";

/// The audit-retention setting resolved once at composition.
#[derive(Debug, Clone, PartialEq, Eq)]
enum AuditRetention {
    /// No retention configured — unlimited; every window is valid.
    Unlimited,
    /// A configured retention window in seconds.
    Secs(u64),
    /// The environment value was present but unusable — every load fails
    /// closed (a misconfigured control must not silently become unlimited).
    Invalid(String),
}

impl AuditRetention {
    /// Parse the raw env value (`None` = unset).
    fn parse(raw: Option<&str>) -> Self {
        match raw {
            None => Self::Unlimited,
            Some(v) if v.trim().is_empty() => Self::Unlimited,
            Some(v) => match v.trim().parse::<u64>() {
                Ok(secs) => Self::Secs(secs),
                Err(_) => Self::Invalid(format!(
                    "{AUDIT_RETENTION_ENV}={v:?} is not a non-negative integer of seconds"
                )),
            },
        }
    }

    /// The `Option<u64>` the domain validator takes (`None` = unlimited).
    const fn as_retention_secs(&self) -> Option<u64> {
        match self {
            Self::Secs(secs) => Some(*secs),
            Self::Unlimited | Self::Invalid(_) => None,
        }
    }
}

/// Decorator enforcing ADR-014's retention coupling on any
/// [`SequencePolicyRepository`] load.
///
/// Wraps the composed source (local TOML / enterprise bundle / precedence)
/// so the retention check runs on the SAME `SequencePolicyConfig` the
/// service evaluates — never on a re-parsed copy.
pub struct RetentionCoupledSequencePolicyRepository {
    inner: Box<dyn SequencePolicyRepository>,
    retention: AuditRetention,
}

impl RetentionCoupledSequencePolicyRepository {
    /// Wrap `inner`, enforcing the explicit `retention_secs`
    /// (`None` = unlimited retention).
    pub fn new(inner: Box<dyn SequencePolicyRepository>, retention_secs: Option<u64>) -> Self {
        Self {
            inner,
            retention: match retention_secs {
                Some(secs) => AuditRetention::Secs(secs),
                None => AuditRetention::Unlimited,
            },
        }
    }

    /// Wrap `inner`, reading [`AUDIT_RETENTION_ENV`] once at composition.
    pub fn from_env(inner: Box<dyn SequencePolicyRepository>) -> Self {
        let retention = AuditRetention::parse(std::env::var(AUDIT_RETENTION_ENV).ok().as_deref());
        match &retention {
            AuditRetention::Secs(secs) => tracing::info!(
                retention_secs = secs,
                "sequence_policy: ADR-014 retention coupling armed — an effect-keyed rule window \
                 longer than {secs}s will fail closed"
            ),
            AuditRetention::Unlimited => tracing::debug!(
                "sequence_policy: {AUDIT_RETENTION_ENV} unset — unlimited audit retention \
                 (retention coupling is a no-op, status quo)"
            ),
            AuditRetention::Invalid(reason) => tracing::error!(
                "sequence_policy: {reason} — sequence-policy config will fail closed until fixed"
            ),
        }
        Self { inner, retention }
    }

    /// The configured retention window (`None` = unlimited), for composition
    /// logging and tests.
    pub const fn retention_secs(&self) -> Option<u64> {
        self.retention.as_retention_secs()
    }
}

#[async_trait]
impl SequencePolicyRepository for RetentionCoupledSequencePolicyRepository {
    async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
        // A configured-but-unusable retention value fails closed before the
        // inner source is even read — never a silent degrade to unlimited.
        if let AuditRetention::Invalid(reason) = &self.retention {
            return Err(SequencePolicyError::InvalidConfig(reason.clone()));
        }

        let config = self.inner.load_config().await?;
        if let Some(config) = &config {
            // Reuse the domain validator — the ONLY retention predicate.
            config.validate_retention(self.retention.as_retention_secs())?;
        }
        Ok(config)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence_policy::domain::{
        HistoryPredicate, RuleAction, SequenceRule, StepPredicate,
    };

    /// A repository returning a fixed config (or `Ok(None)`).
    struct FixedRepo(Option<SequencePolicyConfig>);

    #[async_trait]
    impl SequencePolicyRepository for FixedRepo {
        async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
            Ok(self.0.clone())
        }
    }

    fn effect_keyed_config(window_secs: u64, effect_key: bool) -> SequencePolicyConfig {
        SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![SequenceRule {
                id: "payout-guard".to_string(),
                name: "n".to_string(),
                description: "d".to_string(),
                steps: vec![StepPredicate {
                    tool: "run_command".to_string(),
                    params: vec![],
                }],
                window: None,
                action: RuleAction::Deny,
                history: Some(HistoryPredicate {
                    prior_node: "payout".to_string(),
                    same_principal: true,
                    window_secs,
                    effect_key,
                }),
            }],
        }
    }

    fn wrapped(
        config: Option<SequencePolicyConfig>,
        retention_secs: Option<u64>,
    ) -> RetentionCoupledSequencePolicyRepository {
        RetentionCoupledSequencePolicyRepository::new(Box::new(FixedRepo(config)), retention_secs)
    }

    #[tokio::test]
    async fn effect_keyed_window_over_retention_is_refused() {
        let repo = wrapped(Some(effect_keyed_config(900, true)), Some(600));
        let err = repo
            .load_config()
            .await
            .expect_err("window > retention must fail closed");
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
        assert!(!err.is_retriable());
    }

    #[tokio::test]
    async fn window_within_retention_loads() {
        let repo = wrapped(Some(effect_keyed_config(900, true)), Some(900));
        assert!(
            repo.load_config()
                .await
                .expect("window <= retention")
                .is_some()
        );
    }

    #[tokio::test]
    async fn unset_retention_is_status_quo() {
        let repo = wrapped(Some(effect_keyed_config(900, true)), None);
        assert_eq!(repo.retention_secs(), None);
        assert!(repo.load_config().await.expect("unlimited is Ok").is_some());
    }

    #[tokio::test]
    async fn non_effect_keyed_rule_is_unaffected() {
        let repo = wrapped(Some(effect_keyed_config(900, false)), Some(60));
        assert!(
            repo.load_config()
                .await
                .expect("non-effect-keyed has no retention coupling")
                .is_some()
        );
    }

    #[tokio::test]
    async fn absent_config_is_ok_none() {
        let repo = wrapped(None, Some(60));
        assert!(repo.load_config().await.expect("absent").is_none());
    }

    #[test]
    fn env_parsing_is_explicit() {
        assert_eq!(AuditRetention::parse(None), AuditRetention::Unlimited);
        assert_eq!(AuditRetention::parse(Some("")), AuditRetention::Unlimited);
        assert_eq!(AuditRetention::parse(Some("  ")), AuditRetention::Unlimited);
        assert_eq!(
            AuditRetention::parse(Some("600")),
            AuditRetention::Secs(600)
        );
        assert_eq!(
            AuditRetention::parse(Some("0")),
            AuditRetention::Secs(0),
            "zero is a valid (if aggressive) retention window"
        );
        assert!(matches!(
            AuditRetention::parse(Some("not-a-number")),
            AuditRetention::Invalid(_)
        ));
    }

    #[tokio::test]
    async fn invalid_env_value_fails_closed() {
        let repo = RetentionCoupledSequencePolicyRepository {
            inner: Box::new(FixedRepo(Some(effect_keyed_config(60, false)))),
            retention: AuditRetention::Invalid("bad".to_string()),
        };
        let err = repo
            .load_config()
            .await
            .expect_err("misconfigured retention must fail closed");
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
    }
}
