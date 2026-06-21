//! Integration tests for TemplateFailure — typed classification of execution failures.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#failure
//! Implements: TemplateFailure integration tests
//! Issue: #496
//!
//! Tests the full lifecycle of TemplateFailure:
//! - Construction from error scenarios
//! - Serialization/deserialization roundtrips
//! - SourceLocation formatting
//! - FailureDetail creation
//! - ParsedFailure aggregation
//! - TemplateFailureService operations
//!
//! These are integration tests that exercise the public API surface
//! as consumers would use it.

use rigorix_engine::failure_parser::domain::{
    FailureDetail, FailureSeverity, ParsedFailure, SourceContext, SourceLocation, TemplateFailure,
    TemplateFailureService,
};

// ---------------------------------------------------------------------------
// Helper: create a sample MissingSymbol failure like tsc TS2339
// ---------------------------------------------------------------------------

fn sample_missing_symbol_failure() -> TemplateFailure {
    TemplateFailure::MissingSymbol {
        symbol: "addTask".to_string(),
        available: vec![
            "add".to_string(),
            "list".to_string(),
            "complete".to_string(),
        ],
        suggestion: Some("Use 'add' instead of 'addTask'".to_string()),
        location: SourceLocation::new("tests/tasklist.test.ts", 3, Some(10)),
    }
}

fn sample_wrong_arg_count_failure() -> TemplateFailure {
    TemplateFailure::WrongArgCount {
        function: "add".to_string(),
        expected: 2,
        actual: 3,
        location: SourceLocation::new("tests/tasklist.test.ts", 15, Some(5)),
    }
}

fn sample_type_mismatch_failure() -> TemplateFailure {
    TemplateFailure::TypeMismatch {
        expected: "Task".to_string(),
        actual: "string".to_string(),
        location: SourceLocation::new("tests/tasklist.test.ts", 20, Some(1)),
    }
}

fn sample_compile_error_failure() -> TemplateFailure {
    TemplateFailure::CompileError {
        code: "TS2339".to_string(),
        message: "Property 'addTask' does not exist on type 'TaskList'.".to_string(),
        location: SourceLocation::new("tests/tasklist.test.ts", 3, Some(10)),
    }
}

fn sample_assertion_failure() -> TemplateFailure {
    TemplateFailure::AssertionFailure {
        test_name: "should add a new task".to_string(),
        expected: "5".to_string(),
        received: "3".to_string(),
        location: SourceLocation::new("tests/tasklist.test.ts", 42, Some(1)),
    }
}

fn sample_test_failure() -> TemplateFailure {
    TemplateFailure::TestFailure {
        test_name: "should complete task".to_string(),
        message: "TimeoutError: test exceeded 5000ms".to_string(),
        location: None,
    }
}

// ---------------------------------------------------------------------------
// Tests: TemplateFailure Construction & Accessors
// ---------------------------------------------------------------------------

#[test]
fn integration_template_failure_all_variants() {
    let failures = [
        sample_missing_symbol_failure(),
        sample_wrong_arg_count_failure(),
        sample_type_mismatch_failure(),
        sample_compile_error_failure(),
        sample_assertion_failure(),
        sample_test_failure(),
    ];

    assert_eq!(failures.len(), 6);
    assert_eq!(failures[0].variant_name(), "missing_symbol");
    assert_eq!(failures[1].variant_name(), "wrong_arg_count");
    assert_eq!(failures[2].variant_name(), "type_mismatch");
    assert_eq!(failures[3].variant_name(), "compile_error");
    assert_eq!(failures[4].variant_name(), "assertion_failure");
    assert_eq!(failures[5].variant_name(), "test_failure");
}

#[test]
fn integration_template_failure_summary_formats() {
    let f = sample_missing_symbol_failure();
    let s = f.summary();
    assert!(
        s.contains("MissingSymbol"),
        "Summary should contain variant name: {}",
        s
    );
    assert!(
        s.contains("addTask"),
        "Summary should contain symbol: {}",
        s
    );
    assert!(
        s.contains("tests/tasklist.test.ts"),
        "Summary should contain file: {}",
        s
    );
}

#[test]
fn integration_template_failure_fixable_categories() {
    assert!(sample_missing_symbol_failure().is_fixable());
    assert!(sample_wrong_arg_count_failure().is_fixable());
    assert!(sample_type_mismatch_failure().is_fixable());
    assert!(!sample_compile_error_failure().is_fixable());
    assert!(!sample_assertion_failure().is_fixable());
    assert!(!sample_test_failure().is_fixable());
}

