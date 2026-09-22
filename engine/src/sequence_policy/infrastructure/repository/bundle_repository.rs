//! BundleSequencePolicyRepository — enterprise-exported policy bundle JSON →
//! `SequencePolicyConfig`.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#configuration
//! Implements: #889 (OSS-C5) — engine consumes an enterprise-exported policy
//!   bundle (`rigorix.policy.bundle` / `GET /api/v1/policies/bundle`) and
//!   enforces it identically to the equivalent local TOML.
//!
//! Enterprise is the policy **source of truth**. It exports a bundle whose
//! root surface is `policy.json` v1: `{ "fail_closed": bool, "rules": [...],
//! "requirements": [...] }` — the exact serde shape of
//! [`SequencePolicyConfig`]. This repository makes that bundle a first-class
//! load source alongside [`super::TomlSequencePolicyRepository`].
//!
//! # Contract
//! - **Reuse the validator.** The bundle is parsed straight into
//!   [`SequencePolicyConfig`] and then validated with the SAME
//!   `validate_with_default_caps()` the TOML path uses — never a parallel
//!   validator that could diverge.
//! - **Absent vs invalid is explicit.**
//!   - no bundle configured (`Absent`) → `Ok(None)` (fail-open-absent, status quo)
//!   - an empty configured source / missing bundle file → `Ok(None)` (mirrors
//!     the local file's fail-open-absent semantics)
//!   - a bundle that is present but not a `policy.json` v1 object, is
//!     unparseable, or exceeds the safety caps → `Err` (**fail closed**)
//! - SafetyCaps apply to bundle-sourced rules and requirements exactly as they
//!   do to local ones (shared `validate`).
//!
//! # Sources
//! - `from_value(value)` — an already-parsed JSON value (e.g. fetched by the
//!   server composition from the enterprise backend, or a signed bundle seam)
//! - `from_file(path)` — a JSON file on disk
//! - `from_env()` — `RIGORIX_SEQUENCE_POLICY_BUNDLE` (inline JSON, wins) or
//!   `RIGORIX_SEQUENCE_POLICY_BUNDLE_PATH` (file path)

use std::path::PathBuf;

use async_trait::async_trait;
use serde_json::Value;

use crate::sequence_policy::domain::{SequencePolicyConfig, SequencePolicyError};

use super::SequencePolicyRepository;

/// Environment variable carrying an inline bundle JSON document.
pub const BUNDLE_INLINE_ENV: &str = "RIGORIX_SEQUENCE_POLICY_BUNDLE";
/// Environment variable carrying the path to a fetched bundle JSON file.
pub const BUNDLE_PATH_ENV: &str = "RIGORIX_SEQUENCE_POLICY_BUNDLE_PATH";

/// Where a [`BundleSequencePolicyRepository`] reads its bundle from.
#[derive(Debug)]
enum BundleSource {
    /// No bundle configured — status quo (fail-open-absent).
    Absent,
    /// An in-process JSON value (already fetched/parsed by the composition).
    Value(Value),
    /// A JSON file read per-run.
    File(PathBuf),
    /// Configured but unusable at construction (e.g. malformed inline JSON):
    /// surfaced per-run as a fail-closed `InvalidConfig`.
    Invalid(String),
}

/// Reads an enterprise-exported `policy.json` v1 bundle into a
/// [`SequencePolicyConfig`].
#[derive(Debug)]
pub struct BundleSequencePolicyRepository {
    source: BundleSource,
}

impl BundleSequencePolicyRepository {
    /// No bundle configured (fail-open-absent).
    pub fn absent() -> Self {
        Self {
            source: BundleSource::Absent,
        }
    }

    /// Use an already-parsed bundle JSON value.
    pub fn from_value(value: Value) -> Self {
        Self {
            source: BundleSource::Value(value),
        }
    }

    /// Use a bundle JSON file read per-run.
    pub fn from_file(path: impl Into<PathBuf>) -> Self {
        Self {
            source: BundleSource::File(path.into()),
        }
    }

