//! HostError — adapter-neutral, taxonomy-typed error at the shared dispatch
//! boundary (#888 OSS-C2).
//!
//! [`crate::host::AppState::handle_tool_call`] returns this instead of an
//! opaque `serde_json::Value` error envelope. The **MCP adapter** formats it
//! back to the current text (`message`), preserving behavior; the **native
//! API server** maps it to the frozen `errors.json` taxonomy via
//! [`HostError::to_jsonrpc_error`].
//!
//! This keeps the structured context the engine already produced
//! (`rule_id` + `step` for `denied_by_sequence`, `identity_required`, …)
//! instead of flattening it into a string and re-parsing it downstream.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::audit_tools::domain::error::{AuditError, AuditHandlerError};
use crate::auth::domain::error::AuthError;
use crate::execution_tools::domain::error::{EngineFacadeError, HandlerError};
use crate::template_tools::domain::error::{HandlerError as TemplateHandlerError, TemplateError};

/// The stable `data.type` identifiers from `schemas/api/errors.json` (v1).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorType {
    /// JSON parse error (-32700).
    ParseError,
    /// Invalid JSON-RPC request (-32600).
    InvalidRequest,
    /// Unknown method (-32601).
    MethodNotFound,
    /// Invalid params (-32602).
    InvalidParams,
    /// Internal error (-32603).
    InternalError,
    /// No attested session (-32001).
    NotAuthenticated,
    /// Attested session lacks scope (-32002).
    InsufficientScope,
    /// A `deny` sequence rule refused the step (-32010).
    DeniedBySequence,
    /// Enforcement/policy violation (-32010).
    PolicyViolation,
    /// Step pauses for approval (-32011).
    ApprovalRequired,
    /// An attested identity is required (-32012).
    IdentityRequired,
    /// Budget exhausted (-32020).
    BudgetExhausted,
    /// Operation timed out (-32021).
    Timeout,
    /// Upstream/engine API error (-32030).
    ApiError,
    /// Feature not enabled (-32031).
    NotEnabled,
}

impl ErrorType {
    /// Stable machine identifier (`errors.json` `data.type`).
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ParseError => "parse_error",
            Self::InvalidRequest => "invalid_request",
            Self::MethodNotFound => "method_not_found",
            Self::InvalidParams => "invalid_params",
            Self::InternalError => "internal_error",
            Self::NotAuthenticated => "not_authenticated",
            Self::InsufficientScope => "insufficient_scope",
            Self::DeniedBySequence => "denied_by_sequence",
            Self::PolicyViolation => "policy_violation",
            Self::ApprovalRequired => "approval_required",
            Self::IdentityRequired => "identity_required",
            Self::BudgetExhausted => "budget_exhausted",
            Self::Timeout => "timeout",
            Self::ApiError => "api_error",
            Self::NotEnabled => "not_enabled",
        }
    }

    /// JSON-RPC error code (`errors.json` `x-error-codes`).
    pub const fn code(self) -> i64 {
        match self {
            Self::ParseError => -32700,
            Self::InvalidRequest => -32600,
            Self::MethodNotFound => -32601,
            Self::InvalidParams => -32602,
            Self::InternalError => -32603,
            Self::NotAuthenticated => -32001,
            Self::InsufficientScope => -32002,
            Self::DeniedBySequence => -32010,
            Self::PolicyViolation => -32010,
            Self::ApprovalRequired => -32011,
            Self::IdentityRequired => -32012,
            Self::BudgetExhausted => -32020,
            Self::Timeout => -32021,
            Self::ApiError => -32030,
            Self::NotEnabled => -32031,
        }
    }

    /// Whether the failed operation is safe to retry.
    pub const fn retryable(self) -> bool {
        matches!(self, Self::Timeout | Self::ApiError)
    }
}

/// Adapter-neutral dispatch error carrying the `errors.json` taxonomy plus any
/// structured context the engine produced.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HostError {
    /// Stable taxonomy type (`data.type`).
    pub error_type: ErrorType,
    /// Human-readable message (the MCP adapter surfaces this verbatim).
    pub message: String,
    /// Sequence rule id for `denied_by_sequence` (AC5 / ADR-0001 D6).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub rule_id: Option<String>,
    /// Refused step for `denied_by_sequence` / `identity_required`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<String>,
    /// Extra structured detail (never parameter values).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<Value>,
    /// Whether the caller may retry.
    pub retryable: bool,
}

impl std::fmt::Display for HostError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for HostError {}

impl HostError {
    /// Build an error of `error_type` with a message.
    pub fn new(error_type: ErrorType, message: impl Into<String>) -> Self {
        Self {
            error_type,
            message: message.into(),
            rule_id: None,
            step: None,
            details: None,
            retryable: error_type.retryable(),
        }
    }

    /// Invalid params (client input could not be parsed / validated).
    pub fn invalid_params(message: impl Into<String>) -> Self {
        Self::new(ErrorType::InvalidParams, message)
    }

    /// Unknown catalog method / MCP tool.
    pub fn method_not_found(name: &str) -> Self {
        Self::new(ErrorType::MethodNotFound, format!("Unknown tool: {name}"))
    }

