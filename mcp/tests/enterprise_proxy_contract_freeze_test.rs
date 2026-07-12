//! Contract Freeze verification tests for the Enterprise Proxy module.
//!
//! These tests verify that all public interfaces, contracts, and schemas
//! are properly defined and compilable. They do NOT test implementation
//! logic — they test contract existence and shape.
//!
//! Once the contracts are frozen, implementation issues can depend on them.

#![allow(unused_imports)]

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};

use rigorix_mcp::enterprise_proxy::application::dto::{
    HandleToolCallInput, HandleToolCallOutput, HealthCheckOutput, InitializeOutput,
    ListAvailableToolsOutput, ProxyConfigSummary, SchemaCacheStatus, SchemaCacheUpdateOutput,
    ToolSchemaDto,
};
use rigorix_mcp::enterprise_proxy::application::service::{
    EnterpriseToolRouter, ProxyInitializationService, SchemaCacheService,
};
use rigorix_mcp::enterprise_proxy::domain::entity::{
    EnterpriseProxy, SchemaCache, SharedEnterpriseProxy,
};
use rigorix_mcp::enterprise_proxy::domain::error::{
    HandlerError, ProxyError, ToolCallResult, ToolContentItem,
};
use rigorix_mcp::enterprise_proxy::domain::event::EnterpriseProxyEvent;
use rigorix_mcp::enterprise_proxy::domain::value::{
    EnterpriseMetadata, HealthStatus, JsonRpcError, JsonRpcRequest, JsonRpcResponse, ProxyConfig,
    Secret, ToolSchema,
};
use rigorix_mcp::enterprise_proxy::infrastructure::repository::SchemaCacheRepository;
use rigorix_mcp::enterprise_proxy::interfaces::mcp::{
    ENTERPRISE_TOOL_NAMES, ENTERPRISE_TOOL_PREFIX,
};

// -----------------------------------------------------------------------
// Contract: EnterpriseProxy trait is defined and implementable
// -----------------------------------------------------------------------

struct EnterpriseProxyContractValidator;

#[async_trait]
impl EnterpriseProxy for EnterpriseProxyContractValidator {
    fn is_enabled(&self) -> bool {
        false
    }

    fn available_tools(&self) -> Vec<ToolSchema> {
        vec![]
    }

    fn metadata(&self) -> Option<EnterpriseMetadata> {
        None
    }

    async fn handle(
        &self,
        _method: &str,
        _params: serde_json::Value,
    ) -> Result<serde_json::Value, ProxyError> {
        Err(ProxyError::NotEnabled)
    }

    async fn initialize(&self) -> Result<(), ProxyError> {
        Err(ProxyError::NotEnabled)
    }

    async fn health_check(&self) -> Result<HealthStatus, ProxyError> {
        Err(ProxyError::NotEnabled)
    }
}

#[test]
fn test_enterprise_proxy_trait_is_defined() {
    let proxy = EnterpriseProxyContractValidator;
    let _instance: SharedEnterpriseProxy = Arc::new(proxy);
    // Contract satisfied: EnterpriseProxy trait is implementable
}

#[test]
fn test_enterprise_proxy_trait_methods_exist() {
    let proxy = EnterpriseProxyContractValidator;
    assert!(!proxy.is_enabled());
    assert!(proxy.available_tools().is_empty());
    assert!(proxy.metadata().is_none());
}

#[test]
fn test_shared_enterprise_proxy_type_exists() {
    let proxy: SharedEnterpriseProxy = Arc::new(EnterpriseProxyContractValidator);
    assert!(!proxy.is_enabled());
}

// -----------------------------------------------------------------------
// Contract: SchemaCache struct is defined and constructable
// -----------------------------------------------------------------------

#[test]
fn test_schema_cache_new() {
    let cache = SchemaCache::new();
    assert!(!cache.is_populated());
    assert_eq!(cache.tool_count(), 0);
    assert!(cache.tools().is_empty());
    assert!(cache.metadata().is_none());
}

#[test]
fn test_schema_cache_default() {
    let cache = SchemaCache::default();
    assert!(!cache.is_populated());
    assert_eq!(cache.tool_count(), 0);
}

#[test]
fn test_schema_cache_update_and_query() {
    let mut cache = SchemaCache::new();
    let metadata = EnterpriseMetadata {
        version: "1.0.0".into(),
        tools: vec![ToolSchema {
            name: "rigorix_enterprise_team_audit".into(),
            description: "Audit team activity".into(),
            input_schema: serde_json::json!({"type": "object"}),
        }],
        capabilities: HashMap::new(),
        server_name: "Enterprise Server".into(),
    };

    cache.update(metadata);
    assert!(cache.is_populated());
    assert_eq!(cache.tool_count(), 1);
    assert_eq!(cache.tools()[0].name, "rigorix_enterprise_team_audit");
    assert!(cache.metadata().is_some());
    assert_eq!(cache.metadata().unwrap().version, "1.0.0");
}

