//! Contract Freeze verification tests for the Auth module.
//!
//! These tests verify that all public interfaces, contracts, and schemas are
//! properly defined and compilable. They do NOT test implementation logic —
//! they test contract existence and shape.
//!
//! Once the contracts are frozen, implementation issues can depend on them.

#![allow(unused_imports)]

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{TimeZone, Utc};

use rigorix_mcp::auth::application::dto::{
    LOGGED_OUT_STATUS, LoginInput, LoginOutput, LogoutInput, LogoutOutput, PollInput, PollOutput,
    RefreshInput, RefreshOutput, StatusInput, StatusOutput,
};
use rigorix_mcp::auth::application::factory::AuthServiceFactory;
use rigorix_mcp::auth::application::service::AuthService;
use rigorix_mcp::auth::domain::config::{DEFAULT_ACCESS_TOKEN_TTL_SECS, IdpConfig};
use rigorix_mcp::auth::domain::error::{AuthError, DeviceFlowPollError, SseAuthError};
use rigorix_mcp::auth::domain::event::AuthEvent;
use rigorix_mcp::auth::domain::flow::{DeviceFlowState, DeviceFlowStatus};
use rigorix_mcp::auth::domain::status::TokenStatus;
use rigorix_mcp::auth::domain::value::{ClaimSummary, Secret};
use rigorix_mcp::auth::infrastructure::keychain_store::{
    KeychainStore, REFRESH_TOKEN_ACCOUNT, RIGORIX_KEYCHAIN_SERVICE,
};
use rigorix_mcp::auth::infrastructure::token_provider::TokenProvider;
use rigorix_mcp::auth::infrastructure::{
    DeviceAuthorization, IdpClient, IdpMetadata, TokenPoll, TokenResponse,
};
use rigorix_mcp::auth::interfaces::mcp::{
    AUTH_TOOL_NAMES, AuthToolHandler, RIGORIX_AUTH_LOGIN, RIGORIX_AUTH_LOGIN_INPUT_SCHEMA,
    RIGORIX_AUTH_LOGOUT, RIGORIX_AUTH_LOGOUT_INPUT_SCHEMA, RIGORIX_AUTH_STATUS,
    RIGORIX_AUTH_STATUS_INPUT_SCHEMA, auth_tool_descriptors, rigorix_auth_login_tool_descriptor,
    rigorix_auth_logout_tool_descriptor, rigorix_auth_status_tool_descriptor,
};
use rigorix_mcp::auth::interfaces::sse_auth::{SseAuthDecision, SseAuthGate, SseAuthMode};

// ---------------------------------------------------------------------------
// AuthService (Application) — contract existence
// ---------------------------------------------------------------------------

/// The AuthService trait must exist and be trait-object safe (Send + Sync),
/// so implementations can be shared as `Arc<dyn AuthService>`.
#[test]
fn test_auth_service_trait_is_defined() {
    fn _assert_object_safe<T: Send + Sync + ?Sized>() {}
    _assert_object_safe::<dyn AuthService>();
}

/// A minimal stateless implementation proves the trait is implementable with
/// the frozen method set.
struct StubAuthService;

#[async_trait]
impl AuthService for StubAuthService {
    async fn login(&self, _input: &LoginInput) -> Result<LoginOutput, AuthError> {
        Ok(LoginOutput {
            status: DeviceFlowStatus::Pending,
            verification_uri: "https://idp.example.com/device".into(),
            user_code: "ABCD-EFGH".into(),
            expires_in: 600,
        })
    }

    async fn poll(&self, _input: &PollInput) -> Result<PollOutput, AuthError> {
        Ok(PollOutput {
            status: DeviceFlowStatus::Pending,
            retry_after_secs: Some(5),
            reason: None,
            claim_summary: None,
        })
    }

    async fn status(&self, _input: &StatusInput) -> Result<StatusOutput, AuthError> {
        Ok(StatusOutput {
            status: TokenStatus::Unauthenticated,
            claim_summary: None,
            source: rigorix_engine::identity::IdentitySource::Unverified,
        })
    }

