# Failure Parser Architecture

<!--
Canonical Reference: .pi/architecture/modules/failure-parser.md
Blueprint Source: Rigorix design session (2026-06-19)
Rationale: Parse compiler/test/lint output into structured, actionable failure types for self-correction
-->

## Overview

The Failure Parser module ingests raw compiler, test runner, and linter output and produces structured, typed `TemplateFailure` values. Each failure carries precise location context (file, line, column), a machine-readable error code, and — critically — a **suggested fix** derived from the available source code context.

This is the bridge between "something failed" and "here's exactly what to change." The parser enables the validation loop to feed actionable feedback back to the LLM for self-correction.

## Philosophy

Compiler output is machine-readable — but LLMs need it translated. A TypeScript error like `TS2339: Property 'addTask' does not exist on type 'TaskList'` contains all the information needed to fix the problem, but only if it's structured properly. The Failure Parser extracts:

1. **What** failed — the error code and message
2. **Where** it failed — file, line, column
3. **Why** — the typed failure classification
4. **How to fix** — a suggestion derived from available symbols in the source

## Responsibilities

- Parse TypeScript compiler output (`tsc --noEmit --pretty false`)
- Parse Jest test runner output
- Parse Rust compiler output (`rustc --json=diagnostic`)
- Parse Python test output (pytest)
- Classify failures into typed enum: MissingSymbol, TypeMismatch, WrongArgCount, AssertionFailure, CompileError, TestFailure
- Extract location context: file path, line number, column number
- Generate suggested fixes by cross-referencing source code context
- Support extensible parser registry for new languages

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| TemplateFailure | `engine/src/failure_parser/domain/failure.rs` | Enum of structured failure types | #failure |
| FailureDetail | `engine/src/failure_parser/domain/detail.rs` | Individual error with location and suggestion | #detail |
| CompilerOutput | `engine/src/failure_parser/domain/input.rs` | Raw compiler/test output wrapper | #input |
| ParsedFailure | `engine/src/failure_parser/domain/output.rs` | Parsed result with all failures | #output |
| FailureParserService | `engine/src/failure_parser/application/service.rs` | Service trait: parse, classify, suggest | #service |
| TypeScriptParser | `engine/src/failure_parser/application/ts_parser.rs` | Parses `tsc` JSON output | #ts-parser |
| JestParser | `engine/src/failure_parser/application/jest_parser.rs` | Parses Jest test output | #jest-parser |
| RustcParser | `engine/src/failure_parser/application/rustc_parser.rs` | Parses `rustc` JSON diagnostics | #rustc-parser |
| ParserRegistry | `engine/src/failure_parser/domain/registry.rs` | Extensible parser by language/tool | #registry |
| FailureParserError | `engine/src/failure_parser/domain/error.rs` | Typed error enum | #error |

---

## Component Details

### TemplateFailure

**Purpose:** Typed classification of template execution failures

```rust
/// Typed classification of why a template execution failed.
///
/// Each variant carries structured context suitable for feeding back
/// to the LLM for self-correction.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TemplateFailure {
    /// A symbol (function, method, class, variable) was referenced but doesn't exist.
    MissingSymbol {
        /// The symbol that was referenced but not found.
        symbol: String,
        /// Available symbols in the same scope, if determinable.
        available: Vec<String>,
        /// Suggested replacement, if one closely matches.
        suggestion: Option<String>,
        /// Where the error occurred.
        location: SourceLocation,
    },

    /// Wrong number or type of arguments passed to a function/method.
    WrongArgCount {
        /// The function/method name.
        function: String,
        /// Expected number of arguments.
        expected: usize,
        /// Actual number of arguments provided.
        actual: usize,
        /// The call site.
        location: SourceLocation,
    },

    /// A type mismatch error.
    TypeMismatch {
        /// Expected type.
        expected: String,
        /// Actual type.
        actual: String,
        location: SourceLocation,
    },

    /// A compilation error that doesn't fit into more specific categories.
    CompileError {
        /// The compiler error code (e.g., TS2339, E0308).
        code: String,
        /// The error message.
        message: String,
        location: SourceLocation,
    },

    /// A test assertion failure.
    AssertionFailure {
        /// The test name.
        test_name: String,
        /// Expected value.
        expected: String,
        /// Received value.
        received: String,
        location: SourceLocation,
    },

    /// A generic test failure (test threw, timeout, etc.).
    TestFailure {
        /// The test name.
        test_name: String,
        /// The error message.
        message: String,
        location: Option<SourceLocation>,
    },
}

/// Location in source code where a failure occurred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceLocation {
    pub file: String,
    pub line: usize,
    pub column: Option<usize>,
}
```

