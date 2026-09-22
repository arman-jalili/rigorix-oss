//! SequencePolicyConfig — the loaded rule set with safety caps.
//!
//! @canonical .pi/architecture/modules/sequence-policy.md#configuration
//! Implements: Contract Freeze — SequencePolicyConfig + SafetyCaps + validate()
//! Issue: #838 (sequence-policy epic — contract freeze); load/parse and cap
//!   enforcement in ISSUE-SEQUENCE-POLICY (fail-closed / fail-open-absent)
//!
//! Rules are declared in repository/org configuration
//! (`.rigorix/sequence-policy.toml`, same trust surface as `policy.toml` /
//! `permissions.toml`), authored by platform/security operators — **not** by
//! the executing agent (R5). The config is read per-run from disk.
//!
//! # Contract (Frozen)
//! - `fail_closed = true` (default): an unparseable / over-cap rule set blocks
//!   plan execution; a **missing** config file is `Ok(None)` (fail-open-absent
//!   — status quo, no gating) — never an error
//! - `rules` is the ordered rule set; empty rules → no matches
//! - `validate(&SafetyCaps)` enforces the safety caps (max rules per file, max
//!   predicates per rule, max window, max regex predicates — regex count is a
//!   denial-of-service surface); implementation in the fail-closed /
//!   fail-open-absent issues

use serde::{Deserialize, Serialize};

use super::error::SequencePolicyError;
use super::requirement::{StepRequirement, invalid_match_predicate};
use super::rule::SequenceRule;

/// Safety caps for the loaded rule set (mirrors `EnforcementConfig::validate`
/// posture). Regex predicates are capped because they are a ReDoS /
/// resource-exhaustion surface.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SafetyCaps {
    /// Maximum number of `[[rules]]` entries per config file.
    pub max_rules_per_file: u32,
    /// Maximum number of ordered step predicates per rule.
    pub max_steps_per_rule: u32,
    /// Maximum `window` gap between the first and last matched step.
    pub max_window: u32,
    /// Maximum number of regex parameter predicates across the file.
    pub max_regex_predicates_per_file: u32,
    /// Maximum R7 `history.window_secs` look-back (default 7 days) — keeps
    /// history scans bounded and stops a stale conflict from denying today.
    pub max_history_window_secs: u64,
    /// Maximum number of `[[requirements]]` entries per config file (R9).
    pub max_requirements_per_file: u32,
    /// Maximum required JSON pointers per requirement (R9) — bounds the
    /// pointer-resolution work an operator config can impose on a plan.
    pub max_required_params_per_requirement: u32,
}

/// The loaded sequence-policy rule set.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SequencePolicyConfig {
    /// Fail closed on config errors at plan time. Defaults to `true` — a
    /// corrupt rule file must refuse the plan rather than silently degrade.
    #[serde(default = "default_fail_closed")]
    pub fail_closed: bool,
    /// The ordered rule set (empty → no sequence gating).
    #[serde(default)]
    pub rules: Vec<SequenceRule>,
    /// R9 operator-controlled step requirements (empty → no requirements).
    /// Additive to `rules`; evaluated at plan time for every plan. Skipped
    /// when empty so pre-R9 configs serialize unchanged (additive/absent =
    /// status quo).
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub requirements: Vec<StepRequirement>,
}

const fn default_fail_closed() -> bool {
    true
}

impl Default for SafetyCaps {
    /// Concrete default caps (the values the frozen contract tests use as
    /// the example cap set): 100 rules / 8 step predicates per rule / window
    /// 5 / 8 regex predicates per file. Operators who need more can extend
    /// the caps surface; these defaults keep the rule engine bounded.
    fn default() -> Self {
        Self {
            max_rules_per_file: 100,
            max_steps_per_rule: 8,
            max_window: 5,
            max_regex_predicates_per_file: 8,
            max_history_window_secs: 604_800,
            max_requirements_per_file: 100,
            max_required_params_per_requirement: 16,
        }
    }
}

impl Default for SequencePolicyConfig {
    fn default() -> Self {
        Self {
            fail_closed: true,
            requirements: Vec::new(),
            rules: Vec::new(),
        }
    }
}

