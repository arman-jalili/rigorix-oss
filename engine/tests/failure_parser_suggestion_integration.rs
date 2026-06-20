//! Integration tests for FixSuggestionServiceImpl — suggested fix generation.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#suggested-fix-generation
//! Implements: FixSuggestionService integration tests
//! Issue: #499
//!
//! Tests fix suggestion generation across all TemplateFailure variants
//! with various source context configurations.

use rigorix_engine::failure_parser::application::{FixSuggestionService, FixSuggestionServiceImpl};
use rigorix_engine::failure_parser::domain::{SourceContext, SourceLocation, TemplateFailure};

fn ctx_with_symbols(file: &str, symbols: &[&str]) -> SourceContext {
    let mut ctx = SourceContext::empty();
    ctx.symbols_by_file.insert(
        file.to_string(),
        symbols.iter().map(|s| s.to_string()).collect(),
    );
    ctx
}

fn ctx_with_multiple_files() -> SourceContext {
    let mut ctx = SourceContext::empty();
    ctx.symbols_by_file.insert(
        "src/tasklist.ts".to_string(),
        vec![
            "add".to_string(),
            "remove".to_string(),
            "list".to_string(),
            "complete".to_string(),
            "count".to_string(),
        ],
    );
    ctx.symbols_by_file.insert(
        "src/user.ts".to_string(),
        vec![
            "User".to_string(),
            "createUser".to_string(),
            "deleteUser".to_string(),
        ],
    );
    ctx
}

// ---------------------------------------------------------------------------
// MissingSymbol — comprehensive matching strategies
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_missing_symbol_substring_match() {
    let ctx = ctx_with_symbols("src/app.ts", &["add", "list", "complete"]);
    let failure = TemplateFailure::MissingSymbol {
        symbol: "addTask".into(),
        available: vec![],
        suggestion: None,
        location: SourceLocation::new("src/app.ts", 10, Some(5)),
    };
    let suggestion = FixSuggestionServiceImpl
        .suggest_fix(&failure, &ctx)
        .await
        .unwrap();
    assert!(
        suggestion.is_some(),
        "Should find 'add' via substring match"
    );
    assert!(suggestion.unwrap().contains("add"));
}

#[tokio::test]
async fn integration_missing_symbol_case_insensitive() {
    let ctx = ctx_with_symbols("src/app.ts", &["AddItem", "RemoveItem"]);
    let failure = TemplateFailure::MissingSymbol {
        symbol: "addItem".into(),
        available: vec![],
        suggestion: None,
        location: SourceLocation::new("src/app.ts", 10, Some(5)),
    };
    let suggestion = FixSuggestionServiceImpl
        .suggest_fix(&failure, &ctx)
        .await
        .unwrap();
    assert!(suggestion.is_some(), "Should find case-insensitive match");
}

#[tokio::test]
async fn integration_missing_symbol_cross_file_search() {
    let ctx = ctx_with_multiple_files();
    let failure = TemplateFailure::MissingSymbol {
        symbol: "addTask".into(),
        available: vec![],
        suggestion: None,
        location: SourceLocation::new("src/user.ts", 5, Some(3)), // symbols in different file
    };
    let suggestion = FixSuggestionServiceImpl
        .suggest_fix(&failure, &ctx)
        .await
        .unwrap();
    // Should find "add" from tasklist.ts even though we're in user.ts
    assert!(suggestion.is_some());
}

#[tokio::test]
async fn integration_missing_symbol_available_symbols_listed() {
    let ctx = SourceContext::empty();
    let failure = TemplateFailure::MissingSymbol {
        // Use a symbol that doesn't match via substring with any available symbol
        symbol: "totallyUnrelated".into(),
        available: vec!["knownFunc".into(), "anotherFunc".into()],
        suggestion: None,
        location: SourceLocation::new("src/app.ts", 1, Some(1)),
    };
    let suggestion = FixSuggestionServiceImpl
        .suggest_fix(&failure, &ctx)
        .await
        .unwrap();
    let sug = suggestion.unwrap();
    assert!(
        sug.contains("Available symbols"),
        "Should list available symbols, got: {}",
        sug
    );
    assert!(sug.contains("knownFunc"));
}

#[tokio::test]
async fn integration_missing_symbol_no_hints() {
    let ctx = SourceContext::empty();
    let failure = TemplateFailure::MissingSymbol {
        symbol: "completelyUnknown".into(),
        available: vec![],
        suggestion: None,
        location: SourceLocation::new("src/app.ts", 1, Some(1)),
    };
    let suggestion = FixSuggestionServiceImpl
        .suggest_fix(&failure, &ctx)
        .await
        .unwrap();
    assert!(suggestion.is_none(), "No context → no suggestion");
}

