//! PrecedenceSequencePolicyRepository — resolve a local operator TOML and an
//! enterprise-exported bundle into the one active `SequencePolicyConfig`.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#configuration
//! Implements: #889 (OSS-C5) — precedence + trust between
//!   `.rigorix/sequence-policy.toml` and a fetched enterprise policy bundle.
//!
//! Enterprise is the **source of truth**, but an operator may still keep a
//! local TOML (offline development, migration, or an intentional override).
//! This repository makes the interaction explicit and fail-closed:
//!
//! | Both present? | Precedence | Result |
//! |---|---|---|
//! | no | any | `Ok(None)` (status quo) |
//! | local only | any | local config |
//! | bundle only | any | bundle config |
//! | both | [`SequencePolicyPrecedence::EnterpriseWins`] (default) | bundle config, shadowed local logged |
//! | both | [`SequencePolicyPrecedence::LocalWins`] | local config (explicit override), bundle logged |
//! | both | [`SequencePolicyPrecedence::RefuseOnConflict`] | equal → bundle; differing → `Err` (fail closed) |
//!
//! Sources are never merged: the two rule sets are whole-config alternatives.
//! A malformed source fails closed regardless of precedence (the individual
//! repository surfaces an `Err`).
//!
//! Override the default with `RIGORIX_SEQUENCE_POLICY_PRECEDENCE`:
//! `enterprise` (default) | `local` | `refuse`.

use async_trait::async_trait;

use crate::sequence_policy::domain::{SequencePolicyConfig, SequencePolicyError};

use super::SequencePolicyRepository;

/// Environment variable selecting the local-vs-bundle precedence.
pub const PRECEDENCE_ENV: &str = "RIGORIX_SEQUENCE_POLICY_PRECEDENCE";

/// How a locally-present TOML and a fetched bundle are reconciled.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SequencePolicyPrecedence {
    /// Enterprise (bundle) is the source of truth; a differing local TOML is
    /// shadowed with a warning. Default.
    #[default]
    EnterpriseWins,
    /// Explicit override: the local TOML wins; the bundle is shadowed with a
    /// warning.
    LocalWins,
    /// Refuse to start when both are present and differ — never guess. Equal
    /// configs are not a conflict.
    RefuseOnConflict,
}

impl SequencePolicyPrecedence {
    /// Parse a precedence token (`enterprise` | `local` | `refuse`).
    /// Unknown tokens return `None` (caller keeps the safe default).
    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "" | "enterprise" | "enterprise-wins" | "bundle" => Some(Self::EnterpriseWins),
            "local" | "local-wins" | "toml" => Some(Self::LocalWins),
            "refuse" | "refuse-on-conflict" | "strict" => Some(Self::RefuseOnConflict),
            _ => None,
        }
    }

    /// Read [`PRECEDENCE_ENV`], falling back to the safe default.
    pub fn from_env() -> Self {
        std::env::var(PRECEDENCE_ENV)
            .ok()
            .and_then(|v| Self::parse(&v))
            .unwrap_or_default()
    }
}

/// Composes a local TOML repository and a bundle repository under an explicit
/// precedence policy. Either side may be `None` when not configured.
pub struct PrecedenceSequencePolicyRepository {
    local: Option<Box<dyn SequencePolicyRepository>>,
    bundle: Option<Box<dyn SequencePolicyRepository>>,
    precedence: SequencePolicyPrecedence,
}

impl PrecedenceSequencePolicyRepository {
    /// Create the composing repository.
    pub fn new(
        local: Option<Box<dyn SequencePolicyRepository>>,
        bundle: Option<Box<dyn SequencePolicyRepository>>,
        precedence: SequencePolicyPrecedence,
    ) -> Self {
        Self {
            local,
            bundle,
            precedence,
        }
    }
}

#[async_trait]
impl SequencePolicyRepository for PrecedenceSequencePolicyRepository {
    async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
        // Load both configured sources (each fails closed on its own errors).
        let local = match &self.local {
            Some(repo) => repo.load_config().await?,
            None => None,
        };
        let bundle = match &self.bundle {
            Some(repo) => repo.load_config().await?,
            None => None,
        };

