//! Rigorix CLI binary entry point.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#startup-entry-point
//! Implements: Contract Freeze — binary entry point
//! Issue: issue-contract-freeze
//!
//! # Startup Flow
//!
//! 1. Load config from all sources (TOML + env + CLI flags)
//! 2. Install signal handler (Ctrl+C, SIGTERM)
//! 3. Initialize tracing (tracing-subscriber with RIGORIX_LOG filter)
//! 4. Branch:
//!    - No subcommand → launch interactive TUI (`tui::run`)
//!    - Subcommand given → dispatch via `cli_boundary::dispatch`
//!
//! # Contract (Frozen)
//!
//! The `main()` function signature and startup sequence are frozen.
//! No business logic lives here — only bootstrapping and dispatch.

use rigorix::cli_boundary;

#[tokio::main]
async fn main() {
    // 1. Initialize tracing
    cli_boundary::tracing::init_tracing();

    // 2. Load configuration
    let config = cli_boundary::config::load_config();

    // 3. Install signal handler
    let cancellation_token = cli_boundary::signal::install_signal_handler();

    // 4. Parse CLI arguments
    let command = cli_boundary::cli::parse_args();

    // 5. Dispatch
    match command {
        cli_boundary::cli::CliCommand::Tui { exec, run } => {
            rigorix::tui::run(config, cancellation_token, exec, run).await;
        }
        _ => {
            let result = cli_boundary::dispatch::dispatch(command, config, cancellation_token).await;
            cli_boundary::output::format_and_exit(result);
        }
    }
}