#[test]
fn test_schema_cache_clear() {
    let mut cache = SchemaCache::new();
    let metadata = EnterpriseMetadata {
        version: "1.0.0".into(),
        tools: vec![],
        capabilities: HashMap::new(),
        server_name: "Enterprise Server".into(),
    };
    cache.update(metadata);
    assert!(cache.is_populated());

    cache.clear();
    assert!(!cache.is_populated());
    assert_eq!(cache.tool_count(), 0);
}

#[test]
fn test_schema_cache_is_stale() {
    let mut cache = SchemaCache::new();

    // Empty cache is always stale
    assert!(cache.is_stale(chrono::Duration::seconds(1)));

    let metadata = EnterpriseMetadata {
        version: "1.0.0".into(),
        tools: vec![],
        capabilities: HashMap::new(),
        server_name: "Enterprise Server".into(),
    };
    cache.update(metadata);

    // Just updated — should not be stale
    assert!(!cache.is_stale(chrono::Duration::hours(1)));
    assert!(!cache.is_stale(chrono::Duration::seconds(3600)));

    // Already elapsed — stale
    assert!(cache.is_stale(chrono::Duration::seconds(0)));
}

// -----------------------------------------------------------------------
// Contract: Value objects are constructable
// -----------------------------------------------------------------------

#[test]
fn test_secret_wrapper() {
    let secret = Secret::new("sk-12345".to_string());
    assert_eq!(secret.expose(), "sk-12345");
    assert!(!secret.is_empty());
    assert_eq!(format!("{}", secret), "***REDACTED***");
    assert_eq!(format!("{:?}", secret), "Secret(***REDACTED***)");

    let empty = Secret::new("".to_string());
    assert!(empty.is_empty());
}

#[test]
fn test_proxy_config_validation() {
    // Valid config
    let config = ProxyConfig::new(
        "https://enterprise.example.com".into(),
        "sk-valid-key".into(),
        None,
        None,
        None,
        None,
    );
    assert!(config.is_ok());
    let config = config.unwrap();
    assert_eq!(config.api_url(), "https://enterprise.example.com");
    assert_eq!(config.timeout_secs(), 30);
    assert!(config.tls_verify());
    assert_eq!(config.max_retries(), 3);
    assert_eq!(config.schema_ttl_secs(), 3600);

    // Invalid URL (not HTTPS)
    let config = ProxyConfig::new(
        "http://insecure.example.com".into(),
        "sk-key".into(),
        None,
        None,
        None,
        None,
    );
    assert!(config.is_err());
    assert!(matches!(config.unwrap_err(), ProxyError::Configuration(_)));

    // Empty API key
    let config = ProxyConfig::new(
        "https://example.com".into(),
        "".into(),
        None,
        None,
        None,
        None,
    );
    assert!(config.is_err());

    // Custom values
    let config = ProxyConfig::new(
        "https://example.com".into(),
        "sk-key".into(),
        Some(60),
        Some(false),
        Some(5),
        Some(7200),
    )
    .unwrap();
    assert_eq!(config.timeout_secs(), 60);
    assert!(!config.tls_verify());
    assert_eq!(config.max_retries(), 5);
    assert_eq!(config.schema_ttl_secs(), 7200);
}

#[test]
fn test_json_rpc_request() {
    let req = JsonRpcRequest::new(
        "rigorix_enterprise_team_audit".into(),
        serde_json::json!({"team_id": "123"}),
        1,
    );
    assert_eq!(req.jsonrpc, "2.0");
    assert_eq!(req.method, "rigorix_enterprise_team_audit");
    assert_eq!(req.id, 1);
}

#[test]
fn test_enterprise_metadata() {
    let metadata = EnterpriseMetadata {
        version: "1.0.0".into(),
        tools: vec![ToolSchema {
            name: "rigorix_enterprise_team_audit".into(),
            description: "Audit team activity".into(),
            input_schema: serde_json::json!({"type": "object"}),
        }],
        capabilities: [("supports_approvals".into(), true)].into(),
        server_name: "Rigorix Enterprise".into(),
    };
    assert_eq!(metadata.version, "1.0.0");
    assert_eq!(metadata.tools.len(), 1);
    assert_eq!(metadata.server_name, "Rigorix Enterprise");
}

// -----------------------------------------------------------------------
// Contract: Error types are constructable
// -----------------------------------------------------------------------

