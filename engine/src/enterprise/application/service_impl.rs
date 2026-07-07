//! Implementation of EnterpriseService.
//!
//! Wires together HttpEnterpriseClient, signature verification, and
//! policy merging with in-memory caching.

use async_trait::async_trait;
use std::sync::Mutex;
use std::time::Instant;

use crate::enforcement::domain::EnforcementConfig;
use crate::enterprise::domain::{EnterpriseConfig, EnterpriseError, PolicyBundle};

use super::dto::{FetchBundleOutput, MergePoliciesOutput};
use super::service::EnterpriseService;
use crate::enterprise::infrastructure::HttpEnterpriseClient;

/// In-memory cache entry for a fetched policy bundle.
struct CacheEntry {
    bundle: PolicyBundle,
    fetched_at: Instant,
}

/// Implementation of EnterpriseService.
pub struct EnterpriseServiceImpl {
    client: HttpEnterpriseClient,
    cache: Mutex<Option<CacheEntry>>,
}

impl EnterpriseServiceImpl {
    pub fn new(client: HttpEnterpriseClient) -> Self {
        Self {
            client,
            cache: Mutex::new(None),
        }
    }

    /// Verify the HMAC-SHA256 signature on a policy bundle.
    fn verify_bundle_signature(
        bundle: &PolicyBundle,
        api_key: &str,
    ) -> Result<(), EnterpriseError> {
        use hmac::{Hmac, Mac};
        use hmac::KeyInit;
        use sha2::Sha256;

        let canonical = crate::enterprise::domain::bundle::canonical_json(&bundle.policies);
        let payload = format!("{}{}{}", bundle.team_id, bundle.generated_at.to_rfc3339(), canonical);

        let mut mac = Hmac::<Sha256>::new_from_slice(api_key.as_bytes())
            .map_err(|_| EnterpriseError::SignatureMismatch {
                detail: "Invalid HMAC key".to_string(),
            })?;
        mac.update(payload.as_bytes());
        let expected = hex::encode(mac.finalize().into_bytes());

        // Strip "sha256=" prefix from the bundle signature if present
        let received = bundle.signature.strip_prefix("sha256=").unwrap_or(&bundle.signature);

        // Constant-time comparison (manual)
        let received_bytes = hex::decode(received).map_err(|_| EnterpriseError::SignatureMismatch {
            detail: "Invalid hex in bundle signature".to_string(),
        })?;
        let expected_bytes = hex::decode(&expected).map_err(|_| EnterpriseError::SignatureMismatch {
            detail: "Invalid hex in computed signature".to_string(),
        })?;

        if constant_time_eq(&received_bytes, &expected_bytes) {
            Ok(())
        } else {
            Err(EnterpriseError::SignatureMismatch {
                detail: "Bundle signature does not match computed HMAC".to_string(),
            })
        }
    }

