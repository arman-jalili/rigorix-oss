//! HTTP API contracts for Policy Engine endpoints.
//!
//! @canonical .pi/architecture/modules/policy-engine.md
//! Implements: Contract Freeze — HTTP endpoint contracts and error formats
//! Issue: issue-contract-freeze
//!
//! Defines endpoint paths, methods, request/response schemas, and error
//! response formats. These contracts are framework-agnostic — they describe
//! the API surface that any HTTP server implementation must satisfy.
//!
//! # Contract (Frozen)
//! - All endpoints documented with method, path, request, and response types
//! - Error responses follow a unified format
//! - No framework-specific annotations (axum/actix/warp annotations added by implementation)

use serde::{Deserialize, Serialize};

use crate::policy_engine::application::dto::{
    ActionOutput, EvaluatePolicyOutput, GetActiveRulesOutput, RuleSummary,
};
use crate::policy_engine::domain::PolicyConfig;

// ---------------------------------------------------------------------------
// API Base Path
// ---------------------------------------------------------------------------

/// All policy engine endpoints are served under this base path.
pub const API_BASE_PATH: &str = "/api/v1/policy";

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/policy/evaluate
// ---------------------------------------------------------------------------

/// POST /api/v1/policy/evaluate
///
/// Evaluate all loaded policy rules against a LaneContext.
/// Returns the flat list of actions in priority order.
///
/// **Request:** `EvaluatePolicyRequest`
/// **Response:** `200 OK` with `EvaluatePolicyResponse`
pub const EVALUATE_PATH: &str = "/api/v1/policy/evaluate";
pub const EVALUATE_METHOD: &str = "POST";

/// Request body for POST /api/v1/policy/evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatePolicyRequest {
    /// The lane ID being evaluated.
    pub lane_id: String,

    /// The quality level achieved (0-5).
    pub green_level: u8,

    /// Seconds since the last commit on the branch.
    pub branch_freshness_secs: u64,

    /// Whether the lane is blocked at startup.
    pub startup_blocked: bool,

    /// Whether the lane is externally blocked.
    pub external_blocked: bool,

    /// Review status: "pending", "approved", or "rejected".
    pub review_status: String,

    /// Diff scope: "full" or "scoped".
    pub diff_scope: String,

    /// Whether lane execution completed.
    pub completed: bool,

    /// Whether the lane has been reconciled.
    pub reconciled: bool,

    /// Optional rule name filter.
    pub rule_filter: Option<Vec<String>>,
}

/// Response body for POST /api/v1/policy/evaluate.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluatePolicyResponse {
    /// The lane ID that was evaluated.
    pub lane_id: String,

    /// Flat list of actions from all matching rules, in priority order.
    pub actions: Vec<ActionOutput>,

    /// Number of matching rules.
    pub matching_rule_count: u32,

    /// Total number of rules evaluated.
    pub rules_evaluated: u32,

    /// Whether any rules matched.
    pub matched: bool,
}

impl From<EvaluatePolicyOutput> for EvaluatePolicyResponse {
    fn from(output: EvaluatePolicyOutput) -> Self {
        Self {
            lane_id: output.lane_id,
            actions: output.actions,
            matching_rule_count: output.matching_rule_count,
            rules_evaluated: output.rules_evaluated,
            matched: output.matched,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: GET /api/v1/policy/rules
// ---------------------------------------------------------------------------

/// GET /api/v1/policy/rules
///
/// Get all currently loaded (active) policy rules.
///
/// **Response:** `200 OK` with `GetRulesResponse`
pub const RULES_PATH: &str = "/api/v1/policy/rules";
pub const RULES_METHOD: &str = "GET";

/// Response body for GET /api/v1/policy/rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GetRulesResponse {
    pub rules: Vec<RuleSummary>,
    pub total_count: u32,
}

impl From<GetActiveRulesOutput> for GetRulesResponse {
    fn from(output: GetActiveRulesOutput) -> Self {
        Self {
            rules: output.rules,
            total_count: output.total_count,
        }
    }
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/policy/rules
// ---------------------------------------------------------------------------

/// POST /api/v1/policy/rules
///
/// Load/replace policy rules from a configuration payload.
///
/// **Request:** `LoadRulesRequest`
/// **Response:** `200 OK` with `LoadRulesResponse`
pub const LOAD_RULES_PATH: &str = "/api/v1/policy/rules";
pub const LOAD_RULES_METHOD: &str = "POST";

/// Request body for POST /api/v1/policy/rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadRulesRequest {
    /// The policy configuration in PolicyConfig format.
    pub config: PolicyConfig,

    /// Whether to replace all existing rules. If false, merges by name.
    #[serde(default = "default_replace_all")]
    pub replace_all: bool,
}

fn default_replace_all() -> bool {
    true
}

/// Response body for POST /api/v1/policy/rules.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadRulesResponse {
    pub loaded_count: u32,
    pub replaced_count: u32,
    pub rule_names: Vec<String>,
    pub success: bool,
}

// ---------------------------------------------------------------------------
// Endpoint: POST /api/v1/policy/reload
// ---------------------------------------------------------------------------

/// POST /api/v1/policy/reload
///
/// Reload policy rules from the last loaded source.
///
/// **Response:** `200 OK` with `ReloadRulesResponse`
pub const RELOAD_PATH: &str = "/api/v1/policy/reload";
pub const RELOAD_METHOD: &str = "POST";

/// Response body for POST /api/v1/policy/reload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReloadRulesResponse {
    pub success: bool,
    pub rule_count: u32,
    pub source: String,
    pub error: Option<String>,
}

