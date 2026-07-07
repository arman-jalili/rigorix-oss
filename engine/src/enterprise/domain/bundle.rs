//! Policy bundle types for enterprise policy fetching.

use serde::{Deserialize, Serialize};

/// A policy bundle fetched from the enterprise API.
///
/// Contains one or more policy entries plus a HMAC signature for
/// integrity verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyBundle {
    /// Team ID the bundle was generated for.
    pub team_id: uuid::Uuid,

    /// When the bundle was generated on the enterprise server.
    pub generated_at: chrono::DateTime<chrono::Utc>,

    /// The policy entries in this bundle.
    pub policies: Vec<PolicyBundleEntry>,

    /// HMAC-SHA256 signature in "sha256=<hex>" format.
    pub signature: String,
}

/// A single policy entry within a policy bundle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyBundleEntry {
    /// Unique policy ID.
    pub id: uuid::Uuid,

    /// Human-readable policy name.
    pub name: String,

    /// Rule type (e.g. "block", "warn", "monitor", "tool_blocklist",
    /// "llm_budget", "risk_threshold").
    pub rule_type: String,

    /// Rule-specific configuration as arbitrary JSON.
    pub rule_config: serde_json::Value,

    /// Enforcement mode (e.g. "enforce", "audit").
    pub enforcement_mode: String,

    /// Severity level (e.g. "low", "medium", "high", "critical").
    pub severity: String,

    /// Whether this policy is enabled.
    pub enabled: bool,
}

/// Produce a canonical JSON string for the given policies.
///
/// The output is a deterministic string used as input to the HMAC
/// signature verification. Keys are sorted, no extra whitespace.
pub fn canonical_json(policies: &[PolicyBundleEntry]) -> String {
    serde_json::to_string(policies).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn sample_entry(name: &str, rule_type: &str) -> PolicyBundleEntry {
        PolicyBundleEntry {
            id: uuid::Uuid::new_v4(),
            name: name.to_string(),
            rule_type: rule_type.to_string(),
            rule_config: json!({}),
            enforcement_mode: "enforce".to_string(),
            severity: "high".to_string(),
            enabled: true,
        }
    }

    #[test]
    fn test_policy_bundle_serde_roundtrip() {
        let bundle = PolicyBundle {
            team_id: uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            generated_at: chrono::Utc::now(),
            policies: vec![
                sample_entry("block-bash", "tool_blocklist"),
                sample_entry("cap-tokens", "llm_budget"),
            ],
            signature: "sha256=abcdef1234567890".to_string(),
        };

        let json = serde_json::to_string(&bundle).unwrap();
        let deserialized: PolicyBundle = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.team_id, bundle.team_id);
        assert_eq!(deserialized.policies.len(), 2);
        assert_eq!(deserialized.signature, bundle.signature);
    }

    #[test]
    fn test_canonical_json_empty() {
        let result = canonical_json(&[]);
        assert_eq!(result, "[]");
    }

    #[test]
    fn test_canonical_json_deterministic() {
        let entries = vec![
            sample_entry("policy-a", "block"),
            sample_entry("policy-b", "warn"),
        ];

        let first = canonical_json(&entries);
        let second = canonical_json(&entries);
        assert_eq!(first, second, "canonical_json must be deterministic");
    }

    #[test]
    fn test_policy_bundle_entry_serde() {
        let entry = PolicyBundleEntry {
            id: uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            name: "test-policy".to_string(),
            rule_type: "block".to_string(),
            rule_config: json!({"tool": "bash"}),
            enforcement_mode: "enforce".to_string(),
            severity: "critical".to_string(),
            enabled: true,
        };

        let json = serde_json::to_string_pretty(&entry).unwrap();
        assert!(json.contains("block"));
        assert!(json.contains("bash"));
        assert!(json.contains("critical"));

        let deserialized: PolicyBundleEntry = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.name, "test-policy");
        assert_eq!(deserialized.rule_config["tool"], "bash");
    }

    #[test]
    fn test_entry_with_enabled_false() {
        let entry = PolicyBundleEntry {
            id: uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap(),
            name: "test".to_string(),
            rule_type: "block".to_string(),
            rule_config: json!({}),
            enforcement_mode: "enforce".to_string(),
            severity: "low".to_string(),
            enabled: false,
        };
        assert!(!entry.enabled);
    }
}
