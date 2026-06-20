//! Data Transfer Objects for the Security Configuration module.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md
//! Implements: Contract Freeze — DTO schemas for security validation, fork detection,
//! secret masking, token validation, URL allowlisting, and HMAC signing operations
//! Issue: issue-contract-freeze
//!
//! DTOs define the input/output contracts for service operations.
//! They carry validation metadata and documentation but no behavior.
//!
//! # Contract (Frozen)
//! - Every service operation has a dedicated input and output DTO
//! - DTOs are serializable (JSON for API, TOML for config)
//! - Validation constraints are documented in field docs

use serde::{Deserialize, Serialize};

use crate::security_config::domain::{
    ActionMode, HmacKey, SecurityContext, SecurityLevel, SecurityPolicy,
};

// ---------------------------------------------------------------------------
// Security Validation DTOs
// ---------------------------------------------------------------------------

/// Input for running pre-flight security validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateSecurityInput {
    /// The GitHub token to validate.
    pub github_token: String,

    /// Optional API key to mask (e.g., ANTHROPIC_API_KEY, OPENAI_API_KEY).
    pub api_key: Option<String>,

    /// Path to the policy file (for tamper detection).
    pub policy_path: String,

    /// Backend audit URL to validate.
    pub backend_url: Option<String>,

    /// The action mode to check permissions against.
    pub mode: ActionMode,
}

/// Output from running pre-flight security validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateSecurityOutput {
    /// The complete security context with all check results.
    pub context: SecurityContext,

    /// Whether all checks passed.
    pub valid: bool,

    /// List of validation warnings (non-blocking).
    pub warnings: Vec<String>,

    /// Duration of validation in milliseconds.
    pub duration_ms: u64,
}

// ---------------------------------------------------------------------------
// Fork Detection DTOs
// ---------------------------------------------------------------------------

/// Input for detecting fork PRs.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DetectForkInput {
    /// Override environment for testing (maps variable name → value).
    /// If `None`, reads from real `std::env::var`.
    pub env_override: Option<std::collections::HashMap<String, String>>,
}

/// Output from fork detection.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DetectForkOutput {
    /// Whether this is a fork PR.
    pub is_fork: bool,
    /// Head repository full name (e.g., "user/repo").
    pub head_repo: Option<String>,
    /// Base repository full name (e.g., "org/repo").
    pub base_repo: Option<String>,
    /// Fork owner username (if fork).
    pub fork_owner: Option<String>,
}

// ---------------------------------------------------------------------------
// Secret Masking DTOs
// ---------------------------------------------------------------------------

/// Input for masking secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskSecretsInput {
    /// List of secrets to mask.
    pub secrets: Vec<String>,
}

/// Output from masking secrets.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MaskSecretsOutput {
    /// Number of secrets masked.
    pub masked_count: u32,
    /// Hints about which secrets were masked (not the values).
    pub masked_hints: Vec<String>,
}

// ---------------------------------------------------------------------------
// Token Validation DTOs
// ---------------------------------------------------------------------------

/// Input for validating the GitHub token.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateTokenInput {
    /// The GitHub token to validate.
    pub token: String,
    /// The action mode to check required permissions for.
    pub mode: ActionMode,
}

/// Output from token validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateTokenOutput {
    /// Whether the token is valid (responds to API calls).
    pub valid: bool,
    /// Whether the token has the required permissions for the mode.
    pub has_required_permissions: bool,
    /// List of permissions the token has.
    pub available_scopes: Vec<String>,
    /// List of required permissions the token is missing.
    pub missing_scopes: Vec<String>,
}

// ---------------------------------------------------------------------------
// URL Allowlist DTOs
// ---------------------------------------------------------------------------

/// Input for validating a URL against the allowlist.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateUrlInput {
    /// The URL to validate.
    pub url: String,
    /// Override allowlist for testing.
    pub allowlist_override: Option<Vec<String>>,
}

/// Output from URL validation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidateUrlOutput {
    /// Whether the URL is allowed.
    pub allowed: bool,
    /// The host component of the URL.
    pub host: String,
    /// The allowlist entries that were checked against.
    pub checked_against: Vec<String>,
}

// ---------------------------------------------------------------------------
// HMAC Signing DTOs
// ---------------------------------------------------------------------------

/// Input for signing a payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmacSignInput {
    /// The payload bytes to sign.
    pub payload: Vec<u8>,
    /// Optional key override. If None, uses configured key.
    pub key_override: Option<Vec<u8>>,
}

/// Output from HMAC signing.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmacSignOutput {
    /// The hex-encoded signature.
    pub signature: String,
    /// Key identifier used.
    pub key_id: String,
}

/// Input for verifying an HMAC signature.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmacVerifyInput {
    /// The original payload bytes.
    pub payload: Vec<u8>,
    /// The signature to verify.
    pub signature: String,
    /// Optional key override. If None, uses configured key.
    pub key_override: Option<Vec<u8>>,
}

/// Output from HMAC signature verification.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmacVerifyOutput {
    /// Whether the signature is valid.
    pub valid: bool,
    /// Key identifier used for verification.
    pub key_id: String,
}

// ---------------------------------------------------------------------------
// Security Policy DTOs
// ---------------------------------------------------------------------------

/// Input for loading the security policy.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct LoadPolicyInput {
    /// Override path to security.toml. If None, defaults to `.rigorix/security.toml`.
    pub path_override: Option<String>,
    /// Override content for testing (TOML string).
    pub content_override: Option<String>,
}

/// Output from loading the security policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadPolicyOutput {
    /// The parsed security policy.
    pub policy: SecurityPolicy,
    /// Path the policy was loaded from.
    pub source_path: String,
}

/// Input for loading an organization-level policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadOrgPolicyInput {
    /// URL or path to the org policy file.
    pub policy_url: String,
    /// Whether the org policy is required (fail if unreachable).
    pub required: bool,
}

/// Output from loading an organization-level policy.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LoadOrgPolicyOutput {
    /// The loaded org policy rules.
    pub rules: Vec<String>,
    /// Number of rules loaded.
    pub rules_count: u32,
    /// Whether the policy was loaded from cache.
    pub from_cache: bool,
}
