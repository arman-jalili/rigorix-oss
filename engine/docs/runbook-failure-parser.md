# Runbook: failure-parser Module

<!--
Canonical Reference: .pi/architecture/modules/failure-parser.md
Last Updated: 2026-06-19
-->

## Overview

The `failure-parser` module ingests raw compiler, test runner, and linter output and
produces structured, typed `TemplateFailure` values. Each failure carries precise location
context (file, line, column), a machine-readable error code, and a suggested fix derived
from the available source code context.

This is the bridge between "something failed" and "here's exactly what to change." The
parser enables the validation loop to feed actionable feedback back to the LLM for
self-correction.

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| None | — | The module is pure logic with no external dependencies |

### Initialization

1. Module loads automatically as part of the `rigorix` library
2. Configure `ParserRegistry` with desired parsers
3. Create `FailureParserServiceImpl` with the registry
4. Create `FixSuggestionServiceImpl` for fix suggestion generation
5. Register custom parsers via `ParserRegistry::register()` if needed

```rust
use rigorix_engine::failure_parser::application::*;
use rigorix_engine::failure_parser::domain::*;

// Create registry with built-in parsers
let mut registry = ParserRegistry::new();
registry.register(Box::new(TypeScriptParser::new()));

// Create the main service
let service = FailureParserServiceImpl::new(registry);
let fix_service = FixSuggestionServiceImpl::new();
```

### Quick Start

```rust
use rigorix_engine::failure_parser::application::*;
use rigorix_engine::failure_parser::domain::*;

// Parse tsc output
let service = create_parser_service(); // from factory
let result = service.parse(ParseOutputInput {
    tool: "tsc".into(),
    stdout: "src/app.ts(10,5): error TS2339: Property 'x' not found.".into(),
    stderr: String::new(),
    exit_code: 2,
    source_context: SourceContext::empty(),
    working_directory: "/project".into(),
}).await?;

// Get suggested fixes
for detail in &result.parsed.failures {
    if let Some(fix) = &detail.suggested_fix {
        println!("FIX: {}", fix);
    }
}

// Format for LLM consumption
let llm_context = service.format_for_llm(FormatForLlmInput {
    failures: result.parsed.failures.iter().map(|d| d.failure.clone()).collect(),
    title: Some("TSC Analysis".into()),
}).await?;
println!("{}", llm_context.formatted);
```

## Graceful Shutdown

The failure-parser module is stateless and requires no explicit shutdown. When the
runtime drops service instances, the parser registry and all registered parsers are
cleaned up automatically.

### Steps

1. Drop all references to `FailureParserServiceImpl`
2. Drop all references to `FixSuggestionServiceImpl`
3. The `ParserRegistry` will be deallocated along with all registered parser instances

## Common Failure Modes

### 1. No Parser Registered for Tool

**Symptom:** `FailureParserError::UnsupportedTool` returned from `parse()`.

**Cause:** The requested tool is not in the `ParserRegistry`.

**Recovery:**
1. Register the appropriate parser: `registry.register(Box::new(TypeScriptParser::new()))`
2. Verify the tool name matches exactly (e.g., "tsc", "jest", "rustc", "pytest")

**Prevention:**
- Register all expected parsers at startup
- The `ParserRegistry` provides `available_tools()` to enumerate registered parsers

### 2. Unrecognized Output Format

**Symptom:** `FailureParserError::UnrecognizedFormat` from `TypeScriptParser.parse()`.

**Cause:** The output contains "error TS" but doesn't match the expected line format.

**Recovery:**
1. Verify the tool was invoked with `--pretty false` flag (for tsc)
2. Check for non-standard output wrapper scripts
3. Consider writing a custom `LanguageParser` for the tool variant

**Prevention:**
- Always invoke `tsc` with `--noEmit --pretty false`
- Use structured output format (JSON for rustc)

### 3. Empty Output

**Symptom:** `FailureParserError::EmptyOutput` or empty `ParsedFailure`.

**Cause:** The tool produced no stdout/stderr when exit code was non-zero.

**Recovery:**
1. Check if stderr was captured separately
2. Verify the tool actually ran (process execution layer)
3. Combine stdout + stderr: `format!("{}\n{}", stdout, stderr)`

### 4. No Suggestion Generated

**Symptom:** `suggested_fix` is `None` on a `FailureDetail` or `suggest_fix()` returns `None`.

**Cause:** No matching symbols in `SourceContext` or source context is empty.

**Recovery:**
1. Ensure `SourceContext` is populated before calling `parse()`
2. For `MissingSymbol`: provide available symbols from the compiler error
3. For unknown symbols with no available list: no suggestion can be generated

**Prevention:**
- Build `SourceContext` from the code graph before parsing
- Cross-reference symbols using `CodeGraph` or equivalent

## Configuration Reference

| Parameter | Type | Default | Description |
|-----------|------|---------|-------------|
| Parser registry | `ParserRegistry` | empty | Registry of language/tool parsers |
| Confidence threshold | `f64` | 0.5 | Minimum confidence for fix suggestions |
| Tool name | `String` | — | Exact name matching for parser lookup |

## Health Check

The module provides no standalone health endpoint. Health is determined by:
1. Module compiles and loads successfully
2. `ParserRegistry::len()` returns expected number of parsers
3. Known tool names produce parse results instead of errors

## Metrics

| Metric | Type | Description |
|--------|------|-------------|
| `failure_parser.parse_count` | Counter | Total parse operations |
| `failure_parser.parse_errors` | Counter | Parse failures |
| `failure_parser.failures_parsed` | Counter | Total structured failures produced |
| `failure_parser.suggestions_generated` | Counter | Fix suggestions generated |

## Version History

| Version | Date | Changes |
|---------|------|---------|
| 1.0.0 | 2026-06-19 | Initial runbook |