// ---------------------------------------------------------------------------
// WrongArgCount — singular/plural, missing/extra
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_wrong_arg_count_missing_single() {
    let failure = TemplateFailure::WrongArgCount {
        function: "greet".into(),
        expected: 1,
        actual: 0,
        location: SourceLocation::new("src/app.ts", 5, None),
    };
    let suggestion = FixSuggestionServiceImpl
        .suggest_fix(&failure, &SourceContext::empty())
        .await
        .unwrap();
    assert!(suggestion.is_some());
    let sug = suggestion.unwrap();
    assert!(sug.contains("greet"));
    assert!(sug.contains("missing 1 argument")); // singular
}

#[tokio::test]
async fn integration_wrong_arg_count_extra_multiple() {
    let failure = TemplateFailure::WrongArgCount {
        function: "add".into(),
        expected: 2,
        actual: 5,
        location: SourceLocation::new("src/app.ts", 5, None),
    };
    let suggestion = FixSuggestionServiceImpl
        .suggest_fix(&failure, &SourceContext::empty())
        .await
        .unwrap();
    let sug = suggestion.unwrap();
    assert!(sug.contains("Remove 3 extra arguments")); // plural
}

// ---------------------------------------------------------------------------
// TypeMismatch — context-aware suggestions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_type_mismatch_type_in_file() {
    let ctx = ctx_with_symbols("src/app.ts", &["User", "string", "number"]);
    let failure = TemplateFailure::TypeMismatch {
        expected: "User".into(),
        actual: "string".into(),
        location: SourceLocation::new("src/app.ts", 15, Some(3)),
    };
    let suggestion = FixSuggestionServiceImpl
        .suggest_fix(&failure, &ctx)
        .await
        .unwrap();
    let sug = suggestion.unwrap();
    assert!(sug.contains("User"));
    assert!(sug.contains("is defined in this file"));
}

#[tokio::test]
async fn integration_type_mismatch_type_not_in_project() {
    let ctx = SourceContext::empty();
    let failure = TemplateFailure::TypeMismatch {
        expected: "ExternalType".into(),
        actual: "string".into(),
        location: SourceLocation::new("src/app.ts", 15, Some(3)),
    };
    let suggestion = FixSuggestionServiceImpl
        .suggest_fix(&failure, &ctx)
        .await
        .unwrap();
    let sug = suggestion.unwrap();
    assert!(sug.contains("ExternalType"));
    assert!(sug.contains("type cast"));
}

// ---------------------------------------------------------------------------
// CompileError — code-specific guidance
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_compile_error_ts2307_module_not_found() {
    let failure = TemplateFailure::CompileError {
        code: "TS2307".into(),
        message: "Cannot find module 'lodash' or its corresponding type declarations.".into(),
        location: SourceLocation::new("src/app.ts", 1, Some(1)),
    };
    let suggestion = FixSuggestionServiceImpl
        .suggest_fix(&failure, &SourceContext::empty())
        .await
        .unwrap();
    let sug = suggestion.unwrap();
    assert!(sug.contains("Cannot find module"));
}

#[tokio::test]
async fn integration_compile_error_generic() {
    let failure = TemplateFailure::CompileError {
        code: "TS9999".into(),
        message: "Some exotic error.".into(),
        location: SourceLocation::new("src/app.ts", 10, Some(1)),
    };
    let suggestion = FixSuggestionServiceImpl
        .suggest_fix(&failure, &SourceContext::empty())
        .await
        .unwrap();
    assert!(suggestion.is_some());
}

// ---------------------------------------------------------------------------
// Batch suggestions
// ---------------------------------------------------------------------------

#[tokio::test]
async fn integration_batch_mixed_types() {
    let ctx = ctx_with_multiple_files();
    let failures = vec![
        TemplateFailure::MissingSymbol {
            symbol: "addTask".into(),
            available: vec![],
            suggestion: None,
            location: SourceLocation::new("src/tasklist.ts", 3, Some(10)),
        },
        TemplateFailure::WrongArgCount {
            function: "list".into(),
            expected: 0,
            actual: 1,
            location: SourceLocation::new("src/tasklist.ts", 5, None),
        },
        TemplateFailure::TypeMismatch {
            expected: "Task".into(),
            actual: "string".into(),
            location: SourceLocation::new("src/tasklist.ts", 15, Some(3)),
        },
    ];

    let results = FixSuggestionServiceImpl
        .suggest_fixes_batch(&failures, &ctx)
        .await
        .unwrap();

    assert_eq!(results.len(), 3);
    for (i, (_, sug)) in results.iter().enumerate() {
        assert!(sug.is_some(), "Failure {} should have a suggestion", i);
    }
}

#[tokio::test]
async fn integration_batch_empty() {
    let results = FixSuggestionServiceImpl
        .suggest_fixes_batch(&[], &SourceContext::empty())
        .await
        .unwrap();
    assert!(results.is_empty());
}