    /// Merge enterprise policies into local enforcement config.
    fn merge_policies_impl(
        bundle: &PolicyBundle,
        local_config: &EnforcementConfig,
    ) -> EnforcementConfig {
        let mut merged = local_config.clone();

        for policy in &bundle.policies {
            if !policy.enabled {
                continue;
            }

            match policy.rule_type.as_str() {
                "tool_blocklist" => {
                    // Add tools to tool_policies with allowed: false
                    if let Some(tools) = policy.rule_config.get("tools").and_then(|v| v.as_array()) {
                        for tool_val in tools {
                            if let Some(tool_name) = tool_val.as_str() {
                                let existing = merged.tool_policies.get(tool_name).cloned().unwrap_or_default();
                                merged = merged.with_tool_policy(
                                    tool_name,
                                    crate::enforcement::domain::ToolPolicy {
                                        allowed: false,
                                        ..existing
                                    },
                                );
                            }
                        }
                    }
                }
                "llm_budget" => {
                    // Caps local budget at enterprise limit
                    if let Some(limit) = policy.rule_config.get("max_tokens").and_then(|v| v.as_u64())
                        && let Some(budget) = merged.budgets.get_mut("tokens")
                        && limit < budget.hard_limit
                    {
                        budget.hard_limit = limit;
                    }
                }
                "risk_threshold" => {
                    // Can only raise minimum risk level, never lower
                    if let Some(level) = policy.rule_config.get("min_risk_level").and_then(|v| v.as_str()) {
                        let enterprise_risk = match level {
                            "high" | "critical" => 3u8,
                            "medium" => 2u8,
                            _ => 1u8,
                        };
                        // Apply by restricting specific tool policies
                        for (_, tool_policy) in merged.tool_policies.iter_mut() {
                            let current_risk = match tool_policy.risk_level {
                                crate::enforcement::domain::ToolRiskLevel::Low => 1u8,
                                crate::enforcement::domain::ToolRiskLevel::Medium => 2u8,
                                crate::enforcement::domain::ToolRiskLevel::High => 3u8,
                                crate::enforcement::domain::ToolRiskLevel::Critical => 4u8,
                            };
                            if enterprise_risk > current_risk {
                                // Raise risk to enterprise minimum
                                tool_policy.risk_level = match enterprise_risk {
                                    2 => crate::enforcement::domain::ToolRiskLevel::Medium,
                                    3 => crate::enforcement::domain::ToolRiskLevel::High,
                                    _ => crate::enforcement::domain::ToolRiskLevel::Critical,
                                };
                            }
                        }
                    }
                }
                "block" => {
                    // Enterprise block overrides local settings
                    if let Some(tool_name) = policy.rule_config.get("tool").and_then(|v| v.as_str()) {
                        let existing = merged.tool_policies.get(tool_name).cloned().unwrap_or_default();
                        merged = merged.with_tool_policy(
                            tool_name,
                            crate::enforcement::domain::ToolPolicy {
                                allowed: false,
                                ..existing
                            },
                        );
                    }
                }
                "warn" => {
                    // Enterprise warn overrides local
                    if let Some(tool_name) = policy.rule_config.get("tool").and_then(|v| v.as_str()) {
                        let existing = merged.tool_policies.get(tool_name).cloned().unwrap_or_default();
                        merged = merged.with_tool_policy(
                            tool_name,
                            crate::enforcement::domain::ToolPolicy {
                                allowed: true,
                                requires_confirmation: true,
                                ..existing
                            },
                        );
                    }
                }
                "monitor" => {
                    // Use local setting, just report violations
                    // No config changes needed at this level
                }
                _ => {
                    // Unknown rule type — skip
                    tracing::debug!(
                        "Unknown enterprise policy rule_type: {}",
                        policy.rule_type
                    );
                }
            }
        }

        merged
    }
}

#[async_trait]
impl EnterpriseService for EnterpriseServiceImpl {
    async fn fetch_policy_bundle(
        &self,
        config: &EnterpriseConfig,
    ) -> Result<FetchBundleOutput, EnterpriseError> {
        // Check cache first
        {
            let cache = self.cache.lock().map_err(|_| EnterpriseError::CacheError {
                detail: "Cache lock poisoned".to_string(),
            })?;
            if let Some(entry) = cache.as_ref() {
                let elapsed = entry.fetched_at.elapsed().as_secs();
                if elapsed < config.policy_cache_ttl_secs {
                    return Ok(FetchBundleOutput {
                        bundle: entry.bundle.clone(),
                    });
                }
            }
        }

        // Fetch from API
        let bundle = self.client.fetch_bundle(config).await?;

        // Verify signature
        Self::verify_bundle_signature(&bundle, &config.api_key)?;

        // Update cache
        {
            let mut cache = self.cache.lock().map_err(|_| EnterpriseError::CacheError {
                detail: "Cache lock poisoned".to_string(),
            })?;
            *cache = Some(CacheEntry {
                bundle: bundle.clone(),
                fetched_at: Instant::now(),
            });
        }

        Ok(FetchBundleOutput { bundle })
    }

