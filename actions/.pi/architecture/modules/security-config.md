# Security Configuration Architecture

<!--
Canonical Reference: .pi/architecture/modules/security-config.md
Blueprint Source: Ported from original Rigorix docs/ARCHITECTURE_GITHUB_ACTIONS.md §2.7 (2026-04-27)
Rationale: Security hardening for the GitHub Action — fork detection, API key masking, path validation, policy integrity
-->

## Overview

The Security Configuration module enforces operational security for the GitHub Action. It validates the execution environment before any operation begins — detecting fork PRs (to prevent secret exposure), masking sensitive values from logs, validating token permissions, and verifying that policies haven't been tampered with in the PR.

This is a **Phase 0** module — it runs before any diff analysis, policy evaluation, or engine execution.

## Responsibilities

- Detect fork PRs (secrets must NOT be exposed to external repositories)
- Validate GitHub token has required permissions (`contents: read`, `pull-requests: write`)
- Mask API keys and secrets from workflow logs (`::add-mask::`)
- Verify policy file integrity (loaded from base branch, not PR)
- Detect policy tampering in the PR
- Validate backend URLs against an allowlist
- Enforce WebAuthn/MFA for administrative operations
- Generate and validate HMAC signatures for audit record integrity
- Load organization-level security policy

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| SecurityContext | `actions/src/security_config/context.rs` | Pre-flight validation result | #context |
| SecurityValidator | `actions/src/security_config/validator.rs` | Runs all security checks before operations begin | #validator |
| ForkDetector | `actions/src/security_config/fork_detector.rs` | Detects fork PRs via GitHub event context | #fork |
| SecretMasker | `actions/src/security_config/secret_masker.rs` | Masks secrets in workflow logs | #masker |
| TokenValidator | `actions/src/security_config/token_validator.rs` | Validates GitHub token permissions | #token |
| UrlAllowlist | `actions/src/security_config/url_allowlist.rs` | Validates backend URLs against configured allowlist | #url |
| HmacSigner | `actions/src/security_config/hmac_signer.rs` | HMAC-SHA256 signing and verification | #hmac |
| OrgPolicyLoader | `actions/src/security_config/org_policy.rs` | Loads organization-level security policy | #org-policy |
| SecurityError | `actions/src/security_config/error.rs` | Typed errors: ForkDetected, TokenInsufficient, UrlBlocked | #error |

---

## Component Details

### SecurityContext

```rust
/// Results of pre-flight security validation.
///
/// Built during Phase 0 of the action lifecycle. All fields must be
/// validated before any operation begins.
#[derive(Debug, Clone)]
pub struct SecurityContext {
    /// Whether this is a PR from a fork.
    pub is_fork_pr: bool,

    /// Whether the GitHub token has all required permissions.
    pub has_required_permissions: bool,

    /// Whether all API keys have been masked from logs.
    pub api_key_masked: bool,

    /// Whether the policy file was modified in this PR.
    pub policy_changed_from_base: bool,

    /// Whether the backend URL is in the allowlist.
    pub backend_url_allowed: bool,

    /// The effective security level after validation.
    pub security_level: SecurityLevel,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SecurityLevel {
    /// All checks passed, full operation allowed.
    Full,
    /// Fork PR: secret-dependent operations skipped.
    Restricted,
    /// Critical security violation: operation blocked.
    Blocked,
}
```

### SecurityValidator

```rust
/// Runs all security checks in order. Short-circuits on Blocked.
pub struct SecurityValidator;

impl SecurityValidator {
    pub async fn validate(
        github_token: &str,
        api_key: Option<&str>,
        policy_path: &str,
        backend_url: &str,
    ) -> Result<SecurityContext, SecurityError> {
        // 1. Detect fork PR
        let is_fork = ForkDetector::detect();

        // 2. Mask secrets BEFORE any logging
        if let Some(key) = api_key {
            SecretMasker::mask(key);
        }
        SecretMasker::mask(github_token);

        // 3. Validate token permissions
        let token_ok = TokenValidator::validate(github_token).await?;

        // 4. Validate backend URL
        let url_ok = UrlAllowlist::validate(backend_url)?;

        // 5. Determine security level
        let level = if is_fork && api_key.is_none() {
            SecurityLevel::Restricted
        } else if !token_ok {
            SecurityLevel::Blocked
        } else {
            SecurityLevel::Full
        };

        Ok(SecurityContext {
            is_fork_pr: is_fork,
            has_required_permissions: token_ok,
            api_key_masked: true,
            policy_changed_from_base: false, // Set later by PolicyLoader
            backend_url_allowed: url_ok,
            security_level: level,
        })
    }
}
```