impl SequencePolicyConfig {
    /// Validate this config against the safety caps.
    ///
    /// Returns `Ok(())` when every rule is within the caps.
    ///
    /// # Errors
    /// - `SequencePolicyError::RuleExceedsCaps` — a rule exceeds one of the
    ///   caps (`rule` = rule id, `detail` = which cap and by how much)
    ///
    /// # Implementation
    /// TODO: enforced in the fail-closed / fail-open-absent issues (config
    /// load + parse land in `infrastructure/repository/toml_repository.rs`).
    pub fn validate(&self, caps: &SafetyCaps) -> Result<(), SequencePolicyError> {
        if self.rules.len() as u32 > caps.max_rules_per_file {
            return Err(SequencePolicyError::RuleExceedsCaps {
                rule: "<config>".to_string(),
                detail: format!(
                    "{} rules exceeds cap max_rules_per_file={}",
                    self.rules.len(),
                    caps.max_rules_per_file
                ),
            });
        }

        // Regex parameter predicates are a ReDoS / resource-exhaustion
        // surface — counted across the whole file (module spec §Security).
        let mut regex_predicates: u32 = 0;
        for rule in &self.rules {
            if rule.steps.len() as u32 > caps.max_steps_per_rule {
                return Err(SequencePolicyError::RuleExceedsCaps {
                    rule: rule.id.clone(),
                    detail: format!(
                        "{} step predicates exceeds cap max_steps_per_rule={}",
                        rule.steps.len(),
                        caps.max_steps_per_rule
                    ),
                });
            }
            if let Some(window) = rule.window
                && window > caps.max_window
            {
                return Err(SequencePolicyError::RuleExceedsCaps {
                    rule: rule.id.clone(),
                    detail: format!("window {window} exceeds cap max_window={}", caps.max_window),
                });
            }
            if let Some(hist) = &rule.history {
                if hist.prior_node.trim().is_empty() {
                    return Err(SequencePolicyError::InvalidConfig(format!(
                        "rule '{}': history.prior_node cannot be empty",
                        rule.id
                    )));
                }
                if hist.window_secs > caps.max_history_window_secs {
                    return Err(SequencePolicyError::RuleExceedsCaps {
                        rule: rule.id.clone(),
                        detail: format!(
                            "history.window_secs {} exceeds cap max_history_window_secs={}",
                            hist.window_secs, caps.max_history_window_secs
                        ),
                    });
                }
            }
            for (idx, step) in rule.steps.iter().enumerate() {
                for p in &step.params {
                    if matches!(p.kind, super::rule::ParamMatchKind::Regex) {
                        regex_predicates += 1;
                    }
                    // Literal kinds require a value — a missing one is a
                    // typo, not an empty-string match (fail closed).
                    if !matches!(p.kind, super::rule::ParamMatchKind::EqualsStep)
                        && p.value.is_none()
                    {
                        return Err(SequencePolicyError::InvalidConfig(format!(
                            "rule '{}': parameter predicate '{}' (kind {:?}) requires `value`",
                            rule.id, p.pointer, p.kind
                        )));
                    }
                    // R8 / ADR-014: a value-identity predicate is only valid
                    // when it references an EARLIER step of the same rule —
                    // fail closed otherwise (operator config error).
                    if matches!(p.kind, super::rule::ParamMatchKind::EqualsStep) {
                        match p.step {
                            Some(target) if target < idx => {}
                            Some(target) => {
                                return Err(SequencePolicyError::InvalidConfig(format!(
                                    "rule '{}': equals_step predicate '{}' references step {target}, \
                                     which is not an earlier step (must be < {idx})",
                                    rule.id, p.pointer
                                )));
                            }
                            None => {
                                return Err(SequencePolicyError::InvalidConfig(format!(
                                    "rule '{}': equals_step predicate '{}' requires `step` (earlier \
                                     matched step index)",
                                    rule.id, p.pointer
                                )));
                            }
                        }
                    }
                }
            }
        }
        if regex_predicates > caps.max_regex_predicates_per_file {
            return Err(SequencePolicyError::RuleExceedsCaps {
                rule: "<config>".to_string(),
                detail: format!(
                    "{regex_predicates} regex predicates exceeds cap max_regex_predicates_per_file={}",
                    caps.max_regex_predicates_per_file
                ),
            });
        }

        // ── R9 operator-controlled step requirements (ADR-015) ──────────
        // Fail-closed validation: a malformed requirement refuses the plan
        // (same posture as corrupt rules). An absent requirement set
        // (`requirements: []`) is the status quo.
        if self.requirements.len() as u32 > caps.max_requirements_per_file {
            return Err(SequencePolicyError::RuleExceedsCaps {
                rule: "<config>".to_string(),
                detail: format!(
                    "{} requirements exceeds cap max_requirements_per_file={}",
                    self.requirements.len(),
                    caps.max_requirements_per_file
                ),
            });
        }
        let mut seen_requirement_ids: Vec<&str> = Vec::new();
        for req in &self.requirements {
            if req.id.trim().is_empty() {
                return Err(SequencePolicyError::InvalidConfig(
                    "requirement with empty `id`".to_string(),
                ));
            }
            if seen_requirement_ids.contains(&req.id.as_str()) {
                return Err(SequencePolicyError::InvalidConfig(format!(
                    "duplicate requirement id '{}'",
                    req.id
                )));
            }
            seen_requirement_ids.push(&req.id);

            // A requirement with neither obligation is a no-op — a typo, not
            // an operator intent (fail closed).
            if !req.require_identity && req.require_params.is_empty() {
                return Err(SequencePolicyError::InvalidConfig(format!(
                    "requirement '{}': must set at least one of require_identity / require_params",
                    req.id
                )));
            }
            if req.require_params.len() as u32 > caps.max_required_params_per_requirement {
                return Err(SequencePolicyError::RuleExceedsCaps {
                    rule: req.id.clone(),
                    detail: format!(
                        "{} required params exceeds cap max_required_params_per_requirement={}",
                        req.require_params.len(),
                        caps.max_required_params_per_requirement
                    ),
                });
            }
            for pointer in &req.require_params {
                if !pointer.starts_with('/') {
                    return Err(SequencePolicyError::InvalidConfig(format!(
                        "requirement '{}': required parameter pointer '{}' must start with '/'",
                        req.id, pointer
                    )));
                }
            }

            // The reused StepPredicate matcher is validated like a rule step;
            // `equals_step` has no earlier-step context on a single-step
            // requirement and is rejected (fail closed).
            if let Some(detail) = invalid_match_predicate(&req.r#match) {
                return Err(SequencePolicyError::InvalidConfig(format!(
                    "requirement '{}': {detail}",
                    req.id
                )));
            }
            for p in &req.r#match.params {
                if p.value.is_none() {
                    return Err(SequencePolicyError::InvalidConfig(format!(
                        "requirement '{}': match parameter predicate '{}' (kind {:?}) requires `value`",
                        req.id, p.pointer, p.kind
                    )));
                }
            }
        }
        Ok(())
    }

    /// Load-time convenience: validate against the concrete default caps.
    pub fn validate_with_default_caps(&self) -> Result<(), SequencePolicyError> {
        self.validate(&SafetyCaps::default())
    }

    /// R8 / ADR-014 (AC #17): retention must cover every effect-keyed rule's
    /// look-back window, or the rule **silently stops firing** once old
    /// envelopes are pruned from the signed trail. The composition root calls
    /// this when an audit retention policy is configured; `None` means
    /// unlimited retention and is always valid.
    ///
    /// # Errors
    /// - `SequencePolicyError::InvalidConfig` — an `effect_key`-gated rule's
    ///   `window_secs` exceeds the configured retention.
    pub fn validate_retention(
        &self,
        retention_secs: Option<u64>,
    ) -> Result<(), SequencePolicyError> {
        let Some(retention) = retention_secs else {
            return Ok(());
        };
        for rule in &self.rules {
            if let Some(hist) = &rule.history
                && hist.effect_key
                && hist.window_secs > retention
            {
                return Err(SequencePolicyError::InvalidConfig(format!(
                    "rule '{}': effect-keyed history window {}s exceeds audit retention {}s — the \
                     rule would silently stop firing once old envelopes are pruned",
                    rule.id, hist.window_secs, retention
                )));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence_policy::domain::{
        HistoryPredicate, ParamMatchKind, ParamPredicate, RequirementAction, RuleAction,
        StepPredicate, StepRequirement,
    };

    fn rule(id: &str, steps: usize) -> SequenceRule {
        SequenceRule {
            id: id.to_string(),
            name: "n".to_string(),
            description: "d".to_string(),
            steps: (0..steps)
                .map(|i| StepPredicate {
                    tool: format!("tool_{i}"),
                    params: vec![],
                })
                .collect(),
            window: None,
            action: RuleAction::Promote,
            history: None,
        }
    }

    fn caps() -> SafetyCaps {
        SafetyCaps {
            max_rules_per_file: 2,
            max_steps_per_rule: 3,
            max_window: 5,
            max_regex_predicates_per_file: 1,
            max_history_window_secs: 60,
            max_requirements_per_file: 2,
            max_required_params_per_requirement: 2,
        }
    }

    #[test]
    fn empty_config_validates() {
        let config = SequencePolicyConfig::default();
        assert!(config.validate_with_default_caps().is_ok());
    }

    #[test]
    fn over_cap_rule_count_is_rejected() {
        let config = SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![rule("r1", 2), rule("r2", 2), rule("r3", 2)],
        };
        let err = config.validate(&caps()).unwrap_err();
        assert!(matches!(&err,
            SequencePolicyError::RuleExceedsCaps { rule, .. } if rule == "<config>"
        ));
        assert!(!err.is_retriable());
    }

    #[test]
    fn over_cap_steps_per_rule_is_rejected() {
        let config = SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![rule("r1", 4)],
        };
        let err = config.validate(&caps()).unwrap_err();
        assert!(matches!(&err,
            SequencePolicyError::RuleExceedsCaps { rule, .. } if rule == "r1"
        ));
        assert!(err.to_string().contains("exceeds safety caps"));
    }

    #[test]
    fn over_cap_window_is_rejected() {
        let mut r = rule("r1", 2);
        r.window = Some(6);
        let config = SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![r],
        };
        let err = config.validate(&caps()).unwrap_err();
        assert!(matches!(&err,
            SequencePolicyError::RuleExceedsCaps { rule, .. } if rule == "r1"
        ));
    }

    #[test]
    fn over_cap_regex_predicates_are_rejected() {
        // Two regex predicates > max_regex_predicates_per_file = 1.
        let mut r = rule("r1", 1);
        r.steps[0].params = vec![
            ParamPredicate {
                pointer: "/a".to_string(),
                kind: ParamMatchKind::Regex,
                value: Some(".*".to_string()),
                step: None,
            },
            ParamPredicate {
                pointer: "/b".to_string(),
                kind: ParamMatchKind::Regex,
                value: Some("^x".to_string()),
                step: None,
            },
        ];
        let config = SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![r],
        };
        let err = config.validate(&caps()).unwrap_err();
        assert!(matches!(&err,
            SequencePolicyError::RuleExceedsCaps { rule, .. } if rule == "<config>"
        ));
    }

    #[test]
    fn within_caps_validates_ok() {
        let mut r = rule("r1", 2);
        r.window = Some(3);
        r.steps[0].params = vec![ParamPredicate {
            pointer: "/a".to_string(),
            kind: ParamMatchKind::Exact,
            value: Some("x".to_string()),
            step: None,
        }];
        let config = SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![r],
        };
        assert!(config.validate(&caps()).is_ok());
        // Default caps accept the same config.
        assert!(config.validate_with_default_caps().is_ok());
    }

    #[test]
    fn equals_step_validation_accepts_earlier_reference_and_rejects_bad_ones() {
        // R8: a value-identity predicate must reference an EARLIER step.
        let mut ok = rule("r1", 2);
        ok.steps[1].params = vec![ParamPredicate {
            pointer: "/beneficiary".to_string(),
            kind: ParamMatchKind::EqualsStep,
            value: None,
            step: Some(0),
        }];
        let config = SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![ok],
        };
        assert!(
            config.validate(&caps()).is_ok(),
            "earlier reference is valid"
        );

        // Missing `step` → fail closed.
        let mut missing = rule("r1", 2);
        missing.steps[1].params = vec![ParamPredicate {
            pointer: "/beneficiary".to_string(),
            kind: ParamMatchKind::EqualsStep,
            value: None,
            step: None,
        }];
        let err = SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![missing],
        }
        .validate(&caps())
        .unwrap_err();
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));

        // Forward / self reference → fail closed.
        let mut forward = rule("r1", 2);
        forward.steps[1].params = vec![ParamPredicate {
            pointer: "/beneficiary".to_string(),
            kind: ParamMatchKind::EqualsStep,
            value: None,
            step: Some(1),
        }];
        let err = SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![forward],
        }
        .validate(&caps())
        .unwrap_err();
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
    }

    #[test]
    fn literal_predicate_without_value_fails_closed() {
        let mut r = rule("r1", 2);
        r.steps[0].params = vec![ParamPredicate {
            pointer: "/event_id".to_string(),
            kind: ParamMatchKind::Exact,
            value: None,
            step: None,
        }];
        let err = SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![r],
        }
        .validate(&caps())
        .unwrap_err();
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));

        // Explicit empty value is allowed (operator intent, not a typo).
        let mut ok = rule("r1", 2);
        ok.steps[0].params = vec![ParamPredicate {
            pointer: "/event_id".to_string(),
            kind: ParamMatchKind::Exact,
            value: Some(String::new()),
            step: None,
        }];
        assert!(
            SequencePolicyConfig {
                fail_closed: true,
                requirements: Vec::new(),
                rules: vec![ok],
            }
            .validate(&caps())
            .is_ok()
        );
    }

    #[test]
    fn retention_must_cover_effect_keyed_windows() {
        // AC #17: an effect-keyed rule whose window outlives the audit
        // retention would silently stop firing — refuse it.
        let mut r = rule("r1", 2);
        r.history = Some(HistoryPredicate {
            prior_node: "*".to_string(),
            same_principal: true,
            window_secs: 900,
            effect_key: true,
        });
        let config = SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![r],
        };
        // Unlimited retention → fine.
        assert!(config.validate_retention(None).is_ok());
        // Retention ≥ window → fine.
        assert!(config.validate_retention(Some(900)).is_ok());
        assert!(config.validate_retention(Some(86_400)).is_ok());
        // Retention < window → fail closed.
        let err = config.validate_retention(Some(600)).unwrap_err();
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
    }

    #[test]
    fn retention_ignores_non_effect_keyed_rules() {
        // A node+principal history rule (no effect key) has no retention
        // coupling — it never depends on the effect-key record.
        let mut r = rule("r1", 2);
        r.history = Some(HistoryPredicate {
            prior_node: "payout".to_string(),
            same_principal: true,
            window_secs: 900,
            effect_key: false,
        });
        let config = SequencePolicyConfig {
            fail_closed: true,
            requirements: Vec::new(),
            rules: vec![r],
        };
        assert!(config.validate_retention(Some(60)).is_ok());
    }

    // ── R9 operator-controlled step requirements (ADR-015, AC 22) ─────────

    fn requirement(id: &str) -> StepRequirement {
        StepRequirement {
            id: id.to_string(),
            name: "n".to_string(),
            description: "d".to_string(),
            r#match: StepPredicate {
                tool: "run_command".to_string(),
                params: vec![],
            },
            require_identity: true,
            require_params: vec![],
            action: RequirementAction::Deny,
        }
    }

    fn config_with_requirements(requirements: Vec<StepRequirement>) -> SequencePolicyConfig {
        SequencePolicyConfig {
            fail_closed: true,
            requirements,
            rules: Vec::new(),
        }
    }

    #[test]
    fn requirement_without_obligation_fails_closed() {
        let mut r = requirement("empty");
        r.require_identity = false;
        let err = config_with_requirements(vec![r])
            .validate(&caps())
            .unwrap_err();
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
        assert!(!err.is_retriable());
    }

    #[test]
    fn duplicate_requirement_id_fails_closed() {
        let err = config_with_requirements(vec![requirement("dupe"), requirement("dupe")])
            .validate(&caps())
            .unwrap_err();
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
    }

    #[test]
    fn requirement_pointer_must_start_with_slash() {
        let mut r = requirement("ptr");
        r.require_identity = false;
        r.require_params = vec!["beneficiary".to_string()];
        let err = config_with_requirements(vec![r])
            .validate(&caps())
            .unwrap_err();
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
    }

    #[test]
    fn requirement_too_many_pointers_exceeds_caps() {
        let mut r = requirement("many");
        r.require_identity = false;
        r.require_params = vec![
            "/a".to_string(),
            "/b".to_string(),
            "/c".to_string(), // cap max_required_params_per_requirement = 2
        ];
        let err = config_with_requirements(vec![r])
            .validate(&caps())
            .unwrap_err();
        assert!(matches!(&err,
            SequencePolicyError::RuleExceedsCaps { rule, .. } if rule == "many"
        ));
    }

    #[test]
    fn requirement_count_over_cap_exceeds_caps() {
        let err = config_with_requirements(vec![
            requirement("r1"),
            requirement("r2"),
            requirement("r3"), // cap max_requirements_per_file = 2
        ])
        .validate(&caps())
        .unwrap_err();
        assert!(matches!(&err,
            SequencePolicyError::RuleExceedsCaps { rule, .. } if rule == "<config>"
        ));
    }

    #[test]
    fn requirement_equals_step_match_fails_closed() {
        let mut r = requirement("eq");
        r.r#match.params = vec![ParamPredicate {
            pointer: "/beneficiary".to_string(),
            kind: ParamMatchKind::EqualsStep,
            value: None,
            step: Some(0),
        }];
        let err = config_with_requirements(vec![r])
            .validate(&caps())
            .unwrap_err();
        assert!(matches!(err, SequencePolicyError::InvalidConfig(_)));
    }

    #[test]
    fn valid_requirement_passes_validation_and_is_defaulted_absent() {
        let mut r = requirement("payout-guard");
        r.require_identity = true;
        r.require_params = vec!["/beneficiary".to_string()];
        assert!(config_with_requirements(vec![r]).validate(&caps()).is_ok());
        // Absent requirements = status quo.
        assert!(SequencePolicyConfig::default().validate(&caps()).is_ok());
        assert!(SequencePolicyConfig::default().requirements.is_empty());
    }
}
