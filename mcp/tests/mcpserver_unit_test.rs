// McpServer unit tests — TDD Red→Green→Refactor
//
// @canonical .pi/architecture/modules/mcp-server.md#mcpserver
// Implements: McpServer unit tests

use rigorix_mcp::mcp_server::domain::entity::{McpServer, McpServerStatus};
use rigorix_mcp::mcp_server::domain::value::{
    ClientCapabilities, ClientInfo, ServerCapabilities, ServerConfig, TransportMode,
};

#[test]
fn test_mcpserver_is_defined() {
    let config = ServerConfig::default();
    let server = McpServer::new(config);
    assert_eq!(server.status(), McpServerStatus::Stopped);
}

#[test]
fn test_mcpserver_interacts_with_transport() {
    let mut server = McpServer::new(ServerConfig::default());
    assert!(server.start().is_ok());
    assert!(server.on_transport_opened(TransportMode::Stdio).is_ok());
    assert_eq!(server.status(), McpServerStatus::Running);
}

#[test]
fn test_mcpserver_interacts_with_session_manager() {
    let mut server = McpServer::new(ServerConfig::default());
    server.start().unwrap();
    server.on_transport_opened(TransportMode::Stdio).unwrap();

    let (session, _events) = server
        .create_session(
            ClientInfo {
                name: "test".to_string(),
                version: None,
            },
            ClientCapabilities {
                protocol_version: "2025-03-26".to_string(),
                client_name: Some("test".to_string()),
                client_version: None,
                supports_progress: false,
            },
            ServerCapabilities::default_with_counts(0, 0, 0),
        )
        .expect("Session should be created");

    assert!(!session.initialized);
    assert_eq!(server.session_count(), 1);
}

#[test]
fn test_mcpserver_interacts_with_tool_registry() {
    use rigorix_mcp::mcp_server::domain::entity::ToolRegistry;
    use rigorix_mcp::mcp_server::domain::value::ToolSchema;
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
    let schema = ToolSchema::new("rigorix_test", "Test", serde_json::json!({}));
    let handler: Arc<dyn rigorix_mcp::mcp_server::domain::value::ToolHandler> =
        Arc::new(DummyHandler { schema: schema.clone() });

    assert!(registry.register(schema, handler).is_ok());
    assert_eq!(registry.tool_count(), 1);
}

#[test]
fn test_mcpserver_shutdown_flow() {
    let mut server = McpServer::new(ServerConfig::default());
    server.start().unwrap();
    server.on_transport_opened(TransportMode::Stdio).unwrap();
    assert_eq!(server.status(), McpServerStatus::Running);

    let events = server.shutdown().expect("Shutdown should work");
    assert_eq!(server.status(), McpServerStatus::Stopped);
}
