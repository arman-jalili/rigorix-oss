//! Span privacy utilities for the Rigorix observability system.
//!
//! @canonical .pi/architecture/modules/observability.md#privacy
//!
//! Provides utilities to prevent sensitive data (API keys, tokens, secrets)
//! from appearing in tracing output. All service methods should use
//! `#[tracing::instrument(skip(secret_param))]` to exclude sensitive
//! parameters from span fields.

/// List of field name patterns that are considered sensitive and should
/// be skipped in tracing instrumentation.
pub const SENSITIVE_FIELD_PATTERNS: &[&str] = &[
    "api_key",
    "api_key",
    "token",
    "secret",
    "password",
    "authorization",
    "auth_header",
];

/// Check if a field name matches any sensitive pattern.
pub fn is_sensitive_field(name: &str) -> bool {
    let lower = name.to_lowercase();
    SENSITIVE_FIELD_PATTERNS
        .iter()
        .any(|pattern| lower.contains(pattern))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_sensitive_field_detects_api_key() {
        assert!(is_sensitive_field("api_key"));
        assert!(is_sensitive_field("openai_api_key"));
        assert!(is_sensitive_field("CLAUDE_API_KEY"));
    }

    #[test]
    fn test_is_sensitive_field_detects_token() {
        assert!(is_sensitive_field("auth_token"));
        assert!(is_sensitive_field("access_token"));
    }

    #[test]
    fn test_is_sensitive_field_allows_safe_fields() {
        assert!(!is_sensitive_field("user_id"));
        assert!(!is_sensitive_field("template_name"));
        assert!(!is_sensitive_field("execution_id"));
        assert!(!is_sensitive_field("dag_id"));
    }
}