### FailureParserService

**Purpose:** Parse raw output into structured failures with suggestions

```rust
#[async_trait]
pub trait FailureParserService: Send + Sync {
    /// Parse raw compiler/test output into structured failures.
    ///
    /// Accepts the raw stdout/stderr from a tool execution and produces
    /// typed `TemplateFailure` values. Returns an empty vec if no failures
    /// are detected.
    async fn parse(
        &self,
        tool: &str,            // "tsc", "jest", "rustc", "pytest"
        output: &str,           // raw stdout/stderr
        source_context: &SourceContext,  // available symbols for suggestions
    ) -> Result<Vec<TemplateFailure>, FailureParserError>;

    /// Generate a human-readable summary suitable for LLM context.
    ///
    /// Produces a string like:
    /// "FAILURE ANALYSIS: 1 error found.
    ///  - TS2339 at tests/tasklist.test.ts:3:10: Property 'addTask' not found.
    ///    Available methods on TaskList: add, list, complete, count, activeCount.
    ///    Suggested fix: use 'add' instead of 'addTask'."
    fn format_for_llm(&self, failures: &[TemplateFailure]) -> String;

    /// Classify the severity of the failure set.
    fn classify_severity(&self, failures: &[TemplateFailure]) -> FailureSeverity;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FailureSeverity {
    /// Compile errors — template is syntactically invalid.
    CompileBlock,
    /// Test failures — template compiled but logic is wrong.
    TestBlock,
    /// Warnings — template works but has issues.
    Warning,
}
```

### TypeScript Parser

**Purpose:** Parse `tsc --noEmit --pretty false` output into structured failures

```rust
/// Parses TypeScript compiler diagnostics.
///
/// tsc with --pretty false outputs each error as:
///   src/task.ts(3,10): error TS2339: Property 'addTask' does not exist on type 'TaskList'.
///
/// The parser extracts:
/// - File path and location
/// - Error code (TS2339, TS2554, TS2345, etc.)
/// - Error message
/// - Available symbols (if source context is provided)
pub struct TypeScriptParser;

impl TypeScriptParser {
    /// Known TypeScript error codes and their failure type mappings.
    const ERROR_MAP: &[(u16, fn(&str) -> TemplateFailure)] = &[
        (2339, Self::parse_missing_symbol),   // Property does not exist
        (2554, Self::parse_wrong_arg_count),  // Expected N arguments
        (2345, Self::parse_type_mismatch),    // Type mismatch
        (2304, Self::parse_missing_symbol),   // Cannot find name
        (2551, Self::parse_missing_symbol),   // Property does not exist (alternate)
    ];
}
```

### Suggested Fix Generation

**How suggestions are derived:**

```rust
impl FailureParserService for FailureParserServiceImpl {
    fn suggest_fix(
        failure: &TemplateFailure,
        source_context: &SourceContext,
    ) -> Option<String> {
        match failure {
            TemplateFailure::MissingSymbol { symbol, location, .. } => {
                // Search source context for symbols with similar names
                let all_symbols = source_context.symbols_in_file(&location.file);
                
                // Exact substring match: "addTask" contains "add"
                if let Some(matched) = all_symbols.iter()
                    .find(|s| symbol.contains(s.as_str()) || s.contains(symbol.as_str()))
                {
                    return Some(format!("Use '{matched}' instead of '{symbol}'"));
                }
                
                // Levenshtein distance < 3
                if let Some(closest) = find_closest_match(symbol, &all_symbols) {
                    return Some(format!("Did you mean '{closest}'? (similar to '{symbol}')"));
                }
                
                None
            }
            TemplateFailure::WrongArgCount { function, expected, actual, .. } => {
                Some(format!(
                    "'{function}' expects {expected} arguments but {actual} were provided"
                ))
            }
            _ => None,
        }
    }
}
```