    async fn refresh(&self, _input: &RefreshInput) -> Result<RefreshOutput, AuthError> {
        Err(AuthError::NotAuthenticated)
    }

    async fn logout(&self, _input: &LogoutInput) -> Result<LogoutOutput, AuthError> {
        Ok(LogoutOutput::logged_out())
    }

    async fn attest(&self) -> Result<rigorix_engine::identity::IdentityClaim, AuthError> {
        Err(AuthError::NotAuthenticated)
    }
}

#[tokio::test]
async fn test_auth_service_method_set_invokable() {
    let svc = StubAuthService;
    let login = svc.login(&LoginInput::default()).await.unwrap();
    assert_eq!(login.status, DeviceFlowStatus::Pending);
    assert!(!login.verification_uri.is_empty());
    assert_eq!(login.user_code, "ABCD-EFGH");
    let poll = svc.poll(&PollInput::default()).await.unwrap();
    assert_eq!(poll.status, DeviceFlowStatus::Pending);
    let status = svc.status(&StatusInput::default()).await.unwrap();
    assert_eq!(status.status, TokenStatus::Unauthenticated);
    let logout = svc.logout(&LogoutInput::default()).await.unwrap();
    assert_eq!(logout.status, LOGGED_OUT_STATUS);
}

// ---------------------------------------------------------------------------
// IdpConfig (domain) — value object contract
// ---------------------------------------------------------------------------

#[test]
fn test_idp_config_validation() {
    let cfg = IdpConfig::new(
        "https://idp.example.com/realms/rigorix".into(),
        "rigorix-cli".into(),
        Some("client-secret".into()),
        None,
    )
    .unwrap();
    assert_eq!(cfg.issuer(), "https://idp.example.com/realms/rigorix");
    assert_eq!(cfg.client_id(), "rigorix-cli");
    assert_eq!(cfg.access_token_ttl_secs(), DEFAULT_ACCESS_TOKEN_TTL_SECS);
    assert!(cfg.client_secret().is_some());
    // Secret material never renders.
    assert!(!format!("{cfg:?}").contains("client-secret"));
}

#[test]
fn test_idp_config_rejects_plain_http() {
    let err = IdpConfig::new(
        "http://idp.example.com".into(),
        "rigorix-cli".into(),
        None,
        None,
    )
    .unwrap_err();
    assert!(matches!(err, AuthError::Configuration(_)));
}

// ---------------------------------------------------------------------------
// Secret + ClaimSummary (domain value objects)
// ---------------------------------------------------------------------------

#[test]
fn test_secret_is_redacted_everywhere() {
    let secret = Secret::new("crown-jewel-refresh-token");
    assert!(!format!("{secret:?}").contains("refresh-token"));
    assert_eq!(secret.expose(), &"crown-jewel-refresh-token");
    let json = serde_json::to_string(&secret).unwrap();
    assert!(!json.contains("refresh-token"));
    assert!(json.contains("REDACTED"));
}

#[test]
fn test_claim_summary_redacted_fields() {
    let summary = ClaimSummary {
        subject: "user@org".into(),
        issuer: "https://idp.example.com".into(),
        authority: None,
        expires_at: Some(Utc.timestamp_opt(2_000_000_000, 0).unwrap()),
    };
    let json = serde_json::to_string(&summary).unwrap();
    assert!(json.contains("user@org"));
    // No token material field exists on the summary surface.
    assert!(!json.contains("token"));
}

// ---------------------------------------------------------------------------
// Status / flow enums (domain)
// ---------------------------------------------------------------------------

#[test]
fn test_token_status_serde_literals() {
    assert_eq!(TokenStatus::Authenticated.to_string(), "authenticated");
    assert_eq!(
        serde_json::to_string(&TokenStatus::Expired).unwrap(),
        "\"expired\""
    );
    assert!(TokenStatus::Authenticated.is_authenticated());
    assert!(!TokenStatus::Unauthenticated.is_authenticated());
}

#[test]
fn test_device_flow_status_terminal() {
    assert!(!DeviceFlowStatus::Pending.is_terminal());
    assert!(DeviceFlowStatus::Authorized.is_terminal());
    assert!(DeviceFlowStatus::Denied.is_terminal());
    assert!(DeviceFlowStatus::Expired.is_terminal());
}

