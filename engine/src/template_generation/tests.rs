//! Template Generation module tests.
//!
//! @canonical .pi/architecture/modules/template-generation.md#tests
//! Issue: issue-contract-freeze

use std::path::PathBuf;

use crate::template_generation::domain::{
    ClaudeGeneratorConfig, ClaudeTemplateGenerator, GeneratedTemplate, GeneratedTemplateCost,
    GeneratorError, InvalidSymbolReference, RepoContext, TemplateGenerator,
};

#[test]
fn test_strip_code_fences_no_fences() {
    let input = "simple toml content";
    assert_eq!(
        ClaudeTemplateGenerator::strip_code_fences(input),
        "simple toml content"
    );
}

#[test]
fn test_strip_code_fences_with_language() {
    let input = "```toml\nname = \"test\"\n```";
    assert_eq!(
        ClaudeTemplateGenerator::strip_code_fences(input),
        "name = \"test\""
    );
}

#[test]
fn test_strip_code_fences_no_language() {
    let input = "```\nname = \"test\"\n```";
    assert_eq!(
        ClaudeTemplateGenerator::strip_code_fences(input),
        "name = \"test\""
    );
}

#[test]
fn test_strip_code_fences_only_closing() {
    let input = "name = \"test\"\n```";
    assert_eq!(
        ClaudeTemplateGenerator::strip_code_fences(input),
        "name = \"test\""
    );
}

#[test]
fn test_strip_code_fences_whitespace() {
    let input = "\n  ```toml\n  name = \"test\"\n  ```  \n";
    assert_eq!(
        ClaudeTemplateGenerator::strip_code_fences(input),
        "name = \"test\""
    );
}

#[test]
fn test_generator_error_display_invalid_toml() {
    let err = GeneratorError::InvalidToml {
        raw_response: "{{{bad toml".to_string(),
        parse_error: "expected a value".to_string(),
        attempt: 0,
    };
    let display = format!("{}", err);
    assert!(display.contains("Invalid TOML"));
}

#[test]
fn test_generator_error_display_budget_exhausted() {
    let err = GeneratorError::BudgetExhausted {
        calls_used: 5,
        max_calls: 3,
    };
    let display = format!("{}", err);
    assert!(display.contains("Budget exhausted"));
}

#[test]
fn test_repo_context_default() {
    let ctx = RepoContext::new(PathBuf::from("/test"), "rust".to_string());
    assert_eq!(ctx.project_type, "rust");
    assert!(!ctx.has_files());
}

#[test]
fn test_repo_context_with_files() {
    let mut ctx = RepoContext::new(PathBuf::from("/test"), "rust".to_string());
    ctx.directory_tree = vec!["src/main.rs".to_string()];
    assert!(ctx.has_files());
}

#[test]
fn test_claude_config_defaults() {
    let config = ClaudeGeneratorConfig::default();
    assert_eq!(config.model, "claude-sonnet-4-20250514");
    assert_eq!(config.max_retries, 3);
    assert_eq!(config.max_tokens, 4096);
    assert_eq!(config.temperature, 0.3);
}

#[test]
fn test_generated_template_serde() {
    let t = GeneratedTemplate {
        toml_content: "id = \"test\"".to_string(),
        suggested_id: "test".to_string(),
        suggested_name: "Test".to_string(),
        description: "A test".to_string(),
        llm_calls_used: 1,
        llm_tokens_used: 100,
    };
    let json = serde_json::to_string(&t).unwrap();
    let deserialized: GeneratedTemplate = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized.suggested_id, "test");
    assert_eq!(deserialized.llm_calls_used, 1);
}

#[test]
fn test_generated_template_cost() {
    let cost = GeneratedTemplateCost {
        estimated_calls: 3,
        estimated_tokens: 4096,
    };
    assert_eq!(cost.estimated_calls, 3);
    assert_eq!(cost.estimated_tokens, 4096);
}

#[test]
fn test_invalid_symbol_reference() {
    let ref_ = InvalidSymbolReference {
        symbol: "NonExistentType".to_string(),
        usage: "type_reference".to_string(),
        reason: "Symbol not found in symbol graph".to_string(),
        is_any_type: false,
    };
    assert_eq!(ref_.symbol, "NonExistentType");
    assert!(!ref_.is_any_type);
}

#[test]
fn test_generator_error_display_validation_failed() {
    let err = GeneratorError::ValidationFailed {
        template_id: "test-template".to_string(),
        errors: vec!["missing required field".to_string()],
        attempt: 1,
    };
    let display = format!("{}", err);
    assert!(display.contains("test-template"));
    assert!(display.contains("missing required field"));
}

#[test]
fn test_generator_error_display_symbol_validation() {
    let err = GeneratorError::SymbolValidation {
        template_id: "test".to_string(),
        invalid_references: vec![InvalidSymbolReference {
            symbol: "Foo".to_string(),
            usage: "type".to_string(),
            reason: "not found".to_string(),
            is_any_type: false,
        }],
        attempt: 0,
    };
    let display = format!("{}", err);
    assert!(display.contains("1 invalid references"));
}

#[test]
fn test_generator_error_display_api_error() {
    let err = GeneratorError::ApiError {
        detail: "rate limited".to_string(),
        status_code: Some(429),
        retry_after: Some(30),
    };
    let display = format!("{}", err);
    assert!(display.contains("rate limited"));
}

#[test]
fn test_generator_error_display_max_retries() {
    let err = GeneratorError::MaxRetriesExhausted {
        attempts: 3,
        errors: vec!["parse failed".to_string()],
    };
    let display = format!("{}", err);
    assert!(display.contains("Max retries exhausted"));
}

#[test]
fn test_generator_error_display_context_build_failed() {
    let err = GeneratorError::ContextBuildFailed {
        detail: "no files found".to_string(),
    };
    let display = format!("{}", err);
    assert!(display.contains("Context build failed"));
}

#[test]
fn test_estimate_cost_default() {
    let config = ClaudeGeneratorConfig::default();
    let cost = config.max_retries as u32;
    assert_eq!(cost, 3);
}
