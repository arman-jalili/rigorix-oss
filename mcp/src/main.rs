//! Rigorix MCP Gateway — Binary entry point.
//!
//! @canonical .pi/architecture/modules/mcp-server.md
//! Implements: McpServer composition root with stdio transport
//!
//! Starts the MCP server in stdio mode (default). Reads newline-delimited
//! JSON-RPC messages from stdin and writes responses to stdout. Supports
//! graceful shutdown via SIGINT/SIGTERM.
//!
//! # Usage
//!
//! ```bash
//! # stdio mode (default — for AI tools like Claude Code, Aider)
//! rigorix-mcp
//!
//! # SSE mode (for GUI tools like Claude Desktop, Cursor)
//! rigorix-mcp --sse --bind 127.0.0.1:3001
//! ```

use rigorix_mcp::mcp_server::domain::value::{JsonRpcError, JsonRpcMessage, RequestId};
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::signal;
use tokio_util::sync::CancellationToken;

/// Dispatch an incoming JSON-RPC message to the appropriate handler.
///
/// Returns a JSON-RPC response message, or None for notifications.
async fn dispatch_message(msg: JsonRpcMessage) -> Option<JsonRpcMessage> {
    let method = msg.method.as_deref()?;
    let id = msg.id.clone()?;
    let params = msg.params.unwrap_or(serde_json::Value::Null);

    let response = match method {
        "initialize" => handle_initialize(&id, &params).await,
        "initialized" => {
            // Notification — no response needed
            return None;
        }
        "tools/list" => handle_list_tools(&id).await,
        "tools/call" => handle_call_tool(&id, &params).await,
        "resources/list" => handle_list_resources(&id).await,
        "resources/read" => handle_read_resource(&id, &params).await,
        "prompts/list" => handle_list_prompts(&id).await,
        "prompts/get" => handle_get_prompt(&id, &params).await,
        "notifications/cancelled" => {
            // Notification — no response needed
            return None;
        }
        _ => JsonRpcMessage::error(id, JsonRpcError::method_not_found(method)),
    };

    Some(response)
}

// ---------------------------------------------------------------------------
// Handlers
// ---------------------------------------------------------------------------

async fn handle_initialize(id: &RequestId, _params: &serde_json::Value) -> JsonRpcMessage {
    let result = serde_json::json!({
        "protocolVersion": "2025-03-26",
        "capabilities": {
            "tools": {},
            "resources": {},
            "prompts": {}
        },
        "serverInfo": {
            "name": "Rigorix MCP Gateway",
            "version": "0.1.0"
        }
    });
    JsonRpcMessage::success(id.clone(), result)
}

async fn handle_list_tools(id: &RequestId) -> JsonRpcMessage {
    let result = serde_json::json!({ "tools": [] });
    JsonRpcMessage::success(id.clone(), result)
}

async fn handle_call_tool(id: &RequestId, params: &serde_json::Value) -> JsonRpcMessage {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    JsonRpcMessage::error(
        id.clone(),
        JsonRpcError::tool_execution_failed(name, "Tool not implemented in this phase"),
    )
}

async fn handle_list_resources(id: &RequestId) -> JsonRpcMessage {
    let result = serde_json::json!({
        "resources": [
            {
                "uri": "rigorix://audit/{id}",
                "name": "Audit Trail",
                "description": "Read an audit trail by execution ID",
                "mimeType": "text/plain"
            },
            {
                "uri": "rigorix://templates/{name}",
                "name": "Template",
                "description": "Read a template by name",
                "mimeType": "text/plain"
            }
        ]
    });
    JsonRpcMessage::success(id.clone(), result)
}

async fn handle_read_resource(id: &RequestId, params: &serde_json::Value) -> JsonRpcMessage {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    JsonRpcMessage::error(
        id.clone(),
        JsonRpcError::internal_error(format!("Resource '{}' not implemented", uri)),
    )
}

async fn handle_list_prompts(id: &RequestId) -> JsonRpcMessage {
    let result = serde_json::json!({
        "prompts": [
            {
                "name": "rigorix_introduction",
                "description": "Introduction to Rigorix tool usage",
                "arguments": []
            }
        ]
    });
    JsonRpcMessage::success(id.clone(), result)
}

