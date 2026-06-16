//! Tests for the CLI boundary domain types.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md
//! Implements: Contract Freeze — CLI boundary tests
//! Issue: issue-contract-freeze
//!
//! Contract tests for CLI domain types: DTO serialization roundtrips,
//! config defaults, error exit codes, and event serialization.
//!
//! # Contract (Frozen)
//! - Tests validate serialization roundtrips (JSON, TOML)
//! - Tests validate default values and invariants
//! - Tests validate error semantics (exit codes, retriable)
//! - No tests for implementation details

use crate::cli_boundary::domain::error::CliError;
use crate::cli_boundary::domain::event::{
    CliEvent, SessionCompletedPayload, SessionOutcome, SessionStartedPayload,
};
use crate::configuration::domain::config::{
    CliConfig, ColorMode, LogFormat, LogLevel, OutputFormat,
};

// -----------------------------------------------------------------------
// Config tests
// -----------------------------------------------------------------------

#[test]
fn test_output_format_default() {
    assert_eq!(OutputFormat::default(), OutputFormat::Pretty);
}

#[test]
fn test_output_format_is_human_readable() {
    assert!(OutputFormat::Pretty.is_human_readable());
    assert!(!OutputFormat::Json.is_human_readable());
    assert!(!OutputFormat::Quiet.is_human_readable());
}

#[test]
fn test_output_format_is_machine_readable() {
    assert!(OutputFormat::Json.is_machine_readable());
    assert!(!OutputFormat::Pretty.is_machine_readable());
    assert!(!OutputFormat::Quiet.is_machine_readable());
}

#[test]
fn test_output_format_is_quiet() {
    assert!(OutputFormat::Quiet.is_quiet());
    assert!(!OutputFormat::Pretty.is_quiet());
    assert!(!OutputFormat::Json.is_quiet());
}

#[test]
fn test_color_mode_should_color() {
    assert!(ColorMode::Always.should_color(false));
    assert!(!ColorMode::Never.should_color(true));
    assert!(ColorMode::Auto.should_color(true));
    assert!(!ColorMode::Auto.should_color(false));
}

#[test]
fn test_cli_config_default() {
    let config = CliConfig::default();
    assert_eq!(config.output_format, OutputFormat::Pretty);
    assert!(config.tui_enabled);
    assert_eq!(config.color, ColorMode::Auto);
    assert_eq!(config.log_level, LogLevel::Info);
    assert!(!config.force_tui);
}

#[test]
fn test_log_level_tracing_filter() {
    assert_eq!(LogLevel::Trace.as_tracing_filter(), "trace");
    assert_eq!(LogLevel::Debug.as_tracing_filter(), "debug");
    assert_eq!(LogLevel::Info.as_tracing_filter(), "info");
    assert_eq!(LogLevel::Warn.as_tracing_filter(), "warn");
    assert_eq!(LogLevel::Error.as_tracing_filter(), "error");
}

#[test]
fn test_log_level_all_contains_all() {
    let all = LogLevel::all();
    assert_eq!(all.len(), 5);
}

// -----------------------------------------------------------------------
// Error tests
// -----------------------------------------------------------------------

#[test]
fn test_config_errors_have_exit_code_2() {
    let err = CliError::ConfigNotFound {
        detail: "no config found".into(),
    };
    assert_eq!(err.exit_code(), 2);

    let err = CliError::ConfigParseError {
        path: "rigorix.toml".into(),
        detail: "parse error".into(),
    };
    assert_eq!(err.exit_code(), 2);

    let err = CliError::MissingConfig {
        field: "api_key".into(),
        hint: "set RIGORIX_API_KEY".into(),
    };
    assert_eq!(err.exit_code(), 2);
}

#[test]
fn test_argument_errors_have_exit_code_3() {
    let err = CliError::UnknownCommand {
        command: "foo".into(),
        suggestions: vec!["run".into(), "plan".into()],
    };
    assert_eq!(err.exit_code(), 3);

    let err = CliError::InvalidArguments {
        command: "run".into(),
        detail: "missing intent".into(),
    };
    assert_eq!(err.exit_code(), 3);

    let err = CliError::MissingArgument {
        command: "run".into(),
        argument: "intent".into(),
    };
    assert_eq!(err.exit_code(), 3);
}

#[test]
fn test_cancelled_exit_code_is_130() {
    assert_eq!(CliError::SessionCancelled.exit_code(), 130);
}

