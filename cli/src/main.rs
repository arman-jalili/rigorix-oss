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
    // 1. Parse CLI arguments, then initialize tracing — the order matters for the one
    //    command whose stdout is an artifact (#1414): a log line on the same stream changes
    //    the bytes a conformance vector compares.
    let (command, format) = cli_boundary::cli::parse_args();

    #[cfg(feature = "pinned-inputs")]
    let artifact_stdout = matches!(&command, cli_boundary::cli::CliCommand::GateVector { .. });
    #[cfg(not(feature = "pinned-inputs"))]
    let artifact_stdout = false;

    // 2. Initialize tracing (to stderr when stdout carries an artifact)
    cli_boundary::tracing::init_tracing_with(artifact_stdout);

    // 3. Extract format and verbosity for later use
    // (Format is now threaded to format_and_exit)

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
            cli_boundary::output::format_and_exit(result, format);
        }
    }
}
