//! Event payload schemas for the Security Configuration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md
//! Implements: Contract Freeze — SecurityEvent payload schemas
//! Issue: issue-contract-freeze
//!
//! These events are emitted whenever security checks run, forks are detected,
//! secrets are masked, or signatures are verified. Consumers (audit, action-entrypoint)
//! subscribe to these event types.
//!
//! # Contract (Frozen)
//! - Each event carries the full context needed by consumers
//! - No internal implementation details exposed
//! - Events are serializable for audit logging

use serde::{Deserialize, Serialize};

use crate::security_config::domain::SecurityLevel;

/// Events emitted by the Security Configuration module.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum SecurityEvent {
    /// Pre-flight security validation completed.
    ValidationCompleted {
        /// The resolved security level.
        security_level: SecurityLevel,
        /// Whether the context was validated successfully.
        success: bool,
        /// Count of checks that passed.
        checks_passed: u32,
        /// Count of checks that failed.
        checks_failed: u32,
    },

    /// Pre-flight validation failed — action is blocked.
    ValidationFailed {
        /// The security level that caused the failure.
        security_level: SecurityLevel,
        /// Human-readable failure reason.
        reason: String,
    },

    /// Fork PR detected.
    ForkDetected {
        /// Head repository full name.
        head_repo: String,
        /// Base repository full name.
        base_repo: String,
        /// Fork owner username.
        fork_owner: Option<String>,
    },

    /// A secret was masked from workflow logs.
    SecretMasked {
        /// Hint identifying the secret type (not the value).
        secret_hint: String,
    },

    /// Token permission check completed.
    TokenChecked {
        /// Whether the token has required permissions.
        has_permissions: bool,
        /// The action mode checked against.
        mode: String,
    },

    /// URL allowlist validation completed.
    UrlValidated {
        /// The URL that was validated.
        url: String,
        /// Whether the URL is allowed.
        allowed: bool,
    },

    /// HMAC signature was verified.
    HmacVerification {
        /// Whether the signature was valid.
        valid: bool,
        /// Key identifier used for verification.
        key_id: String,
    },

    /// HMAC signature was created.
    HmacSigned {
        /// Key identifier used for signing.
        key_id: String,
    },

    /// Organization policy was loaded.
    OrgPolicyLoaded {
        /// The path the policy was loaded from.
        path: String,
        /// Whether the load succeeded.
        success: bool,
        /// Number of policy rules loaded.
        rules_count: u32,
    },

    /// Security configuration was loaded from security.toml.
    SecurityConfigLoaded {
        /// Path to the config file.
        path: String,
        /// Number of allowed hosts configured.
        allowed_hosts_count: u32,
        /// Whether HMAC signing is configured.
        hmac_enabled: bool,
    },
}