#[test]
fn test_device_flow_state_fields() {
    let state = DeviceFlowState {
        session_id: "sess-1".into(),
        device_code: Secret::new("device-code".into()),
        verification_uri: "https://idp.example.com/device".into(),
        user_code: "ABCD-EFGH".into(),
        expires_in: 600,
        expires_at: Utc::now() + chrono::Duration::minutes(10),
        interval_secs: 5,
        status: DeviceFlowStatus::Pending,
    };
    assert!(state.is_pending());
    assert!(!state.is_ended());
    // Device code never renders.
    assert!(!format!("{state:?}").contains("device-code"));
}

#[test]
fn test_device_flow_poll_error_rfc_codes() {
    assert_eq!(
        DeviceFlowPollError::from_rfc_code("authorization_pending"),
        Some(DeviceFlowPollError::AuthorizationPending)
    );
    assert_eq!(
        DeviceFlowPollError::from_rfc_code("slow_down"),
        Some(DeviceFlowPollError::SlowDown)
    );
    assert_eq!(
        DeviceFlowPollError::from_rfc_code("access_denied"),
        Some(DeviceFlowPollError::AccessDenied)
    );
    assert_eq!(
        DeviceFlowPollError::from_rfc_code("expired_token"),
        Some(DeviceFlowPollError::ExpiredToken)
    );
    assert_eq!(DeviceFlowPollError::from_rfc_code("invalid_grant"), None);
}

// ---------------------------------------------------------------------------
// AuthError (domain) — error contract
// ---------------------------------------------------------------------------

#[test]
fn test_auth_error_variants_construct() {
    let config_err = AuthError::Configuration("missing issuer".into());
    assert!(matches!(config_err, AuthError::Configuration(_)));

    let transport = AuthError::Transport("idp down".into());
    assert!(transport.is_retriable());

    let discovery = AuthError::Discovery {
        issuer: "https://idp.example.com".into(),
        reason: "404".into(),
    };
    assert!(discovery.is_retriable());

    let denied = AuthError::AccessDenied("user declined".into());
    assert!(!denied.is_retriable());
    assert!(matches!(AuthError::Expired, AuthError::Expired));
    assert!(matches!(
        AuthError::Keychain("unavailable".into()),
        AuthError::Keychain(_)
    ));
}

#[test]
fn test_sse_auth_error_variants_construct() {
    let err = SseAuthError::NotConfigured("idp gate without IdpClient".into());
    assert!(format!("{err}").contains("not configured"));
}

// ---------------------------------------------------------------------------
// AuthEvent (domain) — payload schemas per auth.md
// ---------------------------------------------------------------------------

#[test]
fn test_auth_event_payloads_match_module_docs() {
    let now = Utc::now();
    let started = AuthEvent::AuthLoginStarted {
        session_id: "s1".into(),
        verification_uri: "https://idp.example.com/device".into(),
        user_code: "ABCD-EFGH".into(),
        timestamp: now,
    };
    let json = serde_json::to_value(&started).unwrap();
    assert_eq!(json["type"], "auth_login_started");
    assert_eq!(json["session_id"], "s1");
    assert_eq!(json["verification_uri"], "https://idp.example.com/device");

    let succeeded = AuthEvent::AuthLoginSucceeded {
        session_id: "s1".into(),
        subject: "user@org".into(),
        issuer: "https://idp.example.com".into(),
        token_ttl_secs: 900,
        timestamp: now,
    };
    let json = serde_json::to_value(&succeeded).unwrap();
    assert_eq!(json["type"], "auth_login_succeeded");
    assert_eq!(json["subject"], "user@org");

    let failed = AuthEvent::AuthLoginFailed {
        session_id: "s1".into(),
        error_type: "access_denied".into(),
        reason: "user declined".into(),
        timestamp: now,
    };
    let json = serde_json::to_value(&failed).unwrap();
    assert_eq!(json["error_type"], "access_denied");

    let checked = AuthEvent::AuthStatusChecked {
        session_id: "s1".into(),
        status: TokenStatus::Authenticated,
        claim_summary: None,
        timestamp: now,
    };
    let json = serde_json::to_value(&checked).unwrap();
    assert_eq!(json["type"], "auth_status_checked");
    assert_eq!(json["status"], "authenticated");

    let logged_out = AuthEvent::AuthLoggedOut {
        session_id: "s1".into(),
        revoked: true,
        timestamp: now,
    };
    let json = serde_json::to_value(&logged_out).unwrap();
    assert_eq!(json["type"], "auth_logged_out");
    assert_eq!(json["revoked"], true);
}

