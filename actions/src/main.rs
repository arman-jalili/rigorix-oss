//! Rigorix GitHub Action — binary entry point.
//!
//! Parses GitHub Action context, routes events to engine calls,
//! and formats results as GitHub-native outputs.
//!
//! @canonical actions/.pi/architecture/modules/action-entrypoint.md

use std::process;

fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| "info".into()),
        )
        .init();

    tracing::info!("Rigorix GitHub Action v{}", env!("CARGO_PKG_VERSION"));
    tracing::info!(
        "Mode: {}",
        std::env::var("INPUT_MODE").unwrap_or_else(|_| "auto".into())
    );

    // ── Placeholder: full dispatch will be wired in Phase 5 ──
    tracing::warn!(
        "Action entrypoint is scaffolded — dispatch logic will be wired in Phase 5 (action-entrypoint module)"
    );

    process::exit(0);
}
