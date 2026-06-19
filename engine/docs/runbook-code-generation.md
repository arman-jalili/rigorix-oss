# Runbook: code-generation Module

<!--
Canonical Reference: .pi/architecture/modules/code-generation.md
Last Updated: 2026-06-19
-->

## Overview

The `code-generation` module converts LLM-generated code into correctly-positioned file edits. It provides three tiers of tooling — `read_file` (context gathering), `edit_file` (targeted string replacement), and `write_file` (full file replacement) — plus a post-edit syntax verification gate powered by tree-sitter.

The core innovation is the **exact-string anchor pattern**: the LLM quotes the precise text it wants to replace (`old_string`), and the engine refuses the edit if that text does not exist in the file.

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| tree-sitter parsers | Yes | rust, typescript, python |
| tokio runtime | Yes | Async tool execution |
| Workspace root | Yes | Path validation |
| Configuration | Yes | SyntaxGateConfig, EditFileConfig |

### Initialization

1. Create `SyntaxGateConfig` (default or custom)
2. Create `SyntaxGateImpl` with the config
3. Register `EditFileTool` in `ToolRegistry`
4. Register `FileReadTool` in `ToolRegistry`

```rust
use rigorix::code_gen::application::*;

let gate_config = SyntaxGateConfig::default();
let syntax_gate = SyntaxGateImpl::new(gate_config);

// Tools are registered in the ToolRegistry
let edit_tool = EditFileTool::new(workspace_root);
let read_tool = FileReadTool::new(workspace_root);
registry.register("edit_file", Arc::new(edit_tool));
registry.register("file-read", Arc::new(read_tool));
```

### Quick Start

```rust
use rigorix::code_gen::application::dto::*;
use rigorix::code_gen::application::*;

// 1. Read a file
let read_input = ReadFileInput {
    path: "src/main.rs".into(),
    offset: None,
    limit: None,
    max_file_size: None,
};

// 2. Edit a file (via ToolRegistry)
let edit_input = EditFileInput {
    path: "src/main.rs".into(),
    old_string: "old_function".into(),
    new_string: "new_function".into(),
    replace_all: None,
};

// 3. Verify syntax
let syntax_input = SyntaxGateInput {
    file_path: "src/main.rs".into(),
    content: updated_content,
};
let result = syntax_gate.verify(syntax_input)?;
```

## Graceful Shutdown

The code-generation module has no long-running processes or background tasks. Shutdown is immediate:

1. In-flight tool executions complete or are cancelled via `AbortSignal`
2. No state to flush (edits are written atomically via write-rename)
3. No connections to close

### Atomic Write Safety

- `edit_file` writes to a `.tmp` file, then renames atomically
- If the process crashes between write and rename, the original file is preserved
- `.tmp` files are cleaned up on failure

## Common Failure Modes and Recovery

| Failure Mode | Symptom | Cause | Recovery |
|-------------|---------|-------|----------|
| old_string not found | `CodeGenError::OldStringNotFound` | LLM hallucinated text | LLM must re-read file and issue corrected edit |
| Identity edit | `CodeGenError::IdentityEdit` | old_string == new_string | LLM must provide different strings |
| Binary file | `CodeGenError::BinaryFile` | Attempted edit on binary file | LLM must use write_file instead |
| File too large | `CodeGenError::FileTooLarge` | File exceeds 10MB | LLM must use offset/limit or split file |
| Path escape | `CodeGenError::WorkspaceEscape` | Path traversal attempt | Path must be within workspace root |
| Syntax error | SyntaxGateResult::Failed | Edit broke syntax | LLM receives error locations and issues corrective edit |
| No parser | SyntaxGateResult::Skipped | Unsupported language | Edit applied without verification |

## Configuration Reference

### SyntaxGate Configuration

```rust
SyntaxGateConfig {
    enabled: true,                    // Enable syntax verification
    block_on_error: false,            // Don't block on syntax errors
    skip_unsupported: true,           // Skip unsupported languages
    max_verify_size: 1_048_576,       // 1MB max for verification
    supported_languages: vec![
        "rust".into(),
        "typescript".into(),
        "python".into(),
    ],
}
```

### EditFile Configuration

```rust
EditFileConfig {
    max_file_size: 10_485_760,        // 10MB max for editing
    enable_identity_check: true,      // Reject identical old/new strings
    require_syntax_gate: false,       // Don't require syntax gate
    max_replacements: 1000,           // Max replace_all occurrences
}
```

## Monitoring

### Key Metrics

- `edit_file_executions` — Total edit_file operations
- `edit_file_success_rate` — Percentage of successful edits
- `edit_file_failures_by_cause` — Failure breakdown (not_found, identity, binary, etc.)
- `syntax_gate_passes` — Number of passed syntax checks
- `syntax_gate_failures` — Number of failed syntax checks
- `read_file_operations` — Total read_file operations
- `read_file_bytes_read` — Total bytes read
- `file_size_distribution` — Distribution of edited file sizes

### Logging

- Every `edit_file` operation logs: path, old_string length, new_string length, result
- Every `read_file` operation logs: path, offset, limit, total_lines, bytes
- Syntax gate results logged with error locations
- All errors logged with structured context for debugging

## Health Check

A healthy code-generation module:
1. Syntax gate can parse Rust, TypeScript, and Python files
2. EditFileTool can read, replace, and write files atomically
3. Path validation correctly rejects traversal attempts
4. Binary detection correctly identifies NUL-containing files
5. File size limits are enforced

## Troubleshooting

### EditFileTool Returns "old_string not found"

1. LLM may have hallucinated the old_string
2. File may have been modified since last read
3. Whitespace differences (tabs vs spaces, trailing newlines)
4. Solution: re-read file and re-issue edit

### Syntax Gate False Positives

1. Some tree-sitter grammars are permissive about certain constructs
2. Macro-heavy Rust code may produce false syntax errors
3. Solution: disable syntax gate for specific files or set `block_on_error: false`

### Binary File False Positive

1. Files with high-entropy text may trigger NUL detection
2. Files with null bytes in comments or strings
3. Solution: increase binary scan threshold or use write_file

## Performance

| Metric | Target | Notes |
|--------|--------|-------|
| edit_file (small file, < 1MB) | < 50ms | String replace is O(n) |
| edit_file (large file, < 10MB) | < 500ms | IO-bound for large files |
| read_file (with offset/limit) | < 20ms | Line splitting overhead |
| SyntaxGate verify (Rust, < 1MB) | < 100ms | Tree-sitter parse time |
| SyntaxGate verify (TypeScript, < 1MB) | < 200ms | TSX grammar is larger |

## Related Documents

- [Architecture: code-generation](../.pi/architecture/modules/code-generation.md)
- [DR Plan: code-generation](dr-plan-code-generation.md)
- [Tool System Architecture](../.pi/architecture/modules/tool-system.md)