    async fn merge_policies(
        &self,
        bundle: &PolicyBundle,
        local_config: &EnforcementConfig,
    ) -> Result<MergePoliciesOutput, EnterpriseError> {
        let merged = Self::merge_policies_impl(bundle, local_config);
        Ok(MergePoliciesOutput { merged_config: merged })
    }

    async fn get_enforcement_config(
        &self,
        enterprise_config: &EnterpriseConfig,
        local_config: &EnforcementConfig,
    ) -> Result<EnforcementConfig, EnterpriseError> {
        let output = self.fetch_policy_bundle(enterprise_config).await?;
        let merge_output = self.merge_policies(&output.bundle, local_config).await?;
        Ok(merge_output.merged_config)
    }
}

/// Constant-time byte comparison.
///
/// Returns true if both slices are equal in length and content.
/// Execution time depends only on the length, not the content.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) {
        result |= x ^ y;
    }
    result == 0
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enforcement::domain::{EnforcementConfig, ToolPolicy, ToolRiskLevel};
    use crate::enterprise::domain::{PolicyBundle, PolicyBundleEntry};
    use crate::enterprise::domain::bundle::canonical_json;
    use hmac::{Hmac, Mac};
    use hmac::KeyInit;
    use serde_json::json;
    use sha2::Sha256;

    fn policy_bundle_with_signature(
        policies: Vec<PolicyBundleEntry>,
        api_key: &str,
    ) -> PolicyBundle {
        let team_id = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let generated_at = chrono::DateTime::parse_from_rfc3339("2026-07-07T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let canonical = canonical_json(&policies);
        let payload = format!("{}{}{}", team_id, generated_at.to_rfc3339(), canonical);

        let mut mac = Hmac::<Sha256>::new_from_slice(api_key.as_bytes()).unwrap();
        mac.update(payload.as_bytes());
        let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        PolicyBundle {
            team_id,
            generated_at,
            policies,
            signature,
        }
    }

    fn local_config() -> EnforcementConfig {
        EnforcementConfig::standard()
    }

    fn make_entry(name: &str, rule_type: &str, rule_config: serde_json::Value) -> PolicyBundleEntry {
        PolicyBundleEntry {
            id: uuid::Uuid::new_v4(),
            name: name.to_string(),
            rule_type: rule_type.to_string(),
            rule_config,
            enforcement_mode: "enforce".to_string(),
            severity: "high".to_string(),
            enabled: true,
        }
    }

    // -----------------------------------------------------------------------
    // constant_time_eq tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_constant_time_eq_equal() {
        assert!(constant_time_eq(b"hello", b"hello"));
    }

    #[test]
    fn test_constant_time_eq_different_length() {
        assert!(!constant_time_eq(b"hello", b"hello!"));
    }

    #[test]
    fn test_constant_time_eq_different_content() {
        assert!(!constant_time_eq(b"hello", b"world"));
    }

    #[test]
    fn test_constant_time_eq_empty() {
        assert!(constant_time_eq(b"", b""));
    }

    #[test]
    fn test_constant_time_eq_single_byte() {
        assert!(constant_time_eq(b"\x00", b"\x00"));
        assert!(!constant_time_eq(b"\x00", b"\x01"));
    }

    // -----------------------------------------------------------------------
    // verify_bundle_signature tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_verify_valid_signature() {
        let api_key = "test-api-key-123";
        let policies = vec![make_entry("block-bash", "tool_blocklist", json!({"tools": ["bash"]}))];
        let bundle = policy_bundle_with_signature(policies, api_key);

        let result = EnterpriseServiceImpl::verify_bundle_signature(&bundle, api_key);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn test_verify_invalid_signature() {
        let api_key = "test-api-key-123";
        let policies = vec![make_entry("block-bash", "tool_blocklist", json!({"tools": ["bash"]}))];
        let mut bundle = policy_bundle_with_signature(policies, api_key);
        // Tamper with the signature
        bundle.signature = "sha256=deadbeef".to_string();

        let result = EnterpriseServiceImpl::verify_bundle_signature(&bundle, api_key);
        match result {
            Err(EnterpriseError::SignatureMismatch { .. }) => {} // expected
            other => panic!("Expected SignatureMismatch, got: {:?}", other),
        }
    }

    #[test]
    fn test_verify_wrong_api_key() {
        let api_key = "test-api-key-123";
        let wrong_key = "wrong-key";
        let policies = vec![make_entry("block-bash", "tool_blocklist", json!({"tools": ["bash"]}))];
        let bundle = policy_bundle_with_signature(policies, api_key);

        let result = EnterpriseServiceImpl::verify_bundle_signature(&bundle, wrong_key);
        match result {
            Err(EnterpriseError::SignatureMismatch { .. }) => {} // expected
            other => panic!("Expected SignatureMismatch, got: {:?}", other),
        }
    }

    #[test]
    fn test_verify_signature_no_prefix() {
        // Should also work without "sha256=" prefix
        let api_key = "test-key";
        let policies = vec![make_entry("test", "block", json!({"tool": "bash"}))];
        let team_id = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let generated_at = chrono::DateTime::parse_from_rfc3339("2026-07-07T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let canonical = canonical_json(&policies);
        let payload = format!("{}{}{}", team_id, generated_at.to_rfc3339(), canonical);

        let mut mac = Hmac::<Sha256>::new_from_slice(api_key.as_bytes()).unwrap();
        mac.update(payload.as_bytes());
        let raw_sig = hex::encode(mac.finalize().into_bytes());

        let bundle = PolicyBundle {
            team_id,
            generated_at,
            policies,
            signature: raw_sig, // no "sha256=" prefix
        };

        let result = EnterpriseServiceImpl::verify_bundle_signature(&bundle, api_key);
        assert!(result.is_ok(), "Expected Ok, got: {:?}", result.err());
    }

    #[test]
    fn test_verify_signature_invalid_hex() {
        let api_key = "test-key";
        let policies = vec![make_entry("test", "block", json!({"tool": "bash"}))];
        let team_id = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let generated_at = chrono::Utc::now();

        let bundle = PolicyBundle {
            team_id,
            generated_at,
            policies,
            signature: "sha256=not-hex!!".to_string(),
        };

        let result = EnterpriseServiceImpl::verify_bundle_signature(&bundle, api_key);
        match result {
            Err(EnterpriseError::SignatureMismatch { .. }) => {} // expected
            other => panic!("Expected SignatureMismatch for bad hex, got: {:?}", other),
        }
    }

    // -----------------------------------------------------------------------
    // Merge policy tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_merge_tool_blocklist() {
        let local = local_config();
        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![make_entry("block-dangerous", "tool_blocklist", json!({
                "tools": ["bash", "write"]
            }))],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);

        // bash should be blocked
        let bash_policy = merged.tool_policies.get("bash").unwrap();
        assert!(!bash_policy.allowed, "Enterprise blocklist should set bash to not allowed");

        // write should be blocked
        let write_policy = merged.tool_policies.get("write").unwrap();
        assert!(!write_policy.allowed, "Enterprise blocklist should set write to not allowed");

        // read should be unchanged
        let read_policy = merged.tool_policies.get("read").unwrap();
        assert!(read_policy.allowed, "Read should remain allowed");
    }

    #[test]
    fn test_merge_tool_blocklist_preserves_other_fields() {
        let mut local = local_config();
        local = local.with_tool_policy("bash", ToolPolicy {
            allowed: false,
            risk_level: ToolRiskLevel::Critical,
            requires_confirmation: true,
            dry_run: true,
            max_calls: Some(5),
            budget_key: Some("custom".to_string()),
        });

        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![make_entry("block-bash", "tool_blocklist", json!({
                "tools": ["bash"]
            }))],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);
        let bash_policy = merged.tool_policies.get("bash").unwrap();

        // allowed is overridden
        assert!(!bash_policy.allowed);
        // other fields preserved
        assert_eq!(bash_policy.risk_level, ToolRiskLevel::Critical);
        assert!(bash_policy.requires_confirmation);
        assert!(bash_policy.dry_run);
        assert_eq!(bash_policy.max_calls, Some(5));
    }

    #[test]
    fn test_merge_block_rule() {
        let local = local_config();
        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![make_entry("block-write", "block", json!({
                "tool": "write"
            }))],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);
        let write_policy = merged.tool_policies.get("write").unwrap();
        assert!(!write_policy.allowed, "Enterprise block should block write");
    }

    #[test]
    fn test_merge_warn_rule() {
        let local = local_config();
        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![make_entry("warn-bash", "warn", json!({
                "tool": "bash"
            }))],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);
        let bash_policy = merged.tool_policies.get("bash").unwrap();
        assert!(bash_policy.allowed, "Warn should keep tool allowed");
        assert!(bash_policy.requires_confirmation, "Warn should require confirmation");
    }

    #[test]
    fn test_merge_monitor_rule() {
        let local = local_config();
        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![make_entry("monitor-all", "monitor", json!({}))],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);
        // Monitor should not change anything
        assert_eq!(merged, local);
    }

    #[test]
    fn test_merge_llm_budget_caps() {
        let local = local_config();
        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![make_entry("cap-tokens", "llm_budget", json!({
                "max_tokens": 50000  // local is 100000, so this should cap it
            }))],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);
        let token_budget = merged.budgets.get("tokens").unwrap();
        assert_eq!(token_budget.hard_limit, 50000, "Enterprise llm_budget should cap at 50000");
    }

    #[test]
    fn test_merge_llm_budget_does_not_raise() {
        let local = local_config();
        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![make_entry("raise-tokens", "llm_budget", json!({
                "max_tokens": 200000  // local is 100000, so this should NOT raise it
            }))],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);
        let token_budget = merged.budgets.get("tokens").unwrap();
        assert_eq!(
            token_budget.hard_limit, 100000,
            "Enterprise llm_budget should not raise the limit"
        );
    }

    #[test]
    fn test_merge_risk_threshold() {
        let mut local = local_config();
        // Set bash to Low risk locally
        local = local.with_tool_policy("bash", ToolPolicy {
            allowed: true,
            risk_level: ToolRiskLevel::Low,
            requires_confirmation: false,
            dry_run: false,
            max_calls: None,
            budget_key: None,
        });

        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![make_entry("raise-risk", "risk_threshold", json!({
                "min_risk_level": "high"
            }))],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);

        // bash risk should be raised from Low to High
        let bash_policy = merged.tool_policies.get("bash").unwrap();
        assert_eq!(
            bash_policy.risk_level,
            ToolRiskLevel::High,
            "Enterprise risk_threshold should raise minimum risk level"
        );
    }

    #[test]
    fn test_merge_risk_threshold_medium() {
        let mut local = local_config();
        local = local.with_tool_policy("bash", ToolPolicy {
            allowed: true,
            risk_level: ToolRiskLevel::Low,
            requires_confirmation: false,
            dry_run: false,
            max_calls: None,
            budget_key: None,
        });

        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![make_entry("medium-risk", "risk_threshold", json!({
                "min_risk_level": "medium"
            }))],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);
        let bash_policy = merged.tool_policies.get("bash").unwrap();
        assert_eq!(bash_policy.risk_level, ToolRiskLevel::Medium);
    }

    #[test]
    fn test_merge_risk_threshold_does_not_lower() {
        let mut local = local_config();
        local = local.with_tool_policy("bash", ToolPolicy {
            allowed: true,
            risk_level: ToolRiskLevel::Critical,
            requires_confirmation: true,
            dry_run: false,
            max_calls: None,
            budget_key: None,
        });

        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![make_entry("lower-risk", "risk_threshold", json!({
                "min_risk_level": "low"
            }))],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);
        let bash_policy = merged.tool_policies.get("bash").unwrap();
        assert_eq!(
            bash_policy.risk_level,
            ToolRiskLevel::Critical,
            "risk_threshold should not lower existing risk levels"
        );
    }

    #[test]
    fn test_disabled_policy_not_merged() {
        let local = local_config();
        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![PolicyBundleEntry {
                id: uuid::Uuid::new_v4(),
                name: "disabled-policy".to_string(),
                rule_type: "tool_blocklist".to_string(),
                rule_config: json!({"tools": ["bash"]}),
                enforcement_mode: "enforce".to_string(),
                severity: "critical".to_string(),
                enabled: false,
            }],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);

        // bash should remain unchanged since policy is disabled
        let bash_policy = merged.tool_policies.get("bash").unwrap();
        assert!(bash_policy.allowed, "Disabled enterprise policy should not affect local config");
    }

    #[test]
    fn test_merge_unknown_rule_type() {
        let local = local_config();
        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![make_entry("weird-rule", "unknown_type", json!({
                "some_setting": true
            }))],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);
        // Unknown rule types should be silently ignored
        assert_eq!(merged, local);
    }

    #[test]
    fn test_merge_empty_bundle() {
        let local = local_config();
        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);
        assert_eq!(merged, local, "Empty bundle should result in identical config");
    }

    #[test]
    fn test_merge_multiple_policies() {
        let local = local_config();
        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![
                make_entry("block-bash", "tool_blocklist", json!({"tools": ["bash"]})),
                make_entry("warn-write", "warn", json!({"tool": "write"})),
                make_entry("cap-tokens", "llm_budget", json!({"max_tokens": 50000})),
            ],
            signature: String::new(),
        };

        let merged = EnterpriseServiceImpl::merge_policies_impl(&bundle, &local);

        assert!(!merged.tool_policies.get("bash").unwrap().allowed, "blocklist applied");
        assert!(merged.tool_policies.get("write").unwrap().requires_confirmation, "warn applied");
        assert_eq!(
            merged.budgets.get("tokens").unwrap().hard_limit,
            50000,
            "llm_budget applied"
        );
    }

    // -----------------------------------------------------------------------
    // EnterpriseServiceImpl integration tests
    // -----------------------------------------------------------------------

    #[tokio::test]
    async fn test_merge_policies_via_service() {
        let client = HttpEnterpriseClient::new();
        let service = EnterpriseServiceImpl::new(client);
        let bundle = PolicyBundle {
            team_id: uuid::Uuid::nil(),
            generated_at: chrono::Utc::now(),
            policies: vec![make_entry("block-bash", "tool_blocklist", json!({"tools": ["bash"]}))],
            signature: String::new(),
        };
        let local = local_config();

        let output = service.merge_policies(&bundle, &local).await.unwrap();
        assert!(!output.merged_config.tool_policies.get("bash").unwrap().allowed);
    }

    #[tokio::test]
    async fn test_fetch_policy_bundle_with_mock() {
        use wiremock::matchers::{header, method, path, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let api_key = "test-api-key";

        // Build a valid policy bundle with correct signature
        let team_id = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();
        let generated_at = chrono::DateTime::parse_from_rfc3339("2026-07-07T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let policy_id = uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();

        let policies = vec![PolicyBundleEntry {
            id: policy_id,
            name: "block-bash".to_string(),
            rule_type: "tool_blocklist".to_string(),
            rule_config: json!({"tools": ["bash"]}),
            enforcement_mode: "enforce".to_string(),
            severity: "critical".to_string(),
            enabled: true,
        }];
        let canonical = canonical_json(&policies);
        let payload = format!("{}{}{}", team_id, generated_at.to_rfc3339(), canonical);

        let mut mac = Hmac::<Sha256>::new_from_slice(api_key.as_bytes()).unwrap();
        mac.update(payload.as_bytes());
        let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        let bundle_json = serde_json::json!({
            "team_id": "00000000-0000-0000-0000-000000000001",
            "generated_at": "2026-07-07T12:00:00Z",
            "policies": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "name": "block-bash",
                    "rule_type": "tool_blocklist",
                    "rule_config": {"tools": ["bash"]},
                    "enforcement_mode": "enforce",
                    "severity": "critical",
                    "enabled": true
                }
            ],
            "signature": signature,
        });

        Mock::given(method("GET"))
            .and(path("/api/v1/policies/bundle"))
            .and(query_param("team_id", "00000000-0000-0000-0000-000000000001"))
            .and(header("Authorization", format!("Bearer {api_key}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(&bundle_json))
            .mount(&mock_server)
            .await;

        let client = HttpEnterpriseClient::new();
        let service = EnterpriseServiceImpl::new(client);
        let config = EnterpriseConfig {
            api_url: format!("{}/api/v1", mock_server.uri()),
            api_key: api_key.to_string(),
            team_id,
            fetch_policies: true,
            enforce_policies: true,
            post_audit: true,
            policy_cache_ttl_secs: 300,
        };

        let output = service.fetch_policy_bundle(&config).await.unwrap();
        assert_eq!(output.bundle.policies.len(), 1);
        assert_eq!(output.bundle.policies[0].name, "block-bash");
    }

    #[tokio::test]
    async fn test_get_enforcement_config_full_flow() {
        use wiremock::matchers::{header, method, path, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let api_key = "test-api-key";
        let team_id = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

        // Create a bundle with valid signature
        let generated_at = chrono::DateTime::parse_from_rfc3339("2026-07-07T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let policy_id = uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();

        let policies = vec![PolicyBundleEntry {
            id: policy_id,
            name: "block-bash".to_string(),
            rule_type: "tool_blocklist".to_string(),
            rule_config: json!({"tools": ["bash"]}),
            enforcement_mode: "enforce".to_string(),
            severity: "critical".to_string(),
            enabled: true,
        }];
        let canonical = canonical_json(&policies);
        let payload = format!("{}{}{}", team_id, generated_at.to_rfc3339(), canonical);

        let mut mac = Hmac::<Sha256>::new_from_slice(api_key.as_bytes()).unwrap();
        mac.update(payload.as_bytes());
        let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        let bundle_json = serde_json::json!({
            "team_id": "00000000-0000-0000-0000-000000000001",
            "generated_at": "2026-07-07T12:00:00Z",
            "policies": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "name": "block-bash",
                    "rule_type": "tool_blocklist",
                    "rule_config": {"tools": ["bash"]},
                    "enforcement_mode": "enforce",
                    "severity": "critical",
                    "enabled": true
                }
            ],
            "signature": signature,
        });

        Mock::given(method("GET"))
            .and(path("/api/v1/policies/bundle"))
            .and(query_param("team_id", "00000000-0000-0000-0000-000000000001"))
            .and(header("Authorization", format!("Bearer {api_key}")))
            .respond_with(ResponseTemplate::new(200).set_body_json(&bundle_json))
            .mount(&mock_server)
            .await;

        let client = HttpEnterpriseClient::new();
        let service = EnterpriseServiceImpl::new(client);
        let ent_config = EnterpriseConfig {
            api_url: format!("{}/api/v1", mock_server.uri()),
            api_key: api_key.to_string(),
            team_id,
            fetch_policies: true,
            enforce_policies: true,
            post_audit: true,
            policy_cache_ttl_secs: 300,
        };
        let local = local_config();

        let merged = service.get_enforcement_config(&ent_config, &local).await.unwrap();

        // bash should be blocked by enterprise policy
        assert!(!merged.tool_policies.get("bash").unwrap().allowed);
        // read should remain allowed (not in blocklist)
        assert!(merged.tool_policies.get("read").unwrap().allowed);
    }

    #[tokio::test]
    async fn test_fetch_policy_bundle_cache_hit() {
        use wiremock::matchers::{method, path, query_param};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;
        let api_key = "test-api-key";
        let team_id = uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap();

        let generated_at = chrono::DateTime::parse_from_rfc3339("2026-07-07T12:00:00Z")
            .unwrap()
            .with_timezone(&chrono::Utc);
        let policy_id = uuid::Uuid::parse_str("11111111-1111-1111-1111-111111111111").unwrap();

        let policies = vec![PolicyBundleEntry {
            id: policy_id,
            name: "block-bash".to_string(),
            rule_type: "tool_blocklist".to_string(),
            rule_config: json!({"tools": ["bash"]}),
            enforcement_mode: "enforce".to_string(),
            severity: "critical".to_string(),
            enabled: true,
        }];
        let canonical = canonical_json(&policies);
        let payload = format!("{}{}{}", team_id, generated_at.to_rfc3339(), canonical);

        let mut mac = Hmac::<Sha256>::new_from_slice(api_key.as_bytes()).unwrap();
        mac.update(payload.as_bytes());
        let signature = format!("sha256={}", hex::encode(mac.finalize().into_bytes()));

        let bundle_json = serde_json::json!({
            "team_id": "00000000-0000-0000-0000-000000000001",
            "generated_at": "2026-07-07T12:00:00Z",
            "policies": [
                {
                    "id": "11111111-1111-1111-1111-111111111111",
                    "name": "block-bash",
                    "rule_type": "tool_blocklist",
                    "rule_config": {"tools": ["bash"]},
                    "enforcement_mode": "enforce",
                    "severity": "critical",
                    "enabled": true
                }
            ],
            "signature": signature,
        });

        Mock::given(method("GET"))
            .and(path("/api/v1/policies/bundle"))
            .and(query_param("team_id", "00000000-0000-0000-0000-000000000001"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&bundle_json))
            .expect(1) // Only one HTTP call expected
            .mount(&mock_server)
            .await;

        let client = HttpEnterpriseClient::new();
        let service = EnterpriseServiceImpl::new(client);
        let ent_config = EnterpriseConfig {
            api_url: format!("{}/api/v1", mock_server.uri()),
            api_key: api_key.to_string(),
            team_id,
            fetch_policies: true,
            enforce_policies: true,
            post_audit: true,
            policy_cache_ttl_secs: 300, // TTL long enough for test
        };

        // First call — should hit API
        let first = service.fetch_policy_bundle(&ent_config).await.unwrap();
        assert_eq!(first.bundle.policies.len(), 1);

        // Second call — should hit cache, not API
        let second = service.fetch_policy_bundle(&ent_config).await.unwrap();
        assert_eq!(second.bundle.policies.len(), 1);
    }

    #[tokio::test]
    async fn test_fetch_policy_bundle_signature_mismatch() {
        use wiremock::matchers::{method, path};
        use wiremock::{Mock, MockServer, ResponseTemplate};

        let mock_server = MockServer::start().await;

        // Return bundle with invalid signature
        let bundle_json = serde_json::json!({
            "team_id": "00000000-0000-0000-0000-000000000001",
            "generated_at": "2026-07-07T12:00:00Z",
            "policies": [
                {
                    "id": uuid::Uuid::new_v4(),
                    "name": "block-bash",
                    "rule_type": "tool_blocklist",
                    "rule_config": {"tools": ["bash"]},
                    "enforcement_mode": "enforce",
                    "severity": "critical",
                    "enabled": true
                }
            ],
            "signature": "sha256=deadbeef"
        });

        Mock::given(method("GET"))
            .and(path("/api/v1/policies/bundle"))
            .respond_with(ResponseTemplate::new(200).set_body_json(&bundle_json))
            .mount(&mock_server)
            .await;

        let client = HttpEnterpriseClient::new();
        let service = EnterpriseServiceImpl::new(client);
        let ent_config = EnterpriseConfig {
            api_url: format!("{}/api/v1", mock_server.uri()),
            api_key: "test-key".to_string(),
            team_id: uuid::Uuid::parse_str("00000000-0000-0000-0000-000000000001").unwrap(),
            fetch_policies: true,
            enforce_policies: true,
            post_audit: true,
            policy_cache_ttl_secs: 300,
        };

        let result = service.fetch_policy_bundle(&ent_config).await;
        match result {
            Err(EnterpriseError::SignatureMismatch { .. }) => {} // expected
            other => panic!("Expected SignatureMismatch, got: {:?}", other),
        }
    }
}