    /// Build from the environment. Inline JSON
    /// ([`BUNDLE_INLINE_ENV`]) wins over a path ([`BUNDLE_PATH_ENV`]). Neither
    /// set (or empty) → [`Self::absent`].
    pub fn from_env() -> Self {
        match std::env::var(BUNDLE_INLINE_ENV) {
            Ok(inline) if !inline.trim().is_empty() => {
                match serde_json::from_str::<Value>(inline.trim()) {
                    Ok(value) => Self::from_value(value),
                    Err(e) => Self {
                        source: BundleSource::Invalid(format!(
                            "{BUNDLE_INLINE_ENV} is not valid JSON: {e}"
                        )),
                    },
                }
            }
            _ => match std::env::var(BUNDLE_PATH_ENV) {
                Ok(path) if !path.trim().is_empty() => Self::from_file(path.trim().to_string()),
                _ => Self::absent(),
            },
        }
    }

    /// Whether a bundle source is configured (including an invalid one — an
    /// invalid configured bundle must fail closed, not be treated as absent).
    pub fn is_configured(&self) -> bool {
        !matches!(self.source, BundleSource::Absent)
    }

    /// Parse + validate a `policy.json` v1 bundle value into a
    /// [`SequencePolicyConfig`], reusing the frozen validation + SafetyCaps.
    ///
    /// # Errors
    /// - `SequencePolicyError::InvalidConfig` — the value is not an object,
    ///   lacks the required `rules` array, is structurally unparseable, or
    ///   fails semantic validation
    /// - `SequencePolicyError::RuleExceedsCaps` — a rule/requirement exceeds
    ///   the safety caps
    pub fn parse_bundle(value: &Value) -> Result<SequencePolicyConfig, SequencePolicyError> {
        let object = value.as_object().ok_or_else(|| {
            SequencePolicyError::InvalidConfig(
                "policy bundle must be a JSON object (policy.json v1 root)".to_string(),
            )
        })?;
        // policy.json v1 requires `fail_closed` + `rules`. `fail_closed`
        // serde-defaults to `true` (the safe value), so only the rule array
        // is a hard structural requirement here.
        if !object.contains_key("rules") {
            return Err(SequencePolicyError::InvalidConfig(
                "policy bundle is missing the required `rules` array (policy.json v1 root)"
                    .to_string(),
            ));
        }

        let config: SequencePolicyConfig = serde_json::from_value(value.clone()).map_err(|e| {
            SequencePolicyError::InvalidConfig(format!("policy bundle parse error: {e}"))
        })?;

        // Shared validation — never a parallel validator.
        config.validate_with_default_caps()?;
        Ok(config)
    }
}

