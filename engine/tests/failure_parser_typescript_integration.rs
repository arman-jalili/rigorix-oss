//! Integration tests for TypeScriptParser — parse `tsc --noEmit --pretty false` output.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#ts-parser
//! Implements: TypeScriptParser integration tests
//! Issue: #498
//!
//! Tests the TypeScript parser with real-world tsc output formats,
//! including multi-line output, mixed error types, and edge cases.

use rigorix_engine::failure_parser::application::TypeScriptParser;
use rigorix_engine::failure_parser::domain::{LanguageParser, SourceContext, TemplateFailure};

fn empty_context() -> SourceContext {
    SourceContext::empty()
}

// ---------------------------------------------------------------------------
// Standard tsc --pretty false error formats
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_tsc_standard_format_missing_symbol() {
    let output = "src/app.ts(15,8): error TS2339: Property 'foo' does not exist on type 'Bar'.";
    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 1);
    assert_eq!(result.failures[0].failure.variant_name(), "missing_symbol");
}

#[tokio::test]
async fn integration_tsc_standard_format_wrong_arg_count() {
    let output = "src/utils.ts(42,5): error TS2554: Expected 3 arguments, but got 1.";
    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 1);
    assert_eq!(result.failures[0].failure.variant_name(), "wrong_arg_count");
}

#[tokio::test]
async fn integration_tsc_standard_format_type_mismatch() {
    let output = "src/components/Button.tsx(25,12): error TS2345: Argument of type 'number' is not assignable to parameter of type 'string'.";
    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 1);
    assert_eq!(result.failures[0].failure.variant_name(), "type_mismatch");
}

// ---------------------------------------------------------------------------
// Real-world multi-error outputs
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_tsc_multiple_errors_mixed_types() {
    let output = "\
src/auth.ts(10,3): error TS2339: Property 'token' does not exist on type 'User'.
src/auth.ts(25,1): error TS1005: ';' expected.
src/api.ts(5,10): error TS2345: Argument of type 'string' is not assignable to parameter of type 'number'.";

    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 3);
    assert_eq!(result.failures[0].failure.variant_name(), "missing_symbol");
    assert_eq!(result.failures[1].failure.variant_name(), "compile_error");
    assert_eq!(result.failures[2].failure.variant_name(), "type_mismatch");
}

#[tokio::test]
async fn integration_tsc_5_errors_batch() {
    let output = "\
src/a.ts(1,1): error TS2339: Property 'x' not found.
src/a.ts(2,1): error TS2339: Property 'y' not found.
src/a.ts(3,1): error TS2554: Expected 1 argument, but got 2.
src/b.ts(1,1): error TS2345: Type 'X' not assignable to type 'Y'.
src/b.ts(2,1): error TS1005: ',' expected.";

    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 5);
    assert_eq!(result.total_count, 5);
    assert_eq!(result.source_tool, "tsc");
}

// ---------------------------------------------------------------------------
// Edge cases
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_tsc_error_with_did_you_mean_suggestion() {
    let output = "src/tasklist.ts(5,10): error TS2551: Property 'addd' does not exist on type 'TaskList'. Did you mean 'add'?";
    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 1);
    if let TemplateFailure::MissingSymbol { suggestion, .. } = &result.failures[0].failure {
        assert!(
            suggestion.as_ref().map_or(false, |s| s.contains("add")),
            "Expected suggestion mentioning 'add', got: {:?}",
            suggestion
        );
    } else {
        panic!("Expected MissingSymbol with suggestion");
    }
}

#[tokio::test]
async fn integration_tsc_error_with_dash_separator() {
    // Some tsc versions output hyphen instead of colon before message
    let output = "src/file.ts(1,1): error TS2339 - Property 'x' not found.";
    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 1);
    assert_eq!(result.failures[0].failure.variant_name(), "missing_symbol");
}

#[tokio::test]
async fn integration_tsc_no_newline_at_end() {
    let output = "src/test.ts(1,1): error TS2339: Property 'x' does not exist on type 'Y'.";
    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 1);
}

#[tokio::test]
async fn integration_tsc_output_has_bom_or_whitespace_prefix() {
    let output = "  src/test.ts(1,1): error TS2339: Property 'x' does not exist.";
    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 1);
}

// ---------------------------------------------------------------------------
// Empty/clean outputs
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_tsc_clean_empty() {
    let result = TypeScriptParser.parse("", &empty_context()).await.unwrap();
    assert!(result.is_clean());
}

#[tokio::test]
async fn integration_tsc_clean_success_message() {
    let output = "\
src/index.ts(1,1): error TS2339: Property 'x' does not exist.

Found 1 error. Watching for file changes.";
    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 1);
}

// ---------------------------------------------------------------------------
// Error classifications
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_tsc_all_missing_symbol_variants() {
    let output = "\
src/a.ts(1,1): error TS2339: Property 'x' does not exist.
src/b.ts(1,1): error TS2304: Cannot find name 'y'.
src/c.ts(1,1): error TS2551: Property 'z' does not exist. Did you mean 'a'?";

    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 3);
    for detail in &result.failures {
        assert_eq!(
            detail.failure.variant_name(),
            "missing_symbol",
            "All three should be missing_symbol, got: {}",
            detail.failure.variant_name()
        );
    }
}

#[tokio::test]
async fn integration_tsc_compile_errors() {
    let output = "\
src/a.ts(1,1): error TS1005: ';' expected.
src/b.ts(1,1): error TS1109: Expression expected.";

    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 2);
    for detail in &result.failures {
        assert_eq!(detail.failure.variant_name(), "compile_error");
    }
}

#[tokio::test]
async fn integration_tsc_error_with_no_location_format() {
    // Some tsc errors appear without file/line info
    let output = "error TS6203: Project may not use both 'out' and 'outDir'.";
    let result = TypeScriptParser
        .parse(output, &empty_context())
        .await
        .unwrap();
    assert_eq!(result.total_count, 1);
    assert_eq!(result.failures[0].failure.variant_name(), "compile_error");
}