#[test]
fn test_proxy_error_constructs() {
    let err = ProxyError::Configuration("invalid URL".into());
    assert!(err.to_string().contains("Configuration"));

    let err = ProxyError::Transport("connection refused".into());
    assert!(err.to_string().contains("Transport"));

    let err = ProxyError::ApiError {
        status: 401,
        message: "Unauthorized".into(),
    };
    assert!(err.to_string().contains("401"));

    let err = ProxyError::Timeout {
        operation: "fetch_schemas".into(),
        timeout_secs: 30,
    };
    assert!(err.to_string().contains("timed out"));

    let err = ProxyError::Authentication("invalid key".into());
    assert!(err.to_string().contains("Authentication"));

    let err = ProxyError::NotEnabled;
    assert!(err.to_string().contains("not enabled"));

    let err = ProxyError::Internal("unexpected".into());
    assert!(err.to_string().contains("Internal"));
}

#[test]
fn test_handler_error_constructs() {
    let err = HandlerError::InvalidArguments("missing method".into());
    assert!(err.to_string().contains("Invalid arguments"));

    let err = HandlerError::ProxyError(ProxyError::NotEnabled);
    assert!(err.to_string().contains("proxy is not enabled"));

    let err = HandlerError::Internal("state error".into());
    assert!(err.to_string().contains("Internal"));
}

#[test]
fn test_tool_call_result() {
    let success = ToolCallResult::success(serde_json::json!({"ok": true}));
    assert!(!success.is_error);
    assert_eq!(success.content.len(), 1);

    let error = ToolCallResult::error("something went wrong");
    assert!(error.is_error);
    assert_eq!(error.content[0].text, "something went wrong");
}

// -----------------------------------------------------------------------
// Contract: Domain events are constructable
// -----------------------------------------------------------------------

#[test]
fn test_enterprise_proxy_event_constructs() {
    let now = Utc::now();

    let event = EnterpriseProxyEvent::EnterpriseToolCalled {
        method: "rigorix_enterprise_team_audit".into(),
        call_id: "call-1".into(),
        proxy_duration_ms: 5,
        timestamp: now,
    };
    assert!(matches!(
        event,
        EnterpriseProxyEvent::EnterpriseToolCalled { .. }
    ));

    let event = EnterpriseProxyEvent::EnterpriseToolCompleted {
        method: "rigorix_enterprise_team_audit".into(),
        call_id: "call-1".into(),
        api_duration_ms: 150,
        response_size: 2048,
        timestamp: now,
    };
    assert!(matches!(
        event,
        EnterpriseProxyEvent::EnterpriseToolCompleted { .. }
    ));

    let event = EnterpriseProxyEvent::EnterpriseToolFailed {
        method: "rigorix_enterprise_team_audit".into(),
        call_id: "call-1".into(),
        error_type: "timeout".into(),
        error_message: "Request timed out".into(),
        timestamp: now,
    };
    assert!(matches!(
        event,
        EnterpriseProxyEvent::EnterpriseToolFailed { .. }
    ));

    let event = EnterpriseProxyEvent::EnterpriseSchemaFetched {
        tool_count: 4,
        version: "1.0.0".into(),
        cached_at: now,
    };
    assert!(matches!(
        event,
        EnterpriseProxyEvent::EnterpriseSchemaFetched { .. }
    ));

    let event = EnterpriseProxyEvent::EnterpriseSchemaRefreshFailed {
        error_message: "Network error".into(),
        retry_count: 2,
        timestamp: now,
    };
    assert!(matches!(
        event,
        EnterpriseProxyEvent::EnterpriseSchemaRefreshFailed { .. }
    ));
}

// -----------------------------------------------------------------------
// Contract: Service traits are properly shaped
// -----------------------------------------------------------------------

#[test]
fn test_enterprise_tool_prefix_is_defined() {
    assert_eq!(ENTERPRISE_TOOL_PREFIX, "rigorix_enterprise_");
    assert!(ENTERPRISE_TOOL_NAMES.contains(&"rigorix_enterprise_call"));
    assert!(ENTERPRISE_TOOL_NAMES.contains(&"rigorix_enterprise_health"));
}

#[test]
fn test_tool_schema_fields() {
    let schema = ToolSchema {
        name: "rigorix_enterprise_team_audit".into(),
        description: "Test tool".into(),
        input_schema: serde_json::json!({"type": "object"}),
    };
    assert_eq!(schema.name, "rigorix_enterprise_team_audit");
    assert_eq!(schema.description, "Test tool");
}

// -----------------------------------------------------------------------
// Contract: Included auto-generated placeholder tests
// -----------------------------------------------------------------------

// Include the auto-generated test files so they compile as part of
// the contract freeze validation.
#[path = "unit/enterprise-proxy/enterpriseproxy-aggregate-root-/enterpriseproxy-aggregate-root-_test.rs"]
mod enterpriseproxy_aggregate_root;

#[path = "unit/enterprise-proxy/schemacache-domain-service-/schemacache-domain-service-_test.rs"]
mod schemacache_domain_service;
