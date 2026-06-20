//! Error types for the Security Configuration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md#error
//! Implements: Contract Freeze — SecurityError enum
//! Issue: issue-contract-freeze
//!
//! All errors use `thiserror` derive macros. No `anyhow` in library code.
//!
//! # Contract (Frozen)
//! - `SecurityError` is the single error type for this module
//! - Each variant carries structured context for error reporting
//! - Implements `std::error::Error` for library compatibility

use thiserror::Error;

/// Errors that can occur during security validation.
#[derive(Debug, Error)]
pub enum SecurityError {
    /// The PR originates from a forked repository — secrets not available.
    #[error(
        "Fork PR detected: repository secrets are not available for pull requests from forks. Head repo '{head_repo}' differs from base repo '{base_repo}'."
    )]
    ForkDetected {
        /// The head repository full name (e.g., "user/repo").
        head_repo: String,
        /// The base repository full name (e.g., "org/repo").
        base_repo: String,
    },

    /// GitHub token validation failed (network error or invalid token).
    #[error("GitHub token validation failed: {detail}")]
    TokenValidationFailed {
        /// Details about the failure.
        detail: String,
        /// HTTP status code if available.
        status_code: Option<u16>,
    },

    /// Token is valid but lacks required permissions.
    #[error(
        "GitHub token has insufficient permissions. Required: {required:?}, available: {available:?}"
    )]
    TokenInsufficient {
        /// Permissions the token requires but does not have.
        required: Vec<String>,
        /// Permissions the token actually has.
        available: Vec<String>,
    },

    /// A URL was blocked by the allowlist.
    #[error("URL blocked by allowlist: {url}. Allowed hosts: {allowed:?}")]
    UrlBlocked {
        /// The URL that was blocked.
        url: String,
        /// The list of allowed hosts.
        allowed: Vec<String>,
    },

    /// A URL could not be parsed.
    #[error("Invalid URL: {0}")]
    InvalidUrl(String),

    /// HMAC signature verification failed.
    #[error("HMAC signature verification failed: expected '{expected}', got '{actual}'")]
    HmacVerificationFailed {
        /// The expected signature.
        expected: String,
        /// The actual signature provided.
        actual: String,
    },

    /// HMAC signing key not available.
    #[error("HMAC signing key not available: {detail}")]
    HmacKeyMissing {
        /// Details about the missing key.
        detail: String,
    },

    /// Organization policy could not be loaded.
    #[error("Failed to load organization policy from '{path}': {detail}")]
    OrgPolicyLoadFailed {
        /// The path or URL that was attempted.
        path: String,
        /// Details about the failure.
        detail: String,
    },

    /// Security policy file missing or unreadable.
    #[error("Security policy file not found at '{path}': {detail}")]
    PolicyFileNotFound {
        /// The expected path.
        path: String,
        /// Additional details.
        detail: String,
    },

    /// Security policy parse error.
    #[error("Failed to parse security policy: {detail}")]
    PolicyParseError {
        /// Parse error description.
        detail: String,
    },

    /// IO error (file system, network).
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    /// JSON serialization/deserialization error.
    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    /// YAML serialization/deserialization error.
    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),

    /// TOML parse error.
    #[error("TOML parse error: {detail}")]
    TomlParseError {
        /// Parse error description.
        detail: String,
    },

    /// HTTP client error.
    #[error("HTTP error: {detail}")]
    HttpError {
        /// Error description.
        detail: String,
    },

    /// Internal invariant violation (should not happen).
    #[error("Internal error: {detail}")]
    Internal {
        /// Error description.
        detail: String,
    },
}

impl SecurityError {
    /// Whether the error is retriable.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            SecurityError::Io(_)
                | SecurityError::HttpError { .. }
                | SecurityError::TokenValidationFailed { .. }
                | SecurityError::OrgPolicyLoadFailed { .. }
        )
    }

    /// Whether the error is a security violation (action should abort).
    pub fn is_security_violation(&self) -> bool {
        matches!(
            self,
            SecurityError::ForkDetected { .. }
                | SecurityError::TokenInsufficient { .. }
                | SecurityError::UrlBlocked { .. }
                | SecurityError::HmacVerificationFailed { .. }
        )
    }
}
