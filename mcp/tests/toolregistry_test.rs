// ToolRegistry integration tests — TDD Red→Green→Refactor
//
// @canonical .pi/architecture/modules/mcp-server.md#toolregistry
// Implements: ToolRegistry integration tests

use rigorix_mcp::mcp_server::domain::entity::ToolRegistry;
use rigorix_mcp::mcp_server::domain::error::RegistrationError;
use rigorix_mcp::mcp_server::domain::value::{
    ContentItem, JsonRpcError, ToolHandler, ToolResult, ToolSchema,
};
use std::sync::Arc;

struct DummyHandler {
    schema: ToolSchema,
}

impl ToolHandler for DummyHandler {
    fn handle(&self, _params: serde_json::Value) -> Result<ToolResult, JsonRpcError> {
        Ok(ToolResult::success(vec![ContentItem::text("ok")]))
    }
    fn schema(&self) -> &ToolSchema {
        &self.schema
    }
}

fn make_handler(name: &str) -> Arc<dyn ToolHandler> {
    Arc::new(DummyHandler {
        schema: ToolSchema::new(name, "test", serde_json::json!({})),
    })
}

#[test]
fn test_toolregistry_defined() {
    let registry = ToolRegistry::default();
    assert_eq!(registry.tool_count(), 0);
    assert!(!registry.has_enterprise_tools());
}

#[test]
fn test_toolregistry_register_and_list() {
    let mut registry = ToolRegistry::default();
    let schema = ToolSchema::new("rigorix_test", "Test tool", serde_json::json!({}));
    let handler = make_handler("rigorix_test");

    let _events = registry.register(schema, handler).expect("Should register");
    assert_eq!(registry.tool_count(), 1);
    assert_eq!(registry.oss_tool_count(), 1);
    assert_eq!(registry.list_schemas().len(), 1);
}

#[test]
fn test_toolregistry_rejects_duplicate() {
    let mut registry = ToolRegistry::default();

    let schema1 = ToolSchema::new("rigorix_dup", "First", serde_json::json!({}));
    let schema2 = ToolSchema::new("rigorix_dup", "Second", serde_json::json!({}));

    assert!(registry.register(schema1, make_handler("rigorix_dup")).is_ok());
    assert!(matches!(
        registry.register(schema2, make_handler("rigorix_dup")),
        Err(RegistrationError::AlreadyRegistered(_))
    ));
}

#[test]
fn test_toolregistry_rejects_invalid_prefix() {
    let mut registry = ToolRegistry::default();
    let schema = ToolSchema::new("bad_name", "No prefix", serde_json::json!({}));
    assert!(matches!(
        registry.register(schema, make_handler("bad_name")),
        Err(RegistrationError::InvalidName(_))
    ));
}

#[test]
fn test_toolregistry_enterprise_registration() {
    let mut registry = ToolRegistry::default();
    let schema = ToolSchema::new(
        "rigorix_enterprise_custom",
        "Enterprise tool",
        serde_json::json!({}),
    );
    assert!(registry
        .register_enterprise(schema, make_handler("rigorix_enterprise_custom"))
        .is_ok());
    assert!(registry.has_enterprise_tools());
    assert_eq!(registry.oss_tool_count(), 0);
}

#[test]
fn test_toolregistry_enterprise_through_oss_fails() {
    let mut registry = ToolRegistry::default();
    let schema = ToolSchema::new(
        "rigorix_enterprise_secret",
        "Enterprise tool",
        serde_json::json!({}),
    );
    assert!(matches!(
        registry.register(schema, make_handler("rigorix_enterprise_secret")),
        Err(RegistrationError::EnterpriseRegistrationForbidden)
    ));
}

#[test]
fn test_toolregistry_unregister() {
    let mut registry = ToolRegistry::default();
    let schema = ToolSchema::new("rigorix_temp", "Temporary", serde_json::json!({}));
    registry
        .register(schema, make_handler("rigorix_temp"))
        .expect("Should register");
    assert_eq!(registry.tool_count(), 1);

    let _events = registry
        .unregister("rigorix_temp")
        .expect("Should unregister");
    assert_eq!(registry.tool_count(), 0);
}

#[test]
fn test_toolregistry_unregister_nonexistent() {
    let mut registry = ToolRegistry::default();
    assert!(matches!(
        registry.unregister("rigorix_nonexistent"),
        Err(RegistrationError::NotFound(_))
    ));
}
