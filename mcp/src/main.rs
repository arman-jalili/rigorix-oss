//! Rigorix MCP Gateway — Binary entry point.
//!
//! @canonical .pi/architecture/modules/mcp-server.md
//! Implements: McpServer composition root with stdio transport
//!
//! Starts the MCP server in stdio mode (default). Reads newline-delimited
//! JSON-RPC messages from stdin and writes responses to stdout. Supports
//! graceful shutdown via SIGINT/SIGTERM.
//!
//! This composition root wires together all 15 OSS MCP tools across
//! three bounded contexts (execution, audit, template) with shared
//! in-memory services for development and testing.
//!
//! # Usage
//!
//! ```bash
//! # stdio mode (default — for AI tools like Claude Code, Aider)
//! rigorix-mcp
//!
//! # SSE mode (for GUI tools like Claude Desktop, Cursor)
//! rigorix-mcp (stdio mode)
//! ```

use std::sync::Arc;

use rigorix_engine::configuration::domain::config::Config;
use rigorix_mcp::enterprise_proxy::interfaces::mcp::ENTERPRISE_TOOL_PREFIX;
use rigorix_mcp::host::{
    APP_STATE, AppState, all_tool_descriptors, app_state, build_auth_handler, build_real_engine,
    error_type_name, load_toml_config,
};
use rigorix_mcp::mcp_server::domain::value::{JsonRpcError, JsonRpcMessage, RequestId};
use rigorix_mcp::template_tools::domain::entity::SharedTemplateRepository;
use rigorix_mcp::template_tools::infrastructure::FilesystemTemplateRepository;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::signal;
use tokio_util::sync::CancellationToken;

/// Dispatch an incoming JSON-RPC message to the appropriate handler.
async fn dispatch_message(msg: JsonRpcMessage) -> Option<JsonRpcMessage> {
    let method = msg.method.as_deref()?;
    let id = msg.id.clone()?;
    let params = msg.params.unwrap_or(serde_json::Value::Null);

    let response = match method {
        "initialize" => handle_initialize(&id, &params).await,
        "initialized" => {
            return None;
        }
        "tools/list" => handle_list_tools(&id).await,
        "tools/call" => handle_call_tool(&id, &params).await,
        "resources/list" => handle_list_resources(&id).await,
        "resources/read" => handle_read_resource(&id, &params).await,
        "prompts/list" => handle_list_prompts(&id).await,
        "prompts/get" => handle_get_prompt(&id, &params).await,
        "notifications/cancelled" => {
            return None;
        }
        _ => JsonRpcMessage::error(id, JsonRpcError::method_not_found(method)),
    };

    Some(response)
}

// ---------------------------------------------------------------------------
// initialize handler
// ---------------------------------------------------------------------------

async fn handle_initialize(id: &RequestId, params: &serde_json::Value) -> JsonRpcMessage {
    // GAP-A-22: route initialize through the runtime McpServerService — this
    // creates the session and negotiates the protocol version (no fabricated
    // response).
    let protocol_version = params
        .get("protocolVersion")
        .and_then(|v| v.as_str())
        .unwrap_or("2025-03-26")
        .to_string();
    let client_info = {
        let ci = params.get("clientInfo").cloned().unwrap_or_default();
        rigorix_mcp::mcp_server::domain::value::ClientInfo {
            name: ci
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string(),
            version: ci
                .get("version")
                .and_then(|v| v.as_str())
                .map(str::to_string),
        }
    };
    let caps = params.get("capabilities").cloned().unwrap_or_default();
    let input = rigorix_mcp::mcp_server::application::dto::InitializeInput {
        protocol_version: protocol_version.clone(),
        client_info: client_info.clone(),
        capabilities: rigorix_mcp::mcp_server::domain::value::ClientCapabilities {
            protocol_version: protocol_version.clone(),
            client_name: Some(client_info.name.clone()),
            client_version: client_info.version.clone(),
            supports_progress: caps
                .get("experimental")
                .and_then(|e| e.get("progress"))
                .and_then(|p| p.as_bool())
                .unwrap_or(false),
        },
    };

    match app_state().mcp_service.initialize(input).await {
        Ok((output, _events)) => {
            let result = serde_json::json!({
                "protocolVersion": output.protocol_version,
                "capabilities": {
                    "tools": {},
                    "resources": {},
                    "prompts": {}
                },
                "serverInfo": {
                    "name": output.server_info,
                    "version": "0.1.0"
                }
            });
            JsonRpcMessage::success(id.clone(), result)
        }
        Err(err) => JsonRpcMessage::error(
            id.clone(),
            JsonRpcError::internal_error(format!("initialize failed: {err}")),
        ),
    }
}

// ---------------------------------------------------------------------------
// tools/list — returns all OSS + enterprise tool descriptors
// ---------------------------------------------------------------------------

