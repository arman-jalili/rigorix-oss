//! Binary entry point for the rigorix CLI.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — main entry point
//! Issue: issue-contract-freeze
//!
//! Per ADR-002 (CLI/engine split), this is a thin wrapper:
//! 1. Parse CLI args via clap
//! 2. Load and merge configuration
//! 3. Initialize tracing
//! 4. Install signal handlers
//! 5. Create CLI orchestrator
//! 6. Dispatch command → render output → exit
//!
//! # Contract (Frozen)
//! - Entry point only — no business logic
//! - Exit codes: 0 success, 1 error, 2 config error, 3 arg error,
//!   130 cancelled, 137 killed
//! - All errors are caught and formatted before exit

use clap::Parser;
use rigorix::domain::error::CliError;
use rigorix::interfaces::cli::CliArgs;

fn main() -> Result<(), CliError> {
    // Parse CLI arguments
    let _args = CliArgs::parse();

    // TODO: Implementation — config loading, tracing init, signal setup,
    // orchestrator creation, command dispatch, output rendering.
    //
    // ```rust
    // // 1. Load config
    // let cli_config = // ... from args.global_opts
    //
    // // 2. Initialize tracing
    // tracing_subscriber::fmt()...
    //
    // // 3. Install signal handlers
    // let signal_rx = signal_handler.install().await?;
    //
    // // 4. Create orchestrator
    // let orchestrator = factory.create_from_config(cli_config).await?;
    //
    // // 5. Dispatch command
    // match args.command {
    //     CliCommand::Run { .. } => orchestrator.run(...).await,
    //     CliCommand::Plan { .. } => orchestrator.plan(...).await,
    //     // ...
    // }
    // ```
    //
    // This file is a contract placeholder. Implementation follows in
    // subsequent issues.

    eprintln!("rigorix CLI — contract freeze placeholder");
    eprintln!("Use `rigorix --help` to see available commands.");
    eprintln!("\nThis binary currently exits without doing work.");
    eprintln!("Implementation begins in issue-contract-freeze implementation PR.");

    Ok(())
}
