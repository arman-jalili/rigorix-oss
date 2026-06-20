//! Service interfaces (use cases) for the Security Configuration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md
//! Implements: Contract Freeze — SecurityValidationService, ForkDetectionService,
//! SecretMaskingService, TokenValidationService, UrlAllowlistService,
//! HmacSigningService, PolicyLoadingService traits
//! Issue: issue-contract-freeze
//!
//! These traits define the application-level operations for security validation,
//! fork detection, secret masking, token validation, URL allowlisting, HMAC
//! signing, and policy loading. All methods are async and return domain error types.
//!
//! # Contract (Frozen)
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs defined in `dto/`
//! - All methods are async (use `async-trait` for trait object safety)
//! - No implementation — only contract signatures

use async_trait::async_trait;

use crate::security_config::domain::{ActionMode, HmacKey, SecurityError};

use super::dto::{
    DetectForkInput, DetectForkOutput, HmacSignInput, HmacSignOutput, HmacVerifyInput,
    HmacVerifyOutput, LoadOrgPolicyInput, LoadOrgPolicyOutput, LoadPolicyInput, LoadPolicyOutput,
    MaskSecretsInput, MaskSecretsOutput, ValidateSecurityInput, ValidateSecurityOutput,
    ValidateTokenInput, ValidateTokenOutput, ValidateUrlInput, ValidateUrlOutput,
};

/// Application service for orchestrating all pre-flight security checks.
///
/// Implements the `SecurityValidator` component from the architecture doc.
/// Runs all security checks in the correct order and short-circuits on
/// critical violations:
///
/// 1. Fork detection (block secret exposure)
/// 2. Secret masking (before any logging)
/// 3. Token permission check
/// 4. URL allowlist check
/// 5. Security level determination
///
/// # Contract (Frozen)
/// - `validate()` is the primary entry point
/// - Returns a complete `SecurityContext` with all check results
/// - Short-circuits on `Blocked` security level
/// - Secrets are masked BEFORE any other check that might log
#[async_trait]
pub trait SecurityValidationService: Send + Sync {
    /// Run all pre-flight security checks in order.
    ///
    /// Execution order:
    /// 1. Fork detection
    /// 2. Secret masking (first, before any logging)
    /// 3. Token validation
    /// 4. URL allowlist validation
    /// 5. Security level resolution
    ///
    /// Returns a complete `ValidateSecurityOutput` with the security context
    /// and any warnings.
    async fn validate(
        &self,
        input: ValidateSecurityInput,
    ) -> Result<ValidateSecurityOutput, SecurityError>;

    /// Check if the security level allows operations.
    async fn is_operation_allowed(&self, level: &super::super::domain::SecurityLevel) -> bool;

    /// Get the effective security level from the current context.
    async fn current_security_level(&self) -> super::super::domain::SecurityLevel;
}

/// Application service for detecting PRs from forked repositories.
///
/// Implements the `ForkDetector` component from the architecture doc.
/// Compares the head repository against the base repository to detect
/// forked PRs where secrets are not available.
///
/// # Contract (Frozen)
/// - `detect()` compares `GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME` against `GITHUB_REPOSITORY`
/// - Returns `true` only when running in a PR context and repos differ
/// - Returns `false` for non-PR events (workflow_dispatch, push)
#[async_trait]
pub trait ForkDetectionService: Send + Sync {
    /// Detect if this is a fork PR.
    ///
    /// Compares head repo against base repo using GitHub-provided env vars.
    async fn detect(&self, input: DetectForkInput) -> Result<DetectForkOutput, SecurityError>;

    /// Get the fork owner's username.
    /// Returns `None` if not a fork or env var is absent.
    async fn fork_owner(&self) -> Result<Option<String>, SecurityError>;

    /// Get the head repository full name.
    async fn head_repo(&self) -> Result<Option<String>, SecurityError>;

    /// Get the base repository full name.
    async fn base_repo(&self) -> Result<String, SecurityError>;
}

/// Application service for masking secrets from workflow logs.
///
/// Implements the `SecretMasker` component from the architecture doc.
/// Uses GitHub Actions `::add-mask::<value>` workflow commands to prevent
/// secrets from appearing in CI logs.
///
/// # Contract (Frozen)
/// - `mask()` must be called BEFORE any logging
/// - Logs `::add-mask::<value>` for each secret
/// - Empty secrets are silently ignored
#[async_trait]
pub trait SecretMaskingService: Send + Sync {
    /// Mask a single secret value.
    ///
    /// After this call, the value will appear as `***` in workflow logs.
    /// Must be called before any logging that might include the secret.
    async fn mask(&self, secret: &str) -> Result<(), SecurityError>;

    /// Mask multiple secrets at once.
    async fn mask_all(&self, input: MaskSecretsInput) -> Result<MaskSecretsOutput, SecurityError>;