async fn handle_get_prompt(id: &RequestId, params: &serde_json::Value) -> JsonRpcMessage {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    JsonRpcMessage::error(
        id.clone(),
        JsonRpcError::internal_error(format!("Prompt '{}' not implemented", name)),
    )
}

// ---------------------------------------------------------------------------
// Stdio Server — reads JSON-RPC from stdin, writes responses to stdout
// ---------------------------------------------------------------------------

async fn run_stdio_server(cancel: CancellationToken) {
    let stdin = tokio::io::stdin();
    let stdout = tokio::io::stdout();
    let mut reader = BufReader::new(stdin).lines();
    let mut writer = stdout;

    tracing::info!("Rigorix MCP Gateway ready (stdio mode)");

    loop {
        tokio::select! {
            line = reader.next_line() => {
                match line {
                    Ok(Some(line)) => {
                        let line = line.trim().to_string();
                        if line.is_empty() {
                            continue;
                        }

                        match serde_json::from_str::<JsonRpcMessage>(&line) {
                            Ok(msg) => {
                                let response = dispatch_message(msg).await;
                                if let Some(resp) = response {
                                    let json = serde_json::to_string(&resp)
                                        .unwrap_or_else(|_| "{}".to_string());
                                    if let Err(e) = writer.write_all(format!("{}\n", json).as_bytes()).await {
                                        tracing::error!("Failed to write response: {}", e);
                                        break;
                                    }
                                    let _ = writer.flush().await;
                                }
                            }
                            Err(e) => {
                                // Send parse error
                                let err = JsonRpcError::parse_error();
                                let error_msg = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": null,
                                    "error": {
                                        "code": err.code,
                                        "message": err.message
                                    }
                                });
                                let _ = writer.write_all(format!("{}\n", serde_json::to_string(&error_msg).unwrap()).as_bytes()).await;
                                let _ = writer.flush().await;
                                tracing::warn!("Failed to parse JSON-RPC message: {}", e);
                            }
                        }
                    }
                    Ok(None) => {
                        tracing::info!("stdin closed, shutting down");
                        break;
                    }
                    Err(e) => {
                        tracing::error!("Error reading stdin: {}", e);
                        break;
                    }
                }
            }
            _ = cancel.cancelled() => {
                tracing::info!("Shutdown signal received, stopping stdio server");
                break;
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    // Initialize tracing
    tracing_subscriber::fmt()
        .with_writer(std::io::stderr)
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("info")),
        )
        .init();

    let cancel = CancellationToken::new();

    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let use_sse = args.iter().any(|a| a == "--sse");
    let bind_addr = args
        .iter()
        .position(|a| a == "--bind")
        .and_then(|i| args.get(i + 1).cloned())
        .unwrap_or_else(|| "127.0.0.1:3001".to_string());

    // Set up graceful shutdown
    let cancel_clone = cancel.clone();
    tokio::spawn(async move {
        #[cfg(unix)]
        {
            use signal::unix::SignalKind;
            if let (Ok(mut sigint), Ok(mut sigterm)) = (
                signal::unix::signal(SignalKind::interrupt()),
                signal::unix::signal(SignalKind::terminate()),
            ) {
                tokio::select! {
                    _ = sigint.recv() => tracing::info!("Received SIGINT"),
                    _ = sigterm.recv() => tracing::info!("Received SIGTERM"),
                }
            }
        }
        #[cfg(not(unix))]
        {
            let _ = signal::ctrl_c().await;
        }
        cancel_clone.cancel();
    });

    if use_sse {
        tracing::info!("Starting MCP Server in SSE mode on {}", bind_addr);
        // SSE mode would use Axum here (future phase)
        tracing::warn!("SSE mode is not fully implemented in this phase");
    } else {
        tracing::info!("Starting MCP Server in stdio mode");
        run_stdio_server(cancel).await;
    }

    tracing::info!("MCP Server shut down gracefully");
}
