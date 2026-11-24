//! `rigorix-server` binary — axum bootstrap (#888 OSS-C2).
//! @canonical .pi/architecture/modules/rigorix-server.md
//!
//!
//! Serves the frozen `rigorix.*` catalog at `POST /rpc`. The host composition
//! is the SAME one the stdio MCP binary uses (`rigorix_mcp::host::init_host`).

use std::sync::Arc;

use rigorix_server::backend::HostBackend;
use rigorix_server::events::EventHub;
use rigorix_server::state::ServerState;

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

    // The command line, before anything is initialised or bound (#1589). This used to be read by
    // nobody: `--port 3011` bound 3001 without a word. The environment variable is the supported
    // surface, so an unrecognised argument refuses the start instead of being silently discarded, and
    // `--help` prints the surface and exits 0 without binding.
    let configured_bind = std::env::var("RIGORIX_SERVER_BIND").ok();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let startup = rigorix_server::cli::parse_args(&args, configured_bind.as_deref());
    if let Some(code) = startup.exit_code() {
        match startup {
            rigorix_server::cli::Startup::Help => {
                print!("{}", startup.message().unwrap_or_default());
            }
            _ => eprintln!("{}", startup.message().unwrap_or_default()),
        }
        std::process::exit(code);
    }

    let repo_root = std::env::var("RIGORIX_REPO_ROOT").unwrap_or_else(|_| ".".to_string());
    if let Err(e) = rigorix_mcp::host::init_host(&repo_root).await {
        tracing::error!("Failed to initialize host composition: {e}");
        std::process::exit(1);
    }

    let bind = rigorix_server::cli::effective_bind(configured_bind.as_deref());
    let bind_from = rigorix_server::cli::bind_provenance(configured_bind.as_deref());

    // ADR-0001 D8 / #900: bridge the engine event bus the host already built
    // into the SSE hub, so `GET /events` emits live run events (never a second
    // event source). Kept alive for the process lifetime.
    let hub = Arc::new(EventHub::new());
    let _event_bridge = rigorix_server::event_bridge::spawn_event_bridge(
        rigorix_mcp::host::app_state().engine_event_bus(),
        Arc::clone(&hub),
    );
    let state = ServerState::new(Arc::new(HostBackend), hub);

    let app = rigorix_server::app::router(state);

    let listener = match tokio::net::TcpListener::bind(&bind).await {
        Ok(listener) => listener,
        Err(e) => {
            tracing::error!("Failed to bind {bind}: {e}");
            std::process::exit(1);
        }
    };
    tracing::info!(
        "rigorix-server listening on http://{bind} ({bind_from}) (POST /rpc, GET /events, MCP \
         Streamable HTTP at /mcp)"
    );

    // #1650: free mode is a supported choice, not a defect — but binding it beyond loopback changes
    // what is reachable, and the operator is the only one who can decide that. One loud line, never a
    // refusal: the quickstart must keep working, and a warning that stops the server is a config
    // change disguised as a check.
    if let Some(warning) = rigorix_server::origin::free_mode_warning(
        &bind,
        rigorix_mcp::host::app_state().auth_configured_now(),
    ) {
        tracing::warn!("{warning}");
    }

    if let Err(e) = axum::serve(listener, app).await {
        tracing::error!("server error: {e}");
    }
}
