//! Enterprise configuration domain entity.
//!
//! Defines the configuration for connecting to Rigorix Enterprise,
//! including API credentials, policy fetching, and audit posting settings.

use serde::{Deserialize, Serialize};

/// Enterprise integration configuration.
///
/// When present, the engine will fetch policy bundles from the enterprise
/// API, merge them with local enforcement config, and post audit records.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnterpriseConfig {
    /// Enterprise API base URL (e.g. "https://rigorix.example.com/api/v1").
    pub api_url: String,

    /// Enterprise API key (secret).
    pub api_key: String,

    /// Team UUID in Rigorix Enterprise.
    pub team_id: uuid::Uuid,

    /// Whether to fetch policies from enterprise. Default: true.
    #[serde(default = "default_true")]
    pub fetch_policies: bool,

    /// Whether to enforce enterprise policies. Default: true.
    #[serde(default = "default_true")]
    pub enforce_policies: bool,

    /// Whether to post audit records to enterprise. Default: true.
    #[serde(default = "default_true")]
    pub post_audit: bool,

    /// Policy cache TTL in seconds. Default: 300 (5 minutes).
    #[serde(default = "default_cache_ttl")]
    pub policy_cache_ttl_secs: u64,
}

impl Default for EnterpriseConfig {
    fn default() -> Self {
        Self {
            api_url: String::new(),
            api_key: String::new(),
            team_id: uuid::Uuid::nil(),
            fetch_policies: true,
            enforce_policies: true,
            post_audit: true,
            policy_cache_ttl_secs: 300,
        }
    }
}

fn default_true() -> bool {
    true
}

fn default_cache_ttl() -> u64 {
    300
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        let config = EnterpriseConfig::default();
        assert!(config.api_url.is_empty());
        assert!(config.api_key.is_empty());
        assert_eq!(config.team_id, uuid::Uuid::nil());
        assert!(config.fetch_policies);
        assert!(config.enforce_policies);
        assert!(config.post_audit);
        assert_eq!(config.policy_cache_ttl_secs, 300);
    }

    #[test]
    fn test_serde_roundtrip() {
        let config = EnterpriseConfig {
            api_url: "https://rigorix.example.com/api/v1".to_string(),
            api_key: "sk-test-key".to_string(),
            team_id: uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            fetch_policies: true,
            enforce_policies: false,
            post_audit: true,
            policy_cache_ttl_secs: 600,
        };

        let json = serde_json::to_string(&config).unwrap();
        let deserialized: EnterpriseConfig = serde_json::from_str(&json).unwrap();
        assert_eq!(config, deserialized);
    }

    #[test]
    fn test_serde_defaults_for_optional_fields() {
        let json = r#"{
            "api_url": "https://rigorix.example.com/api/v1",
            "api_key": "sk-test",
            "team_id": "00000000-0000-0000-0000-000000000001"
        }"#;
        let config: EnterpriseConfig = serde_json::from_str(json).unwrap();
        assert!(config.fetch_policies);
        assert!(config.enforce_policies);
        assert!(config.post_audit);
        assert_eq!(config.policy_cache_ttl_secs, 300);
    }
}