// ---------------------------------------------------------------------------
// Unified Error Response Format
// ---------------------------------------------------------------------------

/// Standard error response for all Policy Engine API endpoints.
///
/// All 4xx/5xx responses use this format.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiErrorResponse {
    /// HTTP status code.
    pub status: u16,
    /// Machine-readable error code.
    pub code: String,
    /// Human-readable error message.
    pub message: String,
    /// Detailed error context (optional).
    pub details: Option<serde_json::Value>,
    /// Request ID for tracing (if available).
    pub request_id: Option<String>,
}

/// Standardized error codes for Policy Engine API.
pub mod error_codes {
    /// No matching rules found for the given context.
    pub const NO_MATCHING_RULE: &str = "POLICY_NO_MATCHING_RULE";
    /// Invalid policy configuration.
    pub const INVALID_CONFIGURATION: &str = "POLICY_INVALID_CONFIGURATION";
    /// Policy rule not found.
    pub const RULE_NOT_FOUND: &str = "POLICY_RULE_NOT_FOUND";
    /// Internal server error.
    pub const INTERNAL_ERROR: &str = "POLICY_INTERNAL_ERROR";
}

/// HTTP status code mappings for Policy Engine errors.
pub mod status_codes {
    pub const NO_MATCHING_RULE: u16 = 404;
    pub const INVALID_CONFIGURATION: u16 = 400;
    pub const RULE_NOT_FOUND: u16 = 404;
    pub const INTERNAL_ERROR: u16 = 500;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::policy_engine::domain::config::RuleDefinition;
    use crate::policy_engine::domain::{PolicyAction, PolicyCondition, PolicyConfig};

    #[test]
    fn test_evaluate_policy_request_serde() {
        let request = EvaluatePolicyRequest {
            lane_id: "lane-1".to_string(),
            green_level: 3,
            branch_freshness_secs: 100,
            startup_blocked: false,
            external_blocked: false,
            review_status: "pending".to_string(),
            diff_scope: "scoped".to_string(),
            completed: true,
            reconciled: false,
            rule_filter: None,
        };
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: EvaluatePolicyRequest = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.lane_id, "lane-1");
        assert!(deserialized.completed);
    }

    #[test]
    fn test_evaluate_policy_response_serde() {
        let response = EvaluatePolicyResponse {
            lane_id: "lane-1".to_string(),
            actions: vec![ActionOutput {
                rule_name: "closeout".to_string(),
                priority: 10,
                action: PolicyAction::CloseoutLane,
            }],
            matching_rule_count: 1,
            rules_evaluated: 5,
            matched: true,
        };
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: EvaluatePolicyResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.lane_id, "lane-1");
    }

    #[test]
    fn test_get_rules_response_serde() {
        let response = GetRulesResponse {
            rules: vec![],
            total_count: 0,
        };
        let json = serde_json::to_string(&response).unwrap();
        let deserialized: GetRulesResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.total_count, 0);
    }

    #[test]
    fn test_load_rules_request_serde() {
        let request = LoadRulesRequest {
            config: PolicyConfig::single(RuleDefinition {
                name: "test".to_string(),
                condition: PolicyCondition::LaneCompleted,
                action: PolicyAction::CloseoutLane,
                priority: 10,
            }),
            replace_all: true,
        };
        let json = serde_json::to_string(&request).unwrap();
        let deserialized: LoadRulesRequest = serde_json::from_str(&json).unwrap();
        assert!(deserialized.replace_all);
        assert_eq!(deserialized.config.rules.len(), 1);
    }

    #[test]
    fn test_api_error_response() {
        let error = ApiErrorResponse {
            status: 404,
            code: error_codes::NO_MATCHING_RULE.to_string(),
            message: "No matching rules for lane-1".to_string(),
            details: None,
            request_id: Some("req-123".to_string()),
        };
        let json = serde_json::to_string(&error).unwrap();
        let deserialized: ApiErrorResponse = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.status, 404);
    }

    #[test]
    fn test_empty_rule_filter_default() {
        assert!(default_replace_all());
    }
}