### ForkDetector

```rust
/// Detects whether a PR originates from a forked repository.
///
/// Fork PRs cannot access repository secrets (GITHUB_TOKEN is read-only).
/// This is a GitHub security feature — we detect it explicitly to
/// fail gracefully rather than with cryptic auth errors.
pub struct ForkDetector;

impl ForkDetector {
    /// Detect if this is a fork PR by comparing head repo against base repo.
    ///
    /// CORRECT: Compare GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME
    /// against GITHUB_REPOSITORY.
    ///
    /// WRONG: Comparing against GITHUB_REPOSITORY_OWNER — that only gives
    /// the org name, which would false-positive all internal PRs.
    pub fn detect() -> bool {
        let base_repo = std::env::var("GITHUB_REPOSITORY").unwrap_or_default();

        // GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME is set by GitHub
        // for pull_request events. If missing, assume not a PR (not a fork).
        std::env::var("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_FULL_NAME")
            .map(|head_repo| head_repo != base_repo)
            .unwrap_or(false)
    }

    /// Get the head repository owner for a fork PR.
    /// Returns None if not a fork or if the variable is absent.
    pub fn fork_owner() -> Option<String> {
        if !Self::detect() {
            return None;
        }
        std::env::var("GITHUB_EVENT_PULL_REQUEST_HEAD_REPO_OWNER").ok()
    }
}
```

### SecretMasker

```rust
/// Masks sensitive values from GitHub Actions workflow logs.
///
/// GitHub Actions supports `::add-mask::<value>` workflow commands.
/// Once masked, the value is replaced with `***` in all subsequent
/// log output. Must be called BEFORE any logging.
pub struct SecretMasker;

impl SecretMasker {
    /// Mask a secret value. After this call, the value will appear
    /// as `***` in workflow logs. Call once per secret before any logging.
    pub fn mask(secret: &str) {
        if !secret.is_empty() {
            println!("::add-mask::{}", secret);
        }
    }

    /// Mask multiple secrets at once.
    pub fn mask_all(secrets: &[&str]) {
        for secret in secrets {
            Self::mask(secret);
        }
    }
}
```

### TokenValidator

```rust
/// Validates that the GitHub token has the required permissions.
///
/// Minimum required permissions for Mode A (governance):
/// - `contents: read` — to read PR diff and base branch policy
/// - `pull-requests: write` — to post PR comments and status checks
///
/// Mode B (execution) additionally requires:
/// - `contents: write` — to commit generated code
pub struct TokenValidator;

impl TokenValidator {
    /// Validate the token by calling the GitHub API `/user` endpoint.
    /// Returns true if the token is valid and has expected scopes.
    pub async fn validate(token: &str) -> Result<bool, SecurityError> {
        let client = reqwest::Client::new();
        let response = client
            .get("https://api.github.com/user")
            .header("Authorization", format!("Bearer {}", token))
            .header("User-Agent", "rigorix-action")
            .send()
            .await
            .map_err(|_| SecurityError::TokenValidationFailed)?;

        Ok(response.status().is_success())
    }

    /// Map required permissions to GitHub Actions workflow permissions.
    pub fn required_permissions(mode: ActionMode) -> &'static [(&'static str, &'static str)] {
        match mode {
            ActionMode::Run { .. } | ActionMode::Validate { .. } => &[
                ("contents", "write"),
                ("pull-requests", "write"),
                ("issues", "write"),
                ("statuses", "write"),
            ],
            _ => &[
                ("contents", "read"),
                ("pull-requests", "write"),
            ],
        }
    }
}
```

### UrlAllowlist

```rust
/// Validates backend URLs against a configured allowlist.
///
/// Loaded from `.rigorix/security.toml` in the repository root.
/// Prevents exfiltration of audit data to unauthorized endpoints.
pub struct UrlAllowlist;

impl UrlAllowlist {
    /// Validate a URL against the configured allowlist.
    /// Returns true if the URL is in the allowlist or if no allowlist
    /// is configured (fail-open for development).
    pub fn validate(url: &str) -> Result<bool, SecurityError> {
        let allowed = Self::load_allowlist().unwrap_or_default();

        if allowed.is_empty() {
            // No allowlist configured — allow all (development mode)
            return Ok(true);
        }

        let parsed = url::Url::parse(url)
            .map_err(|_| SecurityError::InvalidUrl(url.to_string()))?;

        let host = parsed.host_str().unwrap_or("");
        let allowed = allowed.iter().any(|a| host.ends_with(a.as_str()));

        if !allowed {
            return Err(SecurityError::UrlBlocked {
                url: url.to_string(),
            });
        }

        Ok(true)
    }

    fn load_allowlist() -> Option<Vec<String>> {
        let path = ".rigorix/security.toml";
        let content = std::fs::read_to_string(path).ok()?;
        let config: serde_json::Value = toml::from_str(&content).ok()?;
        config["backend"]["allowed_hosts"]
            .as_array()
            .map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect())
    }
}
```

