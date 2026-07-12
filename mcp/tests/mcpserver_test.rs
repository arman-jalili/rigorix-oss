// McpServer integration tests — TDD Red→Green→Refactor
//
// @canonical .pi/architecture/modules/mcp-server.md#mcpserver
// Implements: McpServer integration tests

use rigorix_mcp::mcp_server::domain::entity::{McpServer, McpServerStatus};
use rigorix_mcp::mcp_server::domain::value::{
    ClientCapabilities, ClientInfo, ServerCapabilities, ServerConfig, SessionStatus, ToolSchema,
    TransportMode,
};

#[test]
fn test_mcpserver_defined() {
    let config = ServerConfig::default();
    let server = McpServer::new(config);
    assert_eq!(server.status(), McpServerStatus::Stopped);
}

#[test]
fn test_mcpserver_interacts_with_transport() {
    let config = ServerConfig {
        transport_mode: TransportMode::Stdio,
        max_sessions: 10,
        session_timeout_secs: 300,
        bind_address: None,
    };
    let mut server = McpServer::new(config);
    assert!(server.start().is_ok());
    assert!(server.on_transport_opened(TransportMode::Stdio).is_ok());
    assert_eq!(server.status(), McpServerStatus::Running);
}

#[test]
fn test_mcpserver_session_management() {
    let mut server = McpServer::new(ServerConfig::default());
    server.start().unwrap();
    server.on_transport_opened(TransportMode::Stdio).unwrap();

    let client_info = ClientInfo {
        name: "test-client".to_string(),
        version: Some("1.0".to_string()),
    };
    let client_caps = ClientCapabilities {
        protocol_version: "2025-03-26".to_string(),
        client_name: Some("test-client".to_string()),
        client_version: Some("1.0".to_string()),
        supports_progress: false,
    };
    let server_caps = ServerCapabilities::default_with_counts(0, 0, 0);

    let (session, _events) = server
        .create_session(client_info, client_caps, server_caps)
        .expect("Session creation should succeed");

    assert_eq!(session.status, SessionStatus::Pending);
    assert_eq!(server.session_count(), 1);
}

#[test]
fn test_mcpserver_tool_registration() {
    use rigorix_mcp::mcp_server::domain::entity::ToolRegistry;
    use std::sync::Arc;

    struct DummyHandler {
        schema: ToolSchema,
    }

    impl rigorix_mcp::mcp_server::domain::value::ToolHandler for DummyHandler {
        fn handle(
            &self,
            _params: serde_json::Value,
        ) -> Result<
            rigorix_mcp::mcp_server::domain::value::ToolResult,
            rigorix_mcp::mcp_server::domain::value::JsonRpcError,
        > {
            Ok(rigorix_mcp::mcp_server::domain::value::ToolResult::text(
                "ok",
            ))
        }
        fn schema(&self) -> &ToolSchema {
            &self.schema
        }
    }

    let mut registry = ToolRegistry::default();
    let schema = ToolSchema::new("rigorix_execute", "Execute a plan", serde_json::json!({
        "type": "object",
        "properties": {
            "plan": { "type": "string", "description": "Plan to execute" }
        },
        "required": ["plan"]
    }));

    let handler: Arc<dyn rigorix_mcp::mcp_server::domain::value::ToolHandler> =
        Arc::new(DummyHandler { schema: schema.clone() });
    let events = registry.register(schema, handler).expect("Should register");
    assert_eq!(registry.tool_count(), 1);
    assert!(!registry.has_enterprise_tools());
    assert_eq!(events.len(), 1);
}

#[test]
fn test_mcpserver_shutdown() {
    let mut server = McpServer::new(ServerConfig::default());
    server.start().unwrap();
    server.on_transport_opened(TransportMode::Stdio).unwrap();

    let events = server.shutdown().expect("Shutdown should succeed");
    assert_eq!(server.status(), McpServerStatus::Stopped);
}