    /// Internal error.
    pub fn internal(message: impl Into<String>) -> Self {
        Self::new(ErrorType::InternalError, message)
    }

    /// No attested session.
    pub fn not_authenticated(message: impl Into<String>) -> Self {
        Self::new(ErrorType::NotAuthenticated, message)
    }

    /// Attested session lacks the required scope.
    pub fn insufficient_scope(message: impl Into<String>) -> Self {
        Self::new(ErrorType::InsufficientScope, message)
    }

    /// Feature not enabled in this deployment.
    pub fn not_enabled(message: impl Into<String>) -> Self {
        Self::new(ErrorType::NotEnabled, message)
    }

    /// Add structured sequence context (`denied_by_sequence`).
    pub fn with_sequence(mut self, rule_id: impl Into<String>, step: impl Into<String>) -> Self {
        self.rule_id = Some(rule_id.into());
        self.step = Some(step.into());
        self
    }

    /// Attach extra structured detail.
    pub fn with_details(mut self, details: Value) -> Self {
        self.details = Some(details);
        self
    }

    /// Map a structured [`EngineFacadeError`] onto the taxonomy.
    ///
    /// `denied_by_sequence` carries `rule_id` + `step`; `identity_required`
    /// carries `step`; `RequirementUnmet` carries the requirement id / step /
    /// unmet pointer names as `details` (never parameter values).
    pub fn from_engine_facade(error: &EngineFacadeError) -> Self {
        match error {
            EngineFacadeError::SequencePolicyDenied { rule_id, step } => {
                Self::new(ErrorType::DeniedBySequence, error.to_string())
                    .with_sequence(rule_id.clone(), step.clone())
            }
            EngineFacadeError::IdentityRequired { step, status } => {
                Self::new(ErrorType::IdentityRequired, error.to_string())
                    .with_details(serde_json::json!({ "step": step, "status": status }))
            }
            EngineFacadeError::RequirementUnmet {
                requirement_id,
                step,
                unmet,
            } => Self::new(ErrorType::PolicyViolation, error.to_string()).with_details(
                serde_json::json!({
                    "requirement_id": requirement_id,
                    "step": step,
                    "unmet": unmet,
                }),
            ),
            EngineFacadeError::BudgetExceeded { .. } => {
                Self::new(ErrorType::BudgetExhausted, error.to_string())
            }
            EngineFacadeError::Timeout { .. } => Self::new(ErrorType::Timeout, error.to_string()),
            EngineFacadeError::EnforcementBlocked(_) => {
                Self::new(ErrorType::PolicyViolation, error.to_string())
            }
            EngineFacadeError::PlanValidationFailed(_)
            | EngineFacadeError::InvalidPlan(_)
            | EngineFacadeError::ExecutionNotFound(_) => {
                Self::new(ErrorType::InvalidParams, error.to_string())
            }
            EngineFacadeError::EngineError(_) => Self::new(ErrorType::ApiError, error.to_string()),
            EngineFacadeError::EngineNotAvailable(_) | EngineFacadeError::Internal(_) => {
                Self::new(ErrorType::InternalError, error.to_string())
            }
        }
    }

    /// Map a tool-handler error (wrapping an engine error when present).
    pub fn from_handler(error: &HandlerError) -> Self {
        match error {
            HandlerError::EngineError(inner) => Self::from_engine_facade(inner),
            HandlerError::InvalidArguments(_) | HandlerError::ValidationErrors(_) => {
                Self::new(ErrorType::InvalidParams, error.to_string())
            }
            HandlerError::Timeout => Self::new(ErrorType::Timeout, error.to_string()),
            HandlerError::Internal(_) => Self::new(ErrorType::InternalError, error.to_string()),
        }
    }

    /// The JSON-RPC 2.0 error object (per `schemas/api/errors.json`).
    pub fn to_jsonrpc_error(&self) -> Value {
        let mut data = serde_json::Map::new();
        data.insert(
            "type".to_string(),
            Value::String(self.error_type.as_str().to_string()),
        );
        if let Some(rule_id) = &self.rule_id {
            data.insert("rule_id".to_string(), Value::String(rule_id.clone()));
        }
        if let Some(step) = &self.step {
            data.insert("step".to_string(), Value::String(step.clone()));
        }
        if let Some(details) = &self.details {
            data.insert("details".to_string(), details.clone());
        }
        data.insert("retryable".to_string(), Value::Bool(self.retryable));
        serde_json::json!({
            "code": self.error_type.code(),
            "message": self.message,
            "data": Value::Object(data),
        })
    }
}

impl From<EngineFacadeError> for HostError {
    fn from(error: EngineFacadeError) -> Self {
        Self::from_engine_facade(&error)
    }
}

impl From<HandlerError> for HostError {
    fn from(error: HandlerError) -> Self {
        Self::from_handler(&error)
    }
}