#[test]
fn test_timeout_exit_code_is_137() {
    let err = CliError::SessionTimeout { timeout_secs: 60 };
    assert_eq!(err.exit_code(), 137);
}

#[test]
fn test_retriable_errors() {
    assert!(
        CliError::ConfigNotFound {
            detail: "test".into()
        }
        .is_retriable()
    );
    assert!(
        CliError::MissingConfig {
            field: "key".into(),
            hint: "set key".into()
        }
        .is_retriable()
    );
    assert!(
        CliError::MissingArgument {
            command: "run".into(),
            argument: "intent".into()
        }
        .is_retriable()
    );
    assert!(!CliError::SessionCancelled.is_retriable());
}

// -----------------------------------------------------------------------
// Event serialization tests
// -----------------------------------------------------------------------

#[test]
fn test_session_outcome_display() {
    assert_eq!(SessionOutcome::Completed.to_string(), "completed");
    assert_eq!(SessionOutcome::Failed.to_string(), "failed");
    assert_eq!(SessionOutcome::Cancelled.to_string(), "cancelled");
    assert_eq!(SessionOutcome::TimedOut.to_string(), "timed_out");
}

#[test]
fn test_cli_event_serde_roundtrip() {
    let event = CliEvent::SessionStarted(SessionStartedPayload {
        session_id: "test-123".into(),
        command: "run".into(),
        template_id: Some("add-endpoint".into()),
        timestamp: "2026-01-01T00:00:00Z".into(),
    });

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: CliEvent = serde_json::from_str(&json).unwrap();

    match deserialized {
        CliEvent::SessionStarted(payload) => {
            assert_eq!(payload.session_id, "test-123");
            assert_eq!(payload.command, "run");
            assert_eq!(payload.template_id, Some("add-endpoint".into()));
        }
        _ => panic!("Expected SessionStarted variant"),
    }
}

#[test]
fn test_cli_event_serde_tagged() {
    let event = CliEvent::SessionCompleted(SessionCompletedPayload {
        session_id: "test-456".into(),
        outcome: SessionOutcome::Completed,
        duration_ms: 1500,
        nodes_completed: 5,
        nodes_failed: 0,
        nodes_skipped: 1,
        timestamp: "2026-01-01T00:00:00Z".into(),
    });

    let json = serde_json::to_string(&event).unwrap();
    assert!(json.contains(r#""type":"SessionCompleted""#));
    assert!(json.contains(r#""outcome":"completed""#));

    let deserialized: CliEvent = serde_json::from_str(&json).unwrap();
    match deserialized {
        CliEvent::SessionCompleted(payload) => {
            assert_eq!(payload.nodes_completed, 5);
            assert_eq!(payload.nodes_failed, 0);
        }
        _ => panic!("Expected SessionCompleted variant"),
    }
}

#[test]
fn test_session_outcome_serde_roundtrip() {
    for outcome in &[
        SessionOutcome::Completed,
        SessionOutcome::Failed,
        SessionOutcome::Cancelled,
        SessionOutcome::TimedOut,
    ] {
        let json = serde_json::to_string(outcome).unwrap();
        let deserialized: SessionOutcome = serde_json::from_str(&json).unwrap();
        assert_eq!(*outcome, deserialized);
    }
}

// -----------------------------------------------------------------------
// CliConfig serialization tests
// -----------------------------------------------------------------------

#[test]
fn test_cli_config_serde_roundtrip() {
    let config = CliConfig {
        output_format: OutputFormat::Json,
        tui_enabled: false,
        color: ColorMode::Never,
        log_level: LogLevel::Debug,
        log_format: LogFormat::Json,
        config_path: Some("/tmp/rigorix.toml".into()),
        force_tui: false,
        api_key_configured: false,
    };

    let json = serde_json::to_string(&config).unwrap();
    let deserialized: CliConfig = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.output_format, OutputFormat::Json);
    assert!(!deserialized.tui_enabled);
    assert_eq!(deserialized.color, ColorMode::Never);
    assert_eq!(deserialized.log_level, LogLevel::Debug);
}

#[test]
fn test_color_mode_serde_roundtrip() {
    assert_eq!(
        serde_json::from_str::<ColorMode>("\"auto\"").unwrap(),
        ColorMode::Auto
    );
    assert_eq!(
        serde_json::from_str::<ColorMode>("\"always\"").unwrap(),
        ColorMode::Always
    );
    assert_eq!(
        serde_json::from_str::<ColorMode>("\"never\"").unwrap(),
        ColorMode::Never
    );
}