// ---------------------------------------------------------------------------
// IdpClient (Infrastructure) — port interface existence
// ---------------------------------------------------------------------------

/// The IdpClient trait must exist and be trait-object safe.
#[test]
fn test_idp_client_trait_is_defined() {
    fn _assert_object_safe<T: Send + Sync + ?Sized>() {}
    _assert_object_safe::<dyn IdpClient>();
}

#[test]
fn test_idp_contract_types_are_constructible() {
    let meta = IdpMetadata {
        issuer: "https://idp.example.com".into(),
        device_authorization_endpoint: "https://idp.example.com/device_authorization".into(),
        token_endpoint: "https://idp.example.com/token".into(),
        revocation_endpoint: None,
        jwks_uri: Some("https://idp.example.com/jwks".into()),
    };
    assert_eq!(meta.issuer, "https://idp.example.com");

    let authz = DeviceAuthorization {
        device_code: Secret::new("device-code".into()),
        user_code: "ABCD-EFGH".into(),
        verification_uri: "https://idp.example.com/device".into(),
        expires_in: 600,
        interval_secs: 5,
        issued_at: Utc::now(),
    };
    assert_eq!(authz.user_code, "ABCD-EFGH");
    assert!(!format!("{authz:?}").contains("device-code"));

    let tokens = TokenResponse {
        access_token: Secret::new("access-token".into()),
        refresh_token: Some(Secret::new("refresh-token".into())),
        expires_in: 900,
        token_type: "Bearer".into(),
        scope: Some("openid".into()),
    };
    assert_eq!(tokens.token_type, "Bearer");
    assert!(!format!("{tokens:?}").contains("refresh-token"));

    let _poll_pending = TokenPoll::Pending {
        retry_after_secs: Some(5),
    };
    let _poll_denied = TokenPoll::AccessDenied {
        reason: "user declined".into(),
    };
    let _poll_expired = TokenPoll::Expired;
}

// ---------------------------------------------------------------------------
// KeychainStore + TokenProvider (Infrastructure) — port existence
// ---------------------------------------------------------------------------

#[test]
fn test_keychain_constants_defined() {
    assert_eq!(RIGORIX_KEYCHAIN_SERVICE, "rigorix");
    assert_eq!(REFRESH_TOKEN_ACCOUNT, "refresh_token");
}

#[test]
fn test_keychain_store_trait_is_defined() {
    fn _assert_object_safe<T: Send + Sync + ?Sized>() {}
    _assert_object_safe::<dyn KeychainStore>();
}

#[test]
fn test_token_provider_trait_is_defined() {
    fn _assert_object_safe<T: Send + Sync + ?Sized>() {}
    _assert_object_safe::<dyn TokenProvider>();
}

// ---------------------------------------------------------------------------
// AuthServiceFactory (Application) — factory interface existence
// ---------------------------------------------------------------------------

#[test]
fn test_auth_service_factory_trait_is_defined() {
    fn _assert_object_safe<T: Send + Sync + ?Sized>() {}
    _assert_object_safe::<dyn AuthServiceFactory>();
}

// ---------------------------------------------------------------------------
// MCP tool contracts (Interfaces) — names + schemas per auth.md
// ---------------------------------------------------------------------------

#[test]
fn test_auth_tool_names_match_architecture() {
    assert_eq!(
        AUTH_TOOL_NAMES,
        &[
            "rigorix_auth_login",
            "rigorix_auth_status",
            "rigorix_auth_logout"
        ]
    );
    assert_eq!(RIGORIX_AUTH_LOGIN, "rigorix_auth_login");
    assert_eq!(RIGORIX_AUTH_STATUS, "rigorix_auth_status");
    assert_eq!(RIGORIX_AUTH_LOGOUT, "rigorix_auth_logout");
}