async fn handle_list_tools(id: &RequestId) -> JsonRpcMessage {
    let mut tools = all_tool_descriptors();

    // Append enterprise tools if proxy is enabled
    if let Some(proxy) = app_state().enterprise_proxy.as_ref() {
        // Static tools: always available when proxy is configured
        tools.push(
            rigorix_mcp::enterprise_proxy::interfaces::mcp::rigorix_enterprise_call_tool_descriptor(
            ),
        );
        tools.push(
            rigorix_mcp::enterprise_proxy::interfaces::mcp::rigorix_enterprise_health_tool_descriptor(),
        );
        // Dynamic tools: populated from schema cache (if init succeeded)
        for schema in proxy.available_tools() {
            tools.push(serde_json::json!({
                "name": schema.name,
                "description": schema.description,
                "inputSchema": schema.input_schema
            }));
        }
    }

    // Auth tools (ADR-008) — appended only when an IdP is configured.
    if app_state().auth_handler.is_some() {
        tools.extend(rigorix_mcp::auth::interfaces::mcp::auth_tool_descriptors());
    }

    let result = serde_json::json!({ "tools": tools });
    JsonRpcMessage::success(id.clone(), result)
}

// ---------------------------------------------------------------------------
// tools/call — dispatches to the correct handler
// ---------------------------------------------------------------------------

async fn handle_call_tool(id: &RequestId, params: &serde_json::Value) -> JsonRpcMessage {
    let tool_name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or(serde_json::Value::Null);

    // Route rigorix_enterprise_* calls to the enterprise proxy
    if tool_name.starts_with(ENTERPRISE_TOOL_PREFIX) {
        match &app_state().enterprise_proxy {
            Some(proxy) => match proxy.handle(tool_name, arguments.clone()).await {
                Ok(result) => {
                    let response = serde_json::json!({
                        "content": [{"type": "text", "text": serde_json::to_string(&result).unwrap_or_default()}],
                        "isError": false
                    });
                    return JsonRpcMessage::success(id.clone(), response);
                }
                Err(e) => {
                    let diagnostic =
                        rigorix_mcp::enterprise_proxy::interfaces::mcp::format_enterprise_error(
                            error_type_name(&e),
                            &e.to_string(),
                        );
                    let response = serde_json::json!({
                        "content": [{"type": "text", "text": serde_json::to_string(&diagnostic).unwrap_or_default()}],
                        "isError": true
                    });
                    return JsonRpcMessage::success(id.clone(), response);
                }
            },
            None => {
                let response = serde_json::json!({
                    "content": [{"type": "text", "text": "Enterprise proxy is not configured. Set ENTERPRISE_API_URL and ENTERPRISE_API_KEY."}],
                    "isError": true
                });
                return JsonRpcMessage::success(id.clone(), response);
            }
        }
    }

    match app_state().handle_tool_call(tool_name, &arguments).await {
        Ok(result) => {
            let response = serde_json::json!({
                "content": [
                    {
                        "type": "text",
                        "text": serde_json::to_string(&result).unwrap_or_default()
                    }
                ],
                "isError": false
            });
            JsonRpcMessage::success(id.clone(), response)
        }
        Err(error) => {
            let response = serde_json::json!({
                "content": [
                    {
                        "type": "text",
                        "text": error["error"].as_str().unwrap_or("Unknown error")
                    }
                ],
                "isError": true
            });
            JsonRpcMessage::success(id.clone(), response)
        }
    }
}

// ---------------------------------------------------------------------------
// resources/list
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// resources/read
// ---------------------------------------------------------------------------

async fn handle_read_resource(id: &RequestId, params: &serde_json::Value) -> JsonRpcMessage {
    let uri = params
        .get("uri")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    // Route through the wired mcp_server service so the library module is
    // the live backend for the advertised resources.
    let input = rigorix_mcp::mcp_server::application::dto::ReadResourceInput { uri: uri.clone() };
    match app_state().mcp_service.read_resource(input).await {
        Ok(output) => {
            let result = serde_json::json!({
                "contents": [
                    {
                        "uri": output.uri,
                        "mimeType": output.mime_type,
                        "text": output.text
                    }
                ]
            });
            JsonRpcMessage::success(id.clone(), result)
        }
        Err(err) => JsonRpcMessage::error(
            id.clone(),
            JsonRpcError::invalid_params(format!("Resource read failed: {err}")),
        ),
    }
}

// ---------------------------------------------------------------------------
// prompts/list
// ---------------------------------------------------------------------------

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

// ---------------------------------------------------------------------------
// prompts/get
// ---------------------------------------------------------------------------

