//! Output formatter — renders `DispatchResult` in the selected format.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#output-formats
//! Implements: OutputFormatter component: LogFormatter trait and types
//! Issue: issue-outputformatter

use std::fmt;

use serde_json::Value as JsonValue;

use crate::cli_boundary::cli::Format;
use crate::cli_boundary::dispatch::DispatchResult;

// ---------------------------------------------------------------------------
// LogFormatter trait
// ---------------------------------------------------------------------------

/// Output formatter that renders `DispatchResult` into a specific format.
pub trait LogFormatter: fmt::Debug + Send + Sync {
    /// Format the summary text of a dispatch result.
    fn format_summary(&self, result: &DispatchResult) -> String;

    /// Format a single structured data item.
    fn format_item(&self, label: &str, data: &JsonValue) -> String;

    /// Format a list of structured data items.
    fn format_list(&self, title: &str, items: &[JsonValue]) -> String;

    /// Format an error result.
    fn format_error(&self, result: &DispatchResult) -> String;
}

// ---------------------------------------------------------------------------
// Formatter implementations
// ---------------------------------------------------------------------------

/// Pretty (human-readable) formatter with Unicode symbols.
#[derive(Debug)]
pub struct PrettyFormatter;

impl LogFormatter for PrettyFormatter {
    fn format_summary(&self, result: &DispatchResult) -> String {
        if result.is_success() {
            result.summary.clone()
        } else {
            format!("✗ {}", result.summary)
        }
    }

    fn format_item(&self, label: &str, data: &JsonValue) -> String {
        format!("  {}: {}", label, data)
    }

    fn format_list(&self, _title: &str, items: &[JsonValue]) -> String {
        items
            .iter()
            .map(|item| format!("• {}", item))
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn format_error(&self, result: &DispatchResult) -> String {
        format!("Error: {} (exit code {})", result.summary, result.exit_code)
    }
}

/// JSON formatter for CI/CD integration.
#[derive(Debug)]
pub struct JsonFormatter;

impl LogFormatter for JsonFormatter {
    fn format_summary(&self, result: &DispatchResult) -> String {
        let value = serde_json::json!({
            "success": result.is_success(),
            "summary": result.summary,
            "exit_code": result.exit_code,
            "data": result.data,
        });
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| result.summary.clone())
    }

    fn format_item(&self, _label: &str, data: &JsonValue) -> String {
        serde_json::to_string_pretty(data).unwrap_or_default()
    }

    fn format_list(&self, _title: &str, items: &[JsonValue]) -> String {
        serde_json::to_string_pretty(items).unwrap_or_default()
    }

    fn format_error(&self, result: &DispatchResult) -> String {
        let value = serde_json::json!({
            "success": false,
            "error": result.summary,
            "exit_code": result.exit_code,
        });
        serde_json::to_string_pretty(&value).unwrap_or_else(|_| result.summary.clone())
    }
}

/// Markdown formatter for documentation output.
#[derive(Debug)]
pub struct MarkdownFormatter;

impl LogFormatter for MarkdownFormatter {
    fn format_summary(&self, result: &DispatchResult) -> String {
        if result.is_success() {
            format!("✅ **{}**", result.summary)
        } else {
            format!("❌ **{}**", result.summary)
        }
    }

    fn format_item(&self, label: &str, data: &JsonValue) -> String {
        format!("### {}\n\n```json\n{}\n```", label, data)
    }

    fn format_list(&self, title: &str, items: &[JsonValue]) -> String {
        let mut output = format!("## {}\n\n", title);
        for item in items {
            output.push_str(&format!("- {}\n", item));
        }
        output
    }

    fn format_error(&self, result: &DispatchResult) -> String {
        format!(
            "> **Error:** {}  \n> Exit code: `{}`",
            result.summary, result.exit_code
        )
    }
}

/// Quiet formatter — minimal output, exit codes only.
#[derive(Debug)]
pub struct QuietFormatter;

impl LogFormatter for QuietFormatter {
    fn format_summary(&self, _result: &DispatchResult) -> String {
        String::new()
    }

    fn format_item(&self, _label: &str, _data: &JsonValue) -> String {
        String::new()
    }

    fn format_list(&self, _title: &str, _items: &[JsonValue]) -> String {
        String::new()
    }

    fn format_error(&self, result: &DispatchResult) -> String {
        format!("Error: {}", result.summary)
    }
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Resolve a `Format` to its `LogFormatter` implementation.
pub fn formatter_for(format: Format) -> Box<dyn LogFormatter> {
    match format {
        Format::Pretty => Box::new(PrettyFormatter),
        Format::Json => Box::new(JsonFormatter),
        Format::Markdown => Box::new(MarkdownFormatter),
        Format::Quiet => Box::new(QuietFormatter),
    }
}

/// Format a `DispatchResult` and exit the process with the appropriate code.
///
/// This is the single entry point for all command output. It:
/// 1. Selects the formatter (defaults to Pretty)
/// 2. Renders the output to stdout (success) or stderr (error)
/// 3. Calls `std::process::exit()` with the result's exit code
///
/// # Panics
///
/// This function never returns — it always calls `process::exit()`.
pub fn format_and_exit(result: DispatchResult) -> ! {
    let formatter = formatter_for(Format::Pretty);
    if result.is_success() {
        let summary = formatter.format_summary(&result);
        if result.data.is_some() && summary.is_empty() {
            // Only show data if there's no summary
            if let Some(ref data) = result.data {
                println!("{}", formatter.format_item("Details", data));
            }
        } else {
            println!("{}", summary);
        }
    } else {
        eprintln!("{}", formatter.format_error(&result));
    }
    std::process::exit(result.exit_code);
}