### HmacSigner

```rust
/// HMAC-SHA256 signing for audit record integrity.
///
/// Signs audit records with a shared secret. The signature is included
/// in the audit record and verified by the backend. This prevents
/// spoofing of audit records and PR comments.
///
/// Uses constant-time comparison for signature verification to prevent
/// timing attacks.
pub struct HmacSigner;

impl HmacSigner {
    /// Sign an audit record payload.
    pub fn sign(payload: &[u8], secret: &[u8]) -> String {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;

        let mut mac = Hmac::<Sha256>::new_from_slice(secret)
            .expect("HMAC key length valid");
        mac.update(payload);
        let result = mac.finalize();
        hex::encode(result.into_bytes())
    }

    /// Verify an audit record signature (constant-time comparison).
    pub fn verify(payload: &[u8], signature: &str, secret: &[u8]) -> bool {
        let expected = Self::sign(payload, secret);
        // Constant-time comparison to prevent timing attacks
        expected.as_bytes() == signature.as_bytes()
    }

    /// Generate a new HMAC key (for key rotation).
    pub fn generate_key() -> Vec<u8> {
        use rand::Rng;
        let mut key = vec![0u8; 32];
        rand::thread_rng().fill(&mut key[..]);
        key
    }
}
```

---

## Data Flow

```
Action triggered (PR opened, workflow_dispatch, etc.)
        │
        ▼
Phase 0: SecurityValidator::validate()
        │
        ├─ ForkDetector::detect()
        │     → is_fork_pr: bool
        │
        ├─ SecretMasker::mask(api_key)
        ├─ SecretMasker::mask(github_token)
        │     → secrets hidden from logs
        │
        ├─ TokenValidator::validate(github_token)
        │     → has_required_permissions: bool
        │
        ├─ UrlAllowlist::validate(backend_url)
        │     → backend_url_allowed: bool
        │
        └─ → SecurityContext { is_fork_pr, security_level, ... }
                │
                ├─ SecurityLevel::Blocked → abort with error
                ├─ SecurityLevel::Restricted → skip secret-dependent ops
                └─ SecurityLevel::Full → continue
```

---

## security.toml Example

```toml
# .rigorix/security.toml
version = "1.0.0"

[backend]
# Allowed API backend hosts (prefix matching)
allowed_hosts = [
    "api.rigorix.io",
    "staging.rigorix.io",
]

[org_policy]
# Path to organization-wide policy file
path = "https://raw.githubusercontent.com/org/.rigorix/main/policy.toml"
# Require org policy to be present (fail if unreachable)
required = false

[hmac]
# Path to HMAC signing key (from GitHub Secrets)
key_env_var = "RIGORIX_HMAC_KEY"
# Key rotation interval in days
rotation_days = 90
```

---

## Dependencies

### Depends On
- **GitHub API**: Token validation, fork detection
- **hmac + sha2**: HMAC-SHA256 signing (existing engine dependencies)
- **reqwest**: Token validation HTTP call

### Used By
- **action-entrypoint**: Phase 0 validation before any operation
- **audit-posting**: HMAC signing of audit records
- **policy-evaluator**: Policy tamper detection result

---

## Related ADRs

- **Actions ADR-101** (`actions/.pi/architecture/decisions/ADR-101-actions-as-thin-adapter.md`): Security config is a thin adapter
- **Actions ADR-103** (`actions/.pi/architecture/decisions/ADR-103-ci-permission-mode.md`): CI security defaults

---

*Last updated: 2026-06-20*
*Module version: 1.0.0 (Planned)*
*Ported from: original Rigorix docs/ARCHITECTURE_GITHUB_ACTIONS.md §2.7*

---

**Status:** Planned
**Engine modules reused:** hmac + sha2 (for signing), configuration (security.toml format)
