//! Error types for the Enterprise bounded context.

use thiserror::Error;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display_unauthorized() {
        let err = EnterpriseError::Unauthorized {
            detail: "invalid token".to_string(),
        };
        assert_eq!(err.to_string(), "Enterprise API unauthorized: invalid token");
    }

    #[test]
    fn test_error_display_server_error() {
        let err = EnterpriseError::ServerError {
            detail: "HTTP 500".to_string(),
        };
        assert_eq!(err.to_string(), "Enterprise API server error: HTTP 500");
    }

    #[test]
    fn test_error_display_signature_mismatch() {
        let err = EnterpriseError::SignatureMismatch {
            detail: "digest mismatch".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Policy bundle signature mismatch: digest mismatch"
        );
    }

    #[test]
    fn test_error_display_request_failed() {
        let err = EnterpriseError::RequestFailed {
            detail: "connection reset".to_string(),
        };
        assert_eq!(
            err.to_string(),
            "Enterprise API request failed: connection reset"
        );
    }

    #[test]
    fn test_error_display_config_error() {
        let err = EnterpriseError::ConfigError {
            detail: "missing team_id".to_string(),
        };
        assert_eq!(err.to_string(), "Enterprise config error: missing team_id");
    }

    #[test]
    fn test_error_display_cache_error() {
        let err = EnterpriseError::CacheError {
            detail: "lock poisoned".to_string(),
        };
        assert_eq!(err.to_string(), "Enterprise cache error: lock poisoned");
    }
}

/// Errors that can occur during enterprise operations.
#[derive(Debug, Clone, Error)]
pub enum EnterpriseError {
    /// The API returned an unauthorized response (401).
    #[error("Enterprise API unauthorized: {detail}")]
    Unauthorized {
        detail: String,
    },

    /// The server returned a server error (5xx).
    #[error("Enterprise API server error: {detail}")]
    ServerError {
        detail: String,
    },

    /// The bundle signature did not match the computed HMAC.
    #[error("Policy bundle signature mismatch: {detail}")]
    SignatureMismatch {
        detail: String,
    },

    /// A network error occurred during the request.
    #[error("Enterprise API request failed: {detail}")]
    RequestFailed {
        detail: String,
    },

    /// The enterprise configuration is incomplete.
    #[error("Enterprise config error: {detail}")]
    ConfigError {
        detail: String,
    },

    /// Cache error.
    #[error("Enterprise cache error: {detail}")]
    CacheError {
        detail: String,
    },
}