// ---------------------------------------------------------------------------
// Tests: Serialization Roundtrips
// ---------------------------------------------------------------------------

#[test]
fn integration_serialization_json_roundtrip_all_variants() {
    let variants = [
        sample_missing_symbol_failure(),
        sample_wrong_arg_count_failure(),
        sample_type_mismatch_failure(),
        sample_compile_error_failure(),
        sample_assertion_failure(),
        sample_test_failure(),
    ];

    for (i, variant) in variants.iter().enumerate() {
        let json = serde_json::to_string_pretty(variant)
            .unwrap_or_else(|e| panic!("Variant {}: serialization failed: {}", i, e));
        let deserialized: TemplateFailure = serde_json::from_str(&json).unwrap_or_else(|e| {
            panic!(
                "Variant {}: deserialization failed: {} from JSON: {}",
                i,
                e,
                &json[..200]
            )
        });
        assert_eq!(
            *variant, deserialized,
            "Variant {}: roundtrip equality failed",
            i
        );
    }
}

#[test]
fn integration_serialization_json_serde_tag() {
    // Verify that serde tag works and variant names are correct
    let f = sample_missing_symbol_failure();
    let json = serde_json::to_value(&f).unwrap();
    assert_eq!(json["type"], "missing_symbol");
}

// ---------------------------------------------------------------------------
// Tests: SourceLocation
// ---------------------------------------------------------------------------

#[test]
fn integration_source_location_compact() {
    let loc = SourceLocation::new("src/lib.ts", 10, Some(3));
    assert_eq!(loc.to_compact(), "src/lib.ts:10:3");

    let loc = SourceLocation::new("src/lib.ts", 10, None);
    assert_eq!(loc.to_compact(), "src/lib.ts:10");
}

#[test]
fn integration_source_location_new() {
    let loc = SourceLocation::new("test.ts", 1, Some(1));
    assert_eq!(loc.file, "test.ts");
    assert_eq!(loc.line, 1);
    assert_eq!(loc.column, Some(1));
}

// ---------------------------------------------------------------------------
// Tests: FailureDetail
// ---------------------------------------------------------------------------

#[test]
fn integration_failure_detail_location_extraction() {
    let detail = FailureDetail::new(
        sample_missing_symbol_failure(),
        Some("Use 'add'".to_string()),
        FailureSeverity::CompileBlock,
        "error TS2339: Property 'addTask' does not exist".to_string(),
        "tsc",
        0.95,
    );
    assert_eq!(detail.source_tool, "tsc");
    assert_eq!(detail.severity, FailureSeverity::CompileBlock);
    assert_eq!(detail.suggested_fix, Some("Use 'add'".to_string()));
    assert!((detail.confidence - 0.95).abs() < 1e-10);

    let loc = detail.location().unwrap();
    assert_eq!(loc.file, "tests/tasklist.test.ts");
    assert_eq!(loc.line, 3);
}

#[test]
fn integration_failure_detail_test_failure_no_location() {
    let detail = FailureDetail::new(
        sample_test_failure(),
        None,
        FailureSeverity::TestBlock,
        "FAIL should complete task".to_string(),
        "jest",
        1.0,
    );
    assert_eq!(detail.location(), None);
}

#[test]
fn integration_failure_detail_to_log_line() {
    let detail = FailureDetail::new(
        sample_missing_symbol_failure(),
        Some("Use 'add'".to_string()),
        FailureSeverity::CompileBlock,
        "error: TS2339".to_string(),
        "tsc",
        0.95,
    );
    let log = detail.to_log_line();
    assert!(log.contains("tsc"), "Log should contain tool name");
    assert!(
        log.contains("tests/tasklist.test.ts:3:10"),
        "Log should contain location"
    );
    assert!(
        log.contains("Use 'add'"),
        "Log should contain fix suggestion"
    );
}

// ---------------------------------------------------------------------------
// Tests: ParsedFailure
// ---------------------------------------------------------------------------

#[test]
fn integration_parsed_failure_from_details() {
    let failures = vec![
        FailureDetail::new(
            sample_missing_symbol_failure(),
            Some("Use 'add'".to_string()),
            FailureSeverity::CompileBlock,
            "error: TS2339".to_string(),
            "tsc",
            0.95,
        ),
        FailureDetail::new(
            sample_test_failure(),
            None,
            FailureSeverity::TestBlock,
            "FAIL".to_string(),
            "jest",
            1.0,
        ),
    ];

    let parsed = ParsedFailure::from_failures(failures, "tsc");
    assert_eq!(parsed.total_count, 2);
    assert_eq!(parsed.fixable_count, 1);
    assert_eq!(parsed.source_tool, "tsc");
    assert!(!parsed.is_clean());
    assert_eq!(parsed.overall_severity, FailureSeverity::CompileBlock);
}