        match (local, bundle) {
            (None, None) => Ok(None),
            (Some(local), None) => Ok(Some(local)),
            (None, Some(bundle)) => {
                tracing::info!(
                    "sequence_policy: enterprise policy bundle is the active source (no local TOML)"
                );
                Ok(Some(bundle))
            }
            (Some(local), Some(bundle)) => match self.precedence {
                SequencePolicyPrecedence::EnterpriseWins => {
                    if local != bundle {
                        tracing::warn!(
                            "sequence_policy: local .rigorix/sequence-policy.toml differs from the \
                             fetched enterprise bundle — enterprise wins (set \
                             {PRECEDENCE_ENV}=local to override)"
                        );
                    }
                    Ok(Some(bundle))
                }
                SequencePolicyPrecedence::LocalWins => {
                    if local != bundle {
                        tracing::warn!(
                            "sequence_policy: local .rigorix/sequence-policy.toml overrides the \
                             fetched enterprise bundle ({PRECEDENCE_ENV}=local)"
                        );
                    }
                    Ok(Some(local))
                }
                SequencePolicyPrecedence::RefuseOnConflict => {
                    if local == bundle {
                        Ok(Some(bundle))
                    } else {
                        Err(SequencePolicyError::InvalidConfig(format!(
                            "local .rigorix/sequence-policy.toml conflicts with the fetched \
                             enterprise policy bundle — refusing to start (fail closed); set \
                             {PRECEDENCE_ENV}=enterprise to let the bundle win"
                        )))
                    }
                }
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;

    /// A repository returning a fixed config (or `Ok(None)`).
    struct FixedRepo(Option<SequencePolicyConfig>);

    #[async_trait]
    impl SequencePolicyRepository for FixedRepo {
        async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
            Ok(self.0.clone())
        }
    }

    fn config(id: &str) -> SequencePolicyConfig {
        SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![crate::sequence_policy::domain::SequenceRule {
                id: id.to_string(),
                name: "n".to_string(),
                description: "d".to_string(),
                steps: vec![crate::sequence_policy::domain::StepPredicate {
                    tool: "t".to_string(),
                    params: vec![],
                }],
                window: None,
                action: crate::sequence_policy::domain::RuleAction::Deny,
                history: None,
            }],
        }
    }

    fn local(cfg: Option<SequencePolicyConfig>) -> Option<Box<dyn SequencePolicyRepository>> {
        Some(Box::new(FixedRepo(cfg)))
    }
    fn bundle(cfg: Option<SequencePolicyConfig>) -> Option<Box<dyn SequencePolicyRepository>> {
        Some(Box::new(FixedRepo(cfg)))
    }

    #[tokio::test]
    async fn neither_source_is_ok_none() {
        let repo = PrecedenceSequencePolicyRepository::new(
            None,
            None,
            SequencePolicyPrecedence::EnterpriseWins,
        );
        assert!(repo.load_config().await.expect("none").is_none());
    }

    #[tokio::test]
    async fn local_only_returns_local() {
        let repo = PrecedenceSequencePolicyRepository::new(
            local(Some(config("local-rule"))),
            None,
            SequencePolicyPrecedence::EnterpriseWins,
        );
        let got = repo.load_config().await.expect("ok").expect("some");
        assert_eq!(got.rules[0].id, "local-rule");
    }

    #[tokio::test]
    async fn bundle_only_returns_bundle() {
        let repo = PrecedenceSequencePolicyRepository::new(
            None,
            bundle(Some(config("bundle-rule"))),
            SequencePolicyPrecedence::EnterpriseWins,
        );
        let got = repo.load_config().await.expect("ok").expect("some");
        assert_eq!(got.rules[0].id, "bundle-rule");
    }

    #[tokio::test]
    async fn enterprise_wins_is_the_default() {
        let repo = PrecedenceSequencePolicyRepository::new(
            local(Some(config("local-rule"))),
            bundle(Some(config("bundle-rule"))),
            SequencePolicyPrecedence::default(),
        );
        let got = repo.load_config().await.expect("ok").expect("some");
        assert_eq!(got.rules[0].id, "bundle-rule");
        assert_eq!(
            SequencePolicyPrecedence::default(),
            SequencePolicyPrecedence::EnterpriseWins
        );
    }

    #[tokio::test]
    async fn local_wins_override_returns_local() {
        let repo = PrecedenceSequencePolicyRepository::new(
            local(Some(config("local-rule"))),
            bundle(Some(config("bundle-rule"))),
            SequencePolicyPrecedence::LocalWins,
        );
        let got = repo.load_config().await.expect("ok").expect("some");
        assert_eq!(got.rules[0].id, "local-rule");
    }

    #[tokio::test]
    async fn refuse_on_conflict_fails_closed_when_sources_differ() {
        let repo = PrecedenceSequencePolicyRepository::new(
            local(Some(config("local-rule"))),
            bundle(Some(config("bundle-rule"))),
            SequencePolicyPrecedence::RefuseOnConflict,
        );
        let err = repo.load_config().await.expect_err("must fail closed");
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
        assert!(!err.is_retriable());
    }

    #[tokio::test]
    async fn refuse_on_conflict_accepts_equal_sources() {
        let repo = PrecedenceSequencePolicyRepository::new(
            local(Some(config("same-rule"))),
            bundle(Some(config("same-rule"))),
            SequencePolicyPrecedence::RefuseOnConflict,
        );
        let got = repo.load_config().await.expect("equal is not a conflict");
        assert_eq!(got.expect("some").rules[0].id, "same-rule");
    }

    #[test]
    fn precedence_parses_tokens_and_defaults_safe() {
        assert_eq!(
            SequencePolicyPrecedence::parse("enterprise"),
            Some(SequencePolicyPrecedence::EnterpriseWins)
        );
        assert_eq!(
            SequencePolicyPrecedence::parse("LOCAL"),
            Some(SequencePolicyPrecedence::LocalWins)
        );
        assert_eq!(
            SequencePolicyPrecedence::parse("refuse"),
            Some(SequencePolicyPrecedence::RefuseOnConflict)
        );
        assert_eq!(SequencePolicyPrecedence::parse("nonsense"), None);
        assert_eq!(
            SequencePolicyPrecedence::parse(""),
            Some(SequencePolicyPrecedence::EnterpriseWins)
        );
    }
}