    /// Check if a value contains any known secret patterns.
    /// Useful for log scrubbing before output.
    async fn contains_secret(&self, text: &str) -> bool;
}

/// Application service for validating GitHub token permissions.
///
/// Implements the `TokenValidator` component from the architecture doc.
/// Validates the token by calling the GitHub API and checking scopes.
///
/// # Contract (Frozen)
/// - `validate()` calls GitHub API `/user` or `/installation/repositories`
/// - Returns structured results with available and missing scopes
/// - Mode-specific permission requirements
#[async_trait]
pub trait TokenValidationService: Send + Sync {
    /// Validate the GitHub token and check permissions.
    ///
    /// Makes an API call to GitHub to verify the token is valid and
    /// has the required scopes for the given action mode.
    async fn validate(
        &self,
        input: ValidateTokenInput,
    ) -> Result<ValidateTokenOutput, SecurityError>;

    /// Check if the token is valid (basic auth check).
    async fn is_token_valid(&self, token: &str) -> Result<bool, SecurityError>;

    /// Get the required permissions for a given mode.
    fn required_permissions(mode: ActionMode) -> &'static [(&'static str, &'static str)];
}

/// Application service for validating backend URLs against an allowlist.
///
/// Implements the `UrlAllowlist` component from the architecture doc.
/// Loads allowed hosts from `.rigorix/security.toml` and validates URLs
/// against the configured allowlist.
///
/// # Contract (Frozen)
/// - `validate()` parses the URL and checks host against allowlist
/// - Returns `Ok(true)` if no allowlist configured (fail-open for dev)
/// - Returns `UrlBlocked` error if host is not allowed
/// - Host matching uses suffix/prefix matching
#[async_trait]
pub trait UrlAllowlistService: Send + Sync {
    /// Validate a URL against the configured allowlist.
    async fn validate(&self, input: ValidateUrlInput) -> Result<ValidateUrlOutput, SecurityError>;

    /// Load the allowlist from the security policy.
    async fn load_allowlist(&self) -> Result<Vec<String>, SecurityError>;

    /// Add a host to the runtime allowlist (for programmatic additions).
    async fn add_allowed_host(&self, host: String) -> Result<(), SecurityError>;

    /// Check if a host matches any allowlist entry.
    async fn is_host_allowed(&self, host: &str) -> Result<bool, SecurityError>;
}

/// Application service for HMAC-SHA256 signing and verification.
///
/// Implements the `HmacSigner` component from the architecture doc.
/// Signs audit record payloads and verifies signatures using constant-time
/// comparison to prevent timing attacks.
///
/// # Contract (Frozen)
/// - `sign()` produces hex-encoded HMAC-SHA256 signatures
/// - `verify()` uses constant-time comparison
/// - `generate_key()` produces cryptographically random 32-byte keys
/// - Key rotation is supported via key_id tracking
#[async_trait]
pub trait HmacSigningService: Send + Sync {
    /// Sign a payload and return the hex-encoded signature.
    async fn sign(&self, input: HmacSignInput) -> Result<HmacSignOutput, SecurityError>;

    /// Verify a signature against a payload (constant-time comparison).
    async fn verify(&self, input: HmacVerifyInput) -> Result<HmacVerifyOutput, SecurityError>;

    /// Generate a new HMAC key (32 random bytes).
    async fn generate_key(&self) -> Result<HmacKey, SecurityError>;

    /// Load the HMAC signing key from the configured source.
    async fn load_key(&self) -> Result<HmacKey, SecurityError>;

    /// Rotate the HMAC signing key.
    async fn rotate_key(&self) -> Result<HmacKey, SecurityError>;
}

/// Application service for loading security policy configuration.
///
/// Implements the `OrgPolicyLoader` component from the architecture doc.
/// Loads `.rigorix/security.toml` and organization-level security policies.
///
/// # Contract (Frozen)
/// - Loads from `.rigorix/security.toml` by default
/// - Supports organization-level policy URLs
/// - Returns defaults if file not found (non-fatal)
#[async_trait]
pub trait PolicyLoadingService: Send + Sync {
    /// Load the security policy from configuration.
    async fn load_policy(&self, input: LoadPolicyInput) -> Result<LoadPolicyOutput, SecurityError>;

    /// Load an organization-level security policy.
    async fn load_org_policy(
        &self,
        input: LoadOrgPolicyInput,
    ) -> Result<LoadOrgPolicyOutput, SecurityError>;

    /// Get the configured allowed hosts from the policy.
    async fn allowed_hosts(&self) -> Result<Vec<String>, SecurityError>;

    /// Get the HMAC key configuration.
    async fn hmac_config(&self) -> Result<(String, u32), SecurityError>;
}