#[async_trait]
impl SequencePolicyRepository for BundleSequencePolicyRepository {
    async fn load_config(&self) -> Result<Option<SequencePolicyConfig>, SequencePolicyError> {
        match &self.source {
            BundleSource::Absent => Ok(None),
            BundleSource::Invalid(reason) => {
                Err(SequencePolicyError::InvalidConfig(reason.clone()))
            }
            BundleSource::Value(value) => Ok(Some(Self::parse_bundle(value)?)),
            BundleSource::File(path) => {
                let text = match tokio::fs::read_to_string(path).await {
                    Ok(text) => text,
                    // A configured-but-missing bundle file is the
                    // fail-open-absent case (mirrors the local TOML file).
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                        tracing::debug!(
                            path = %path.display(),
                            "sequence_policy: enterprise bundle file absent — fail-open-absent"
                        );
                        return Ok(None);
                    }
                    Err(e) => {
                        return Err(SequencePolicyError::InvalidConfig(format!(
                            "failed to read policy bundle {}: {e}",
                            path.display()
                        )));
                    }
                };
                if text.trim().is_empty() {
                    return Ok(None);
                }
                let value: Value = serde_json::from_str(&text).map_err(|e| {
                    SequencePolicyError::InvalidConfig(format!(
                        "policy bundle {} is not valid JSON: {e}",
                        path.display()
                    ))
                })?;
                Ok(Some(Self::parse_bundle(&value)?))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn minimal_bundle() -> Value {
        json!({
            "fail_closed": true,
            "rules": [
                {
                    "id": "remove-then-add",
                    "name": "No remove-then-add",
                    "description": "d",
                    "steps": [
                        { "tool": "registration_remove" },
                        { "tool": "registration_add" }
                    ],
                    "window": 3,
                    "action": "deny"
                }
            ]
        })
    }

    #[tokio::test]
    async fn absent_bundle_is_ok_none() {
        let repo = BundleSequencePolicyRepository::absent();
        assert!(!repo.is_configured());
        assert!(
            repo.load_config()
                .await
                .expect("absent is Ok(None)")
                .is_none()
        );
    }

    #[tokio::test]
    async fn valid_bundle_parses_and_validates() {
        let repo = BundleSequencePolicyRepository::from_value(minimal_bundle());
        let config = repo
            .load_config()
            .await
            .expect("valid bundle")
            .expect("Some");
        assert!(config.fail_closed);
        assert_eq!(config.rules.len(), 1);
        assert!(config.requirements.is_empty());
    }

    #[tokio::test]
    async fn requirements_and_equals_step_round_trip() {
        let value = json!({
            "fail_closed": true,
            "rules": [{
                "id": "effect-keyed",
                "name": "n",
                "description": "d",
                "steps": [
                    { "tool": "run_command", "params": [
                        { "pointer": "/command", "kind": "glob", "value": "*remove*" }
                    ]},
                    { "tool": "run_command", "params": [
                        { "pointer": "/command", "kind": "glob", "value": "*add*" },
                        { "pointer": "/beneficiary", "kind": "equals_step", "step": 0 }
                    ]}
                ],
                "history": {
                    "prior_node": "remove",
                    "same_principal": true,
                    "window_secs": 900,
                    "effect_key": true
                }
            }],
            "requirements": [{
                "id": "payout-guard",
                "name": "Payout guard",
                "description": "d",
                "match": { "tool": "run_command", "params": [
                    { "pointer": "/command", "kind": "glob", "value": "*payout*" }
                ]},
                "require_identity": true,
                "require_params": ["/beneficiary"],
                "action": "deny"
            }]
        });
        let config = BundleSequencePolicyRepository::parse_bundle(&value).expect("round-trip");
        assert_eq!(config.requirements.len(), 1);
        assert!(config.requirements[0].require_identity);
        let rule = &config.rules[0];
        assert!(rule.history.as_ref().expect("history").effect_key);
        assert_eq!(
            rule.steps[1].params[1].kind,
            crate::sequence_policy::domain::ParamMatchKind::EqualsStep
        );
    }

    #[test]
    fn non_object_bundle_fails_closed() {
        let err = BundleSequencePolicyRepository::parse_bundle(&json!(["not", "an", "object"]))
            .expect_err("must fail closed");
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
        assert!(!err.is_retriable());
    }

    #[test]
    fn bundle_missing_rules_fails_closed() {
        let err = BundleSequencePolicyRepository::parse_bundle(&json!({ "fail_closed": true }))
            .expect_err("missing rules must fail closed");
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
    }

    #[test]
    fn structurally_invalid_rule_fails_closed() {
        // A rule missing `id` is a serde error → fail closed.
        let value = json!({
            "rules": [{ "name": "n", "description": "d", "steps": [{ "tool": "x" }] }]
        });
        let err = BundleSequencePolicyRepository::parse_bundle(&value)
            .expect_err("missing id must fail closed");
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
    }

    #[test]
    fn over_cap_bundle_fails_closed_with_safety_caps() {
        // 9 steps > default max_steps_per_rule = 8.
        let steps: Vec<Value> = (0..9).map(|i| json!({ "tool": format!("t{i}") })).collect();
        let value = json!({
            "rules": [{
                "id": "over-cap",
                "name": "n",
                "description": "d",
                "steps": steps
            }]
        });
        let err = BundleSequencePolicyRepository::parse_bundle(&value)
            .expect_err("over-cap must fail closed");
        assert!(matches!(
            &err,
            SequencePolicyError::RuleExceedsCaps { rule, .. } if rule == "over-cap"
        ));
    }

    #[tokio::test]
    async fn missing_bundle_file_is_ok_none() {
        let repo = BundleSequencePolicyRepository::from_file(
            std::env::temp_dir().join("rigorix-no-such-bundle-xyz.json"),
        );
        assert!(repo.is_configured());
        assert!(repo.load_config().await.expect("missing file").is_none());
    }

    #[tokio::test]
    async fn malformed_bundle_file_fails_closed() {
        let path =
            std::env::temp_dir().join(format!("rigorix-bad-bundle-{}.json", uuid::Uuid::new_v4()));
        std::fs::write(&path, b"{ not json").expect("write");
        let repo = BundleSequencePolicyRepository::from_file(&path);
        let err = repo.load_config().await.expect_err("must fail closed");
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
        let _ = std::fs::remove_file(&path);
    }
}
