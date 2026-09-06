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
        }
    }
}

impl Default for SequencePolicyConfig {
    fn default() -> Self {
        Self {
            fail_closed: true,
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
            for step in &rule.steps {
                regex_predicates += step
                    .params
                    .iter()
                    .filter(|p| matches!(p.kind, super::rule::ParamMatchKind::Regex))
                    .count() as u32;
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
        Ok(())
    }

    /// Load-time convenience: validate against the concrete default caps.
    pub fn validate_with_default_caps(&self) -> Result<(), SequencePolicyError> {
        self.validate(&SafetyCaps::default())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::sequence_policy::domain::{
        ParamMatchKind, ParamPredicate, RuleAction, StepPredicate,
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
                value: ".*".to_string(),
            },
            ParamPredicate {
                pointer: "/b".to_string(),
                kind: ParamMatchKind::Regex,
                value: "^x".to_string(),
            },
        ];
        let config = SequencePolicyConfig {
            fail_closed: true,
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
            value: "x".to_string(),
        }];
        let config = SequencePolicyConfig {
            fail_closed: true,
            rules: vec![r],
        };
        assert!(config.validate(&caps()).is_ok());
        // Default caps accept the same config.
        assert!(config.validate_with_default_caps().is_ok());
    }
}