impl From<AuditHandlerError> for HostError {
    fn from(error: AuditHandlerError) -> Self {
        let message = error.to_string();
        match error {
            AuditHandlerError::InvalidArguments(_) => Self::new(ErrorType::InvalidParams, message),
            AuditHandlerError::Timeout => Self::new(ErrorType::Timeout, message),
            AuditHandlerError::Internal(_) => Self::new(ErrorType::InternalError, message),
            AuditHandlerError::AuditError(inner) => match inner {
                AuditError::NotFound(_) | AuditError::InvalidFilter(_) => {
                    Self::new(ErrorType::InvalidParams, message)
                }
                AuditError::Timeout { .. } => Self::new(ErrorType::Timeout, message),
                AuditError::EngineError(_)
                | AuditError::EngineNotAvailable(_)
                | AuditError::Internal(_) => Self::new(ErrorType::InternalError, message),
            },
        }
    }
}

impl From<TemplateHandlerError> for HostError {
    fn from(error: TemplateHandlerError) -> Self {
        let message = error.to_string();
        match error {
            TemplateHandlerError::InvalidArguments(_) => {
                Self::new(ErrorType::InvalidParams, message)
            }
            TemplateHandlerError::Timeout { .. } => Self::new(ErrorType::Timeout, message),
            TemplateHandlerError::Internal(_) => Self::new(ErrorType::InternalError, message),
            TemplateHandlerError::TemplateError(inner) => match inner {
                TemplateError::NotFound(_)
                | TemplateError::InvalidName(_)
                | TemplateError::AlreadyExists(_)
                | TemplateError::ValidationError(_) => Self::new(ErrorType::InvalidParams, message),
                _ => Self::new(ErrorType::InternalError, message),
            },
        }
    }
}

impl From<AuthError> for HostError {
    fn from(error: AuthError) -> Self {
        let message = error.to_string();
        match error {
            AuthError::NotAuthenticated | AuthError::Expired | AuthError::AccessDenied(_) => {
                Self::new(ErrorType::NotAuthenticated, message)
            }
            AuthError::Transport(_) | AuthError::Discovery { .. } => {
                Self::new(ErrorType::ApiError, message)
            }
            AuthError::Configuration(_)
            | AuthError::DeviceAuthorizationRejected(_)
            | AuthError::InvalidTokenResponse(_)
            | AuthError::RefreshFailed(_)
            | AuthError::Keychain(_)
            | AuthError::Attestation(_)
            | AuthError::Internal(_) => Self::new(ErrorType::InternalError, message),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn denied_by_sequence_carries_rule_id_and_step() {
        let error = EngineFacadeError::SequencePolicyDenied {
            rule_id: "no-repeat-beneficiary-payout".to_string(),
            step: "pay_b".to_string(),
        };
        let host = HostError::from_engine_facade(&error);
        assert_eq!(host.error_type, ErrorType::DeniedBySequence);
        assert_eq!(
            host.rule_id.as_deref(),
            Some("no-repeat-beneficiary-payout")
        );
        assert_eq!(host.step.as_deref(), Some("pay_b"));

        let rpc = host.to_jsonrpc_error();
        assert_eq!(rpc["code"], -32010);
        assert_eq!(rpc["data"]["type"], "denied_by_sequence");
        assert_eq!(rpc["data"]["rule_id"], "no-repeat-beneficiary-payout");
        assert_eq!(rpc["data"]["step"], "pay_b");
    }

    #[test]
    fn identity_required_maps_to_taxonomy() {
        let error = EngineFacadeError::IdentityRequired {
            step: "pay".to_string(),
            status: "unauthenticated".to_string(),
        };
        let host = HostError::from_engine_facade(&error);
        assert_eq!(host.error_type, ErrorType::IdentityRequired);
        let rpc = host.to_jsonrpc_error();
        assert_eq!(rpc["code"], -32012);
        assert_eq!(rpc["data"]["type"], "identity_required");
        assert_eq!(rpc["data"]["details"]["step"], "pay");
    }

    #[test]
    fn requirement_unmet_is_policy_violation_with_redacted_details() {
        let error = EngineFacadeError::RequirementUnmet {
            requirement_id: "payout-guard".to_string(),
            step: "pay".to_string(),
            unmet: vec!["/effect_key".to_string()],
        };
        let host = HostError::from_engine_facade(&error);
        assert_eq!(host.error_type, ErrorType::PolicyViolation);
        let rpc = host.to_jsonrpc_error();
        assert_eq!(rpc["code"], -32010);
        assert_eq!(rpc["data"]["details"]["requirement_id"], "payout-guard");
    }

    #[test]
    fn distinct_engine_variants_map_to_distinct_types() {
        assert_eq!(
            HostError::from_engine_facade(&EngineFacadeError::BudgetExceeded {
                tool_calls_remaining: 0,
                tokens_remaining: 0
            })
            .error_type,
            ErrorType::BudgetExhausted
        );
        assert_eq!(
            HostError::from_engine_facade(&EngineFacadeError::Timeout {
                operation: "run".to_string(),
                duration_secs: 1
            })
            .error_type,
            ErrorType::Timeout
        );
        assert_eq!(
            HostError::from_engine_facade(&EngineFacadeError::Internal("x".to_string())).error_type,
            ErrorType::InternalError
        );
    }
}
