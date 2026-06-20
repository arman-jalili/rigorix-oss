//! Domain types for the Security Configuration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md#types
//! Implements: Contract Freeze — SecurityContext, SecurityLevel, and related types
//! Issue: issue-contract-freeze
//!
//! These are the core domain types that represent the security validation
//! context, including fork detection results, token permissions, secret masking
//! status, URL allowlist validation, and HMAC signing. They serve as the
//! frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces
//! - All domain types are serializable (Serialize + Deserialize)

use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// SecurityContext
// ---------------------------------------------------------------------------

/// Results of pre-flight security validation.
///
/// Built during Phase 0 of the action lifecycle. All fields must be
/// validated before any operation begins.
///
/// ## Validation Order
///
/// 1. Fork detection (block secret exposure)
/// 2. Secret masking (before any logging)
/// 3. Token permission check
/// 4. URL allowlist check
/// 5. Policy integrity check (deferred to PolicyLoader)
/// 6. Security level determination
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityContext {
    /// Whether this is a PR from a forked repository.
    pub is_fork_pr: bool,

    /// Whether the GitHub token has all required permissions.
    pub has_required_permissions: bool,

    /// Whether all API keys have been masked from workflow logs.
    pub api_key_masked: bool,

    /// Whether the policy file was modified in this PR (compared to base).
    pub policy_changed_from_base: bool,

    /// Whether the backend URL is in the configured allowlist.
    pub backend_url_allowed: bool,

    /// Whether organization-level security policy was loaded.
    pub org_policy_loaded: bool,

    /// The effective security level after validation.
    pub security_level: SecurityLevel,
}

impl SecurityContext {
    /// Create a new SecurityContext with all fields defaulting to safe values.
    ///
    /// Safe defaults assume the most restrictive posture:
    /// - `is_fork_pr`: `false` (assume safe until proven otherwise)
    /// - `has_required_permissions`: `false` (must be explicitly validated)
    /// - `api_key_masked`: `false` (must be explicitly done)
    /// - `security_level`: `Blocked` (must pass checks to unlock)
    pub fn new() -> Self {
        Self {
            is_fork_pr: false,
            has_required_permissions: false,
            api_key_masked: false,
            policy_changed_from_base: false,
            backend_url_allowed: false,
            org_policy_loaded: false,
            security_level: SecurityLevel::Blocked,
        }
    }
}

impl Default for SecurityContext {
    fn default() -> Self {
        Self::new()
    }
}

// ---------------------------------------------------------------------------
// SecurityLevel
// ---------------------------------------------------------------------------

/// The effective security level after all pre-flight checks.
///
/// Determines what operations are permitted during the action execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum SecurityLevel {
    /// All checks passed, full operation allowed.
    Full,

    /// Fork PR or restricted environment: secret-dependent operations skipped.
    Restricted,

    /// Critical security violation: all operations blocked.
    Blocked,
}

impl SecurityLevel {
    /// Whether operations are permitted at this level.
    pub fn is_allowed(&self) -> bool {
        matches!(self, SecurityLevel::Full | SecurityLevel::Restricted)
    }

    /// Whether secret-dependent operations are permitted.
    pub fn secrets_allowed(&self) -> bool {
        matches!(self, SecurityLevel::Full)
    }
}

// ---------------------------------------------------------------------------
// ActionMode
// ---------------------------------------------------------------------------

/// The mode of action execution, used to determine required permissions.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum ActionMode {
    /// Mode A: Policy governance — PR diff analysis + policy enforcement.
    Governance,

    /// Mode B: Full execution — plan → execute → validate → persist.
    Run,

    /// Mode B: Self-correcting validation loop.
    Validate,

    /// Mode B: Planning phase only.
    Plan,

    /// Show current execution status.
    Status,

    /// Auto-detect from event context.
    Auto,
}

impl ActionMode {
    /// Get the required GitHub token permissions for this mode.
    pub fn required_scopes(&self) -> &'static [(&'static str, &'static str)] {
        match self {
            ActionMode::Run | ActionMode::Validate | ActionMode::Plan => &[
                ("contents", "write"),
                ("pull-requests", "write"),
                ("issues", "write"),
                ("statuses", "write"),
            ],
            ActionMode::Governance => &[
                ("contents", "read"),
                ("pull-requests", "write"),
                ("statuses", "write"),
            ],
            ActionMode::Status | ActionMode::Auto => &[("contents", "read")],
        }
    }
}

// ---------------------------------------------------------------------------
// HmacKey
// ---------------------------------------------------------------------------

/// An HMAC signing key with metadata for key rotation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HmacKey {
    /// The raw key bytes (32 bytes for SHA-256).
    pub key: Vec<u8>,

    /// A human-readable identifier for the key (for key rotation).
    pub key_id: String,

    /// ISO 8601 timestamp when the key was created.
    pub created_at: String,

    /// ISO 8601 timestamp when the key expires.
    pub expires_at: String,
}

// ---------------------------------------------------------------------------
// SecurityPolicy
// ---------------------------------------------------------------------------

/// Organization-level security policy configuration.
///
/// Loaded from `.rigorix/security.toml` or organization-level URL.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SecurityPolicy {
    /// Allowed backend API hosts (prefix matching).
    pub allowed_hosts: Vec<String>,

    /// Path to organization-wide policy file.
    pub org_policy_path: Option<String>,

    /// Whether org policy is required (fail if unreachable).
    pub org_policy_required: bool,

    /// Environment variable name for the HMAC signing key.
    pub hmac_key_env_var: Option<String>,

    /// HMAC key rotation interval in days.
    pub hmac_rotation_days: Option<u32>,
}