async fn handle_get_prompt(id: &RequestId, params: &serde_json::Value) -> JsonRpcMessage {
    let name = params
        .get("name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown");

    match name {
        "rigorix_introduction" => {
            let text = concat!(
                "You are using Rigorix, an AI code-governance engine that plans, ",
                "executes, and audits multi-step work against a frozen contract.\n\n",
                "Key capabilities:\n",
                "  • rigorix_list_templates / rigorix_get_template — inspect plan templates\n",
                "  • rigorix_validate_plan — check a plan against enforcement policies\n",
                "  • rigorix_run — execute a template's DAG through the engine\n",
                "  • rigorix_approve_execution — human sign-off when a step requires it\n",
                "    (plans may mark steps requires_approval: true; execution pauses until approved)\n",
                "  • rigorix_check_enforcement — current enforcement status and budget\n",
                "  • rigorix_get_execution_status / rigorix_get_audit_log — inspect evidence\n\n",
                "All execution is gated: budgets, safety caps, tool policy, and (when ",
                "configured) a permission mode (read_only / workspace_write / ",
                "dangerous_full_access). Every audit event is timestamped and, when a ",
                "signing key is configured, HMAC-signed for tamper-evident evidence.\n\n",
                "Start by listing templates, then run one with rigorix_run.",
            );
            let result = serde_json::json!({
                "description": "Introduction to Rigorix tool usage",
                "messages": [
                    {
                        "role": "user",
                        "content": { "type": "text", "text": text }
                    }
                ]
            });
            JsonRpcMessage::success(id.clone(), result)
        }
        _ => JsonRpcMessage::error(
            id.clone(),
            JsonRpcError::internal_error(format!("Prompt '{}' not found", name)),
        ),
    }
}

// ---------------------------------------------------------------------------
// Stdio Server
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
                                let err = JsonRpcError::parse_error();
                                let error_msg = serde_json::json!({
                                    "jsonrpc": "2.0",
                                    "id": null,
                                    "error": {
                                        "code": err.code,
                                        "message": err.message
                                    }
                                });
                                let _ = writer.write_all(format!("{}\n", serde_json::to_string(&error_msg).unwrap_or_default()).as_bytes()).await;
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
    // Initialize tracing via the engine's centralized observability layer.
    // Logs MUST go to stderr: stdout is the JSON-RPC channel for stdio mode.
    let tracing_config = rigorix_engine::observability::TracingConfig {
        write_to_stderr: true,
        ..rigorix_engine::observability::TracingConfig::default()
    };
    if let Err(e) = rigorix_engine::observability::init_tracing(&tracing_config) {
        eprintln!("Failed to initialize tracing: {e}");
    }

    // ── Build real engine facade ──
    let repo_root = std::env::var("RIGORIX_REPO_ROOT").unwrap_or_else(|_| ".".to_string());
    let (engine, engine_audit) = match build_real_engine(&repo_root).await {
        Ok((e, audit)) => {
            tracing::info!("EngineFacadeImpl initialized with real rigorix-engine");
            (e, audit)
        }
        Err(e) => {
            tracing::error!("Failed to build real engine: {}. Exiting.", e);
            return;
        }
    };

    // ── Initialize app state ──
    let template_repo: SharedTemplateRepository =
        Arc::new(FilesystemTemplateRepository::new(".rigorix/templates"));
    // Same resolution as build_real_engine: rigorix.toml audit_hmac_key or
    // RIGORIX_HMAC_KEY env — used to sign the envelopes read back via
    // rigorix_read_audit so the evidence is real, not a sample.
    let audit_hmac_key = load_toml_config::<Config>(&repo_root, "rigorix.toml")
        .audit_hmac_key
        .or_else(|| std::env::var("RIGORIX_HMAC_KEY").ok())
        .filter(|k| !k.is_empty());
    let _ = APP_STATE.set(AppState::new(
        engine,
        template_repo,
        audit_hmac_key,
        build_auth_handler().await,
        engine_audit,
    ));

    let cancel = CancellationToken::new();

    // Parse args
    let args: Vec<String> = std::env::args().collect();
    let use_sse = args.iter().any(|a| a == "--sse");
    // --bind is accepted for CLI compatibility; the server runs over stdio
    // (GAP-A-10: SSE transport removed).
    let _bind_addr = args
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

    // GAP-A-10: SSE transport removed — the --sse flag never started a real
    // server (it logged 'not fully implemented' and exited). The server now
    // always runs over stdio; --sse is accepted with a deprecation notice so
    // existing invocations do not silently change behavior.
    if use_sse {
        tracing::warn!(
            "SSE transport is not supported (removed); starting in stdio mode. See .pi/architecture/modules/mcp-server.md"
        );
    }
    tracing::info!("Starting MCP Server in stdio mode");
    run_stdio_server(cancel).await;

    tracing::info!("MCP Server shut down gracefully");
}
