//! `rigorix-server` binary — axum bootstrap (#888 OSS-C2).
//!
//! Serves the frozen `rigorix.*` catalog at `POST /rpc`. The host composition
//! is the SAME one the stdio MCP binary uses (`rigorix_mcp::host::init_host`).

use std::sync::Arc;

use axum::body::Bytes;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};

use rigorix_server::backend::{HostBackend, MethodBackend};
use rigorix_server::rpc;

#[tokio::main]
async fn main() {
    // Logs go to stderr; stdout is not a protocol channel for HTTP.
    let tracing_config = rigorix_engine::observability::TracingConfig {
        write_to_stderr: true,
        ..rigorix_engine::observability::TracingConfig::default()
    };
    if let Err(e) = rigorix_engine::observability::init_tracing(&tracing_config) {
        eprintln!("Failed to initialize tracing: {e}");
    }

    let repo_root = std::env::var("RIGORIX_REPO_ROOT").unwrap_or_else(|_| ".".to_string());
    if let Err(e) = rigorix_mcp::host::init_host(&repo_root).await {
        tracing::error!("Failed to initialize host composition: {e}");
        std::process::exit(1);
    }

    let bind =
        std::env::var("RIGORIX_SERVER_BIND").unwrap_or_else(|_| "127.0.0.1:3001".to_string());
    let backend: Arc<dyn MethodBackend> = Arc::new(HostBackend);

    let app = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/rpc", post(rpc_handler))
        .with_state(backend);

    let listener = match tokio::net::TcpListener::bind(&bind).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!("Failed to bind {bind}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!("rigorix-server listening on http://{bind} (POST /rpc)");

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("server error: {e}");
    }
}

/// `POST /rpc` — JSON-RPC 2.0 (single, batch, notification).
///
/// Always HTTP 200 for a well-formed envelope; notifications yield an empty
/// body. Malformed JSON yields a `-32700` error object.
async fn rpc_handler(State(backend): State<Arc<dyn MethodBackend>>, body: Bytes) -> Response {
    let value: serde_json::Value = match serde_json::from_slice(&body) {
        Ok(value) => value,
        Err(_) => return Json(rpc::parse_error()).into_response(),
    };
    match rpc::handle_value(value, backend.as_ref()).await {
        Some(response) => Json(response).into_response(),
        None => StatusCode::OK.into_response(),
    }
}