#[test]
fn integration_parsed_failure_empty() {
    let parsed = ParsedFailure::from_failures(vec![], "tsc");
    assert!(parsed.is_clean());
    // all_fixable requires total_count > 0, so empty returns false
    assert!(!parsed.all_fixable());
    assert_eq!(parsed.total_count, 0);
    assert_eq!(parsed.fixable_count, 0);
}

#[test]
fn integration_parsed_failure_all_fixable() {
    let failures = vec![
        FailureDetail::new(
            sample_missing_symbol_failure(),
            Some("Fix 1".to_string()),
            FailureSeverity::CompileBlock,
            "e1".to_string(),
            "tsc",
            1.0,
        ),
        FailureDetail::new(
            sample_wrong_arg_count_failure(),
            Some("Fix 2".to_string()),
            FailureSeverity::CompileBlock,
            "e2".to_string(),
            "tsc",
            1.0,
        ),
    ];
    let parsed = ParsedFailure::from_failures(failures, "tsc");
    assert!(parsed.all_fixable());
}

// ---------------------------------------------------------------------------
// Tests: SourceContext
// ---------------------------------------------------------------------------

#[test]
fn integration_source_context_with_multiple_files() {
    let mut ctx = SourceContext::empty();
    ctx.symbols_by_file.insert(
        "src/tasklist.ts".to_string(),
        vec!["add".to_string(), "remove".to_string(), "list".to_string()],
    );
    ctx.source_by_file.insert(
        "src/tasklist.ts".to_string(),
        "export class TaskList { ... }".to_string(),
    );

    assert!(ctx.has_content());
    let symbols = ctx.symbols_in_file("src/tasklist.ts");
    assert_eq!(symbols.len(), 3);
    assert!(symbols.contains(&"add".to_string()));
    assert_eq!(
        ctx.source_for_file("src/tasklist.ts"),
        Some("export class TaskList { ... }")
    );
}

// ---------------------------------------------------------------------------
// Tests: TemplateFailureService
// ---------------------------------------------------------------------------

#[test]
fn integration_service_group_by_variant() {
    let failures = vec![
        sample_missing_symbol_failure(),
        sample_compile_error_failure(),
        sample_missing_symbol_failure(),
    ];
    let groups = TemplateFailureService::group_by_variant(&failures);
    assert_eq!(groups.len(), 2);
    assert_eq!(groups.get("missing_symbol").unwrap().len(), 2);
    assert_eq!(groups.get("compile_error").unwrap().len(), 1);
}

#[test]
fn integration_service_to_parsed_full_flow() {
    let failures = vec![sample_missing_symbol_failure(), sample_test_failure()];

    let parsed = TemplateFailureService::to_parsed(failures, "tsc");
    assert_eq!(parsed.total_count, 2);
    assert!(!parsed.is_clean());

    // Verify the details have proper severity
    // First is compile block (missing symbol), second is test block
    assert_eq!(parsed.failures[0].severity, FailureSeverity::CompileBlock);
    assert_eq!(parsed.failures[1].severity, FailureSeverity::TestBlock);
}

#[test]
fn integration_service_summary_multiple_types() {
    let failures = vec![
        sample_missing_symbol_failure(),
        sample_compile_error_failure(),
        sample_assertion_failure(),
        sample_test_failure(),
    ];
    let summary = TemplateFailureService::summary(&failures);
    assert!(summary.contains("4 errors"));
    assert!(summary.contains("1 fixable"));
    assert!(summary.contains("3 non-fixable"));
}

#[test]
fn integration_service_error_codes() {
    let failures = vec![
        sample_compile_error_failure(),
        TemplateFailure::CompileError {
            code: "TS2554".to_string(),
            message: "Expected 2 arguments".to_string(),
            location: SourceLocation::new("test.ts", 1, None),
        },
        sample_missing_symbol_failure(),
    ];
    let codes = TemplateFailureService::error_codes(&failures);
    assert_eq!(codes.len(), 2);
    assert!(codes.contains("TS2339"));
    assert!(codes.contains("TS2554"));
}

#[test]
fn integration_service_test_names() {
    let failures = vec![sample_assertion_failure(), sample_test_failure()];
    let names = TemplateFailureService::test_names(&failures);
    assert_eq!(names.len(), 2);
    assert!(names.contains("should add a new task"));
    assert!(names.contains("should complete task"));
}