#[test]
fn test_auth_input_schemas_are_valid_json_schema() {
    for schema in [
        RIGORIX_AUTH_LOGIN_INPUT_SCHEMA,
        RIGORIX_AUTH_STATUS_INPUT_SCHEMA,
        RIGORIX_AUTH_LOGOUT_INPUT_SCHEMA,
    ] {
        let value: serde_json::Value = serde_json::from_str(schema).expect("valid JSON");
        assert_eq!(value["type"], "object");
    }
}

#[test]
fn test_auth_tool_descriptors_are_complete() {
    let descriptors = auth_tool_descriptors();
    assert_eq!(descriptors.len(), 3);
    for d in &descriptors {
        assert!(d["name"].is_string());
        assert!(d["description"].is_string());
        assert!(d["inputSchema"].is_object());
    }
    // Individual descriptor helpers exist.
    assert_eq!(
        rigorix_auth_login_tool_descriptor()["name"],
        "rigorix_auth_login"
    );
    assert_eq!(
        rigorix_auth_status_tool_descriptor()["name"],
        "rigorix_auth_status"
    );
    assert_eq!(
        rigorix_auth_logout_tool_descriptor()["name"],
        "rigorix_auth_logout"
    );
}

#[test]
fn test_auth_tool_handler_trait_is_defined() {
    fn _assert_object_safe<T: Send + Sync + ?Sized>() {}
    _assert_object_safe::<dyn AuthToolHandler>();
}

// ---------------------------------------------------------------------------
// SSE auth (Interfaces) — gate contract existence
// ---------------------------------------------------------------------------

#[test]
fn test_sse_auth_mode_serde_matches_config_schema() {
    assert_eq!(
        serde_json::to_string(&SseAuthMode::None).unwrap(),
        "\"none\""
    );
    assert_eq!(serde_json::to_string(&SseAuthMode::Idp).unwrap(), "\"idp\"");
    assert_eq!(
        serde_json::to_string(&SseAuthMode::ApiKey).unwrap(),
        "\"api_key\""
    );
    assert!(!SseAuthMode::None.is_enforced());
    assert!(SseAuthMode::Idp.is_enforced());
    assert!(SseAuthMode::ApiKey.is_enforced());
}

#[test]
fn test_sse_auth_gate_trait_is_defined() {
    fn _assert_object_safe<T: Send + Sync + ?Sized>() {}
    _assert_object_safe::<dyn SseAuthGate>();
}

#[test]
fn test_sse_auth_decision_shape() {
    match SseAuthDecision::Allow {
        SseAuthDecision::Allow => {}
        SseAuthDecision::Deny { .. } => panic!("wrong variant"),
    }
    let denied = SseAuthDecision::Deny {
        reason: "missing bearer token".into(),
    };
    match denied {
        SseAuthDecision::Allow => panic!("wrong variant"),
        SseAuthDecision::Deny { reason } => assert_eq!(reason, "missing bearer token"),
    }
}

// ---------------------------------------------------------------------------
// Module structure accessibility (mirrors rigorix_mcp::auth::* layering)
// ---------------------------------------------------------------------------

#[test]
fn test_module_structure_accessible() {
    // Layer modules are reachable from the crate root.
    let _ = rigorix_mcp::auth::application::dto::LoginInput::default();
    let _ = rigorix_mcp::auth::domain::TokenStatus::Unauthenticated;
    let _ = rigorix_mcp::auth::infrastructure::IdpMetadata {
        issuer: String::new(),
        device_authorization_endpoint: String::new(),
        token_endpoint: String::new(),
        revocation_endpoint: None,
        jwks_uri: None,
    };
    let _ = rigorix_mcp::auth::interfaces::sse_auth::SseAuthMode::None;
}

#[test]
fn test_logout_output_canonical_literal() {
    assert_eq!(LOGGED_OUT_STATUS, "logged_out");
    let out = LogoutOutput::logged_out();
    assert_eq!(out.status, "logged_out");
}
