//! Integration tests for the cli_boundary module.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#tests
//! Implements: Contract Freeze — integration test stubs
//! Issue: issue-contract-freeze

use crate::cli_boundary::cli::*;
use crate::cli_boundary::config::CliConfig;
use crate::cli_boundary::dispatch::DispatchResult;
use crate::cli_boundary::error::CliError;
use crate::cli_boundary::output::{self, LogFormatter};

// ------------------------------------------------------------------
// CliParser tests
// ------------------------------------------------------------------

#[test]
fn test_parse_args_defaults_to_tui() {
    let (cmd, _format) = parse_args();
    assert!(matches!(
        cmd,
        CliCommand::Tui {
            exec: None,
            run: None
        }
    ));
}

#[test]
fn test_format_variants() {
    let _ = Format::Pretty;
    let _ = Format::Json;
    let _ = Format::Markdown;
    let _ = Format::Quiet;
}

#[test]
fn test_format_default() {
    assert_eq!(Format::default(), Format::Pretty);
}

// ------------------------------------------------------------------
// Output formatter tests
// ------------------------------------------------------------------

#[test]
fn test_pretty_formatter_success() {
    let formatter = output::PrettyFormatter;
    let result = DispatchResult::success("test");
    let output = formatter.format_summary(&result);
    assert!(
        output.contains("test"),
        "expected summary text, got: {output:?}"
    );
}

#[test]
fn test_pretty_formatter_error() {
    let formatter = output::PrettyFormatter;
    let result = DispatchResult::error("something failed", 1);
    let output = formatter.format_error(&result);
    assert!(output.contains("Error"));
    assert!(output.contains("something failed"));
}

#[test]
fn test_json_formatter_success() {
    let formatter = output::JsonFormatter;
    let result = DispatchResult::success("test");
    let output = formatter.format_summary(&result);
    assert!(output.contains("\"success\": true"));
    assert!(output.contains("\"summary\": \"test\""));
}

#[test]
fn test_json_formatter_error() {
    let formatter = output::JsonFormatter;
    let result = DispatchResult::error("failed", 1);
    let output = formatter.format_error(&result);
    assert!(output.contains("\"success\": false"));
    assert!(output.contains("\"exit_code\": 1"));
}

#[test]
fn test_markdown_formatter_success() {
    let formatter = output::MarkdownFormatter;
    let result = DispatchResult::success("test");
    let output = formatter.format_summary(&result);
    assert!(output.contains("✅"));
    assert!(output.contains("**test**"));
}

#[test]
fn test_quiet_formatter_success() {
    let formatter = output::QuietFormatter;
    let result = DispatchResult::success("test");
    let output = formatter.format_summary(&result);
    assert!(output.is_empty());
}

#[test]
fn test_formatter_for() {
    let _ = output::formatter_for(Format::Pretty);
    let _ = output::formatter_for(Format::Json);
    let _ = output::formatter_for(Format::Markdown);
    let _ = output::formatter_for(Format::Quiet);
}

// ------------------------------------------------------------------
// Config loader tests
// ------------------------------------------------------------------

#[test]
fn test_config_defaults() {
    let config = CliConfig::default();
    assert!(config.engine_config.is_none());
    assert_eq!(config.verbose, 0);
}

#[test]
fn test_config_engine_config_not_loaded() {
    let config = CliConfig::default();
    assert!(config.engine_config().is_err());
}

// ------------------------------------------------------------------
// Error type tests
// ------------------------------------------------------------------

#[test]
fn test_cli_error_exit_codes() {
    assert_eq!(CliError::General("err".into()).exit_code(), 1);
    assert_eq!(CliError::Config("err".into()).exit_code(), 2);
    assert_eq!(CliError::InvalidArgs("err".into()).exit_code(), 3);
    assert_eq!(CliError::Cancelled.exit_code(), 130);
    assert_eq!(CliError::Killed.exit_code(), 137);
}

#[test]
fn test_cli_error_display() {
    let err = CliError::Config("missing file".into());
    let msg = err.to_string();
    assert!(msg.contains("Configuration error"));
    assert!(msg.contains("missing file"));
}

#[test]
fn test_cli_error_from_string() {
    let err: CliError = "something went wrong".into();
    assert_eq!(err.exit_code(), 1);
}

// ------------------------------------------------------------------
// DispatchResult tests
// ------------------------------------------------------------------

#[test]
fn test_dispatch_result_success() {
    let result = DispatchResult::success("done");
    assert!(result.is_success());
    assert_eq!(result.exit_code, 0);
}

#[test]
fn test_dispatch_result_error() {
    let result = DispatchResult::error("fail", 1);
    assert!(!result.is_success());
    assert_eq!(result.exit_code, 1);
}

#[test]
fn test_dispatch_result_display() {
    let result = DispatchResult::success("hello");
    assert_eq!(result.to_string(), "hello");
}

#[test]
fn test_dispatch_result_success_with_data() {
    let data = serde_json::json!({"key": "value"});
    let result = DispatchResult::success_with_data("with data", data);
    assert!(result.is_success());
    assert!(result.data.is_some());
}
