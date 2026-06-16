//! Rigorix CLI binary entry point.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#startup-entry-point
//! Implements: Binary entry point
//! Issue: issue-cliparser
//!
//! # Startup Flow
//!
//! 1. Initialize tracing (tracing-subscriber with RIGORIX_LOG filter)
//! 2. Parse CLI arguments (Clap)
//! 3. Load configuration from all sources
//! 4. Install signal handler (Ctrl+C, SIGTERM)
//! 5. Branch:
//!    - TUI command → launch interactive TUI
//!    - Other commands → dispatch via cli_boundary::dispatch

use rigorix::cli_boundary;

#[tokio::main]
async fn main() {
    // 1. Initialize tracing
    cli_boundary::tracing::init_tracing();

    // 2. Parse CLI arguments
    let command = cli_boundary::cli::parse_args();

    // 3. Extract format and verbosity for later use
    // (These are stored in CliConfig after config loading)

    // 4. Load configuration
    let config = cli_boundary::config::load_config();

    // 5. Install signal handler
    let cancellation_token = cli_boundary::signal::install_signal_handler();

    // 6. Dispatch
    match command {
        cli_boundary::cli::CliCommand::Tui { exec, run } => {
            rigorix::tui::run(config, cancellation_token, exec, run).await;
        }
        _ => {
            let result =
                cli_boundary::dispatch::dispatch(command, config, cancellation_token).await;
            cli_boundary::output::format_and_exit(result);
        }
    }
}