---

## Extensible Parser Registry

```rust
pub struct ParserRegistry {
    parsers: HashMap<String, Box<dyn LanguageParser>>,
}

#[async_trait]
pub trait LanguageParser: Send + Sync {
    /// The tool this parser handles (e.g., "tsc", "jest", "rustc").
    fn tool_name(&self) -> &str;

    /// Parse the output and return structured failures.
    async fn parse(
        &self,
        output: &str,
        source_context: &SourceContext,
    ) -> Result<Vec<TemplateFailure>, FailureParserError>;
}

// Built-in parsers registered at startup:
// - TypeScriptParser → "tsc"
// - JestParser → "jest"
// - RustcParser → "rustc"
// - PytestParser → "pytest"
```

---

## Data Flow

```
Compile-check node executes:
  command = "npx tsc --noEmit --pretty false"
        │
        ▼
Exit code != 0 → stdout/stderr captured
        │
        ▼
FailureParserService::parse("tsc", output, source_context)
        │
        ▼
TypeScriptParser extracts errors:
  TS2339 at tests/tasklist.test.ts:3:10
    → TemplateFailure::MissingSymbol {
        symbol: "addTask",
        available: ["add", "list", "complete", "count", "activeCount"],
        suggestion: Some("Use 'add' instead of 'addTask'"),
        location: SourceLocation { file: "tests/tasklist.test.ts", line: 3, column: 10 },
      }
        │
        ▼
FailureParserService::format_for_llm(failures)
        │
        ▼
"FAILURE ANALYSIS: 1 compile error found.
 - tests/tasklist.test.ts:3:10: Property 'addTask' does not exist on TaskList.
   Available methods: add, list, complete, count, activeCount.
   SUGGESTED FIX: Use 'add' instead of 'addTask'.
   
   The correct call is: list.add(task) not list.addTask(task)."
        │
        ▼
Fed to PlanValidation → augments llm_generate prompt → retry
```

---

## Dependencies

### Depends On
- **Code Graph**: Symbol extraction from source files for suggestion generation
- **Event System**: Parser events for observability

### Used By
- **Plan Validation**: Consumes parsed failures for context augmentation
- **LLM Step**: Receives FailureAnalysis in retry context
- **Quality Gates**: Failure severity feeds into quality gate evaluation

---

## Error Code Mappings

| Compiler | Error Code | TemplateFailure Variant |
|----------|-----------|------------------------|
| tsc | TS2339 | MissingSymbol |
| tsc | TS2554 | WrongArgCount |
| tsc | TS2345 | TypeMismatch |
| tsc | TS2304 | MissingSymbol |
| tsc | TS1005 | CompileError |
| rustc | E0308 | TypeMismatch |
| rustc | E0061 | WrongArgCount |
| rustc | E0425 | MissingSymbol |
| rustc | E0432 | MissingSymbol |
| jest | AssertionError | AssertionFailure |
| jest | ReferenceError | MissingSymbol |
| pytest | AssertionError | AssertionFailure |
| pytest | NameError | MissingSymbol |

---

*Last updated: 2026-06-19*
*Module version: 1.0.0*

---

**Status:** Implemented
**Implementation:** [Issue #495 (Contract Freeze)](https://github.com/arman-jalili/rigorix-oss/issues/495), [Issue #496 (TemplateFailure)](https://github.com/arman-jalili/rigorix-oss/issues/496),
[Issue #497 (FailureParserService)](https://github.com/arman-jalili/rigorix-oss/issues/497),
[Issue #498 (TypeScript Parser)](https://github.com/arman-jalili/rigorix-oss/issues/498),
[Issue #499 (Suggested Fix Generation)](https://github.com/arman-jalili/rigorix-oss/issues/499),
[Issue #500 (Proofing & CI)](https://github.com/arman-jalili/rigorix-oss/issues/500),
[Issue #501 (Architecture Readiness)](https://github.com/arman-jalili/rigorix-oss/issues/501)
**Implementation priority:** P0 — bridges raw failures to self-correction
**Epic:** [Issue #494 (failure-parser)](https://github.com/arman-jalili/rigorix-oss/issues/494)
**Coverage:** 136+ tests (123 unit + 13 integration), 24 contract checks in CI
