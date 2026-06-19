# Code Generation Pipeline Architecture

<!--
Canonical Reference: .pi/architecture/modules/code-generation.md
Rationale: Reliable LLM → code insertion with exact-string anchoring, structured feedback, and syntax verification
-->

## Overview

The Code Generation Pipeline is the subsystem that converts LLM-generated code into correctly-positioned file edits. It provides three tiers of tooling — `read_file` (context gathering), `edit_file` (targeted string replacement as the primary insertion mechanism), and `write_file` (full file replacement) — plus a post-edit syntax verification gate powered by Rigorix's existing tree-sitter integration.

The core innovation over naive "write this code" approaches is the **exact-string anchor pattern**: the LLM quotes the precise text it wants to replace (`old_string`), and the engine refuses the edit if that text does not exist in the file. This eliminates hallucinated edits, wrong-line insertions, and silent corruption.

## Adoption Rationale

`edit_file` tool with exact-string matching is the foundation of reliable code generation. Rigorix currently has `FileWriteTool` (full replace) and `FilePatchTool` (AST-aware patching) but lacks the simple, battle-tested `old_string`/`new_string` pattern. Adopting this gives Rigorix:

- **Position-anchored insertion**: `old_string` doubles as both the position marker and the correctness check
- **Self-correcting feedback loop**: `EditFileResult` returns `original_file`, `updated_content`, and `unified_diff` so the LLM can verify its edit in the next turn
- **Tree-sitter syntax verification**: Rigorix's existing `code_graph` module provides AST validation that no other agent harness has
- **Binary safety**: NUL-byte detection prevents attempts to edit binary files
- **Structured patch output**: Human-readable and LLM-readable diff format for verification

## Responsibilities

- Provide `read_file` with offset/limit and binary detection
- Provide `edit_file` with exact string matching (old_string → new_string)
- Provide `write_file` with workspace boundary enforcement and atomic writes
- Return structured feedback on every write/edit: original content, new content, unified diff
- Run post-edit tree-sitter syntax verification (optional gate)
- Enforce file size limits (read: 10MB, write: 10MB)
- Validate file paths stay within workspace boundary
- Support `replace_all` flag for multi-occurrence edits
- Emit code generation events for audit trail

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| ReadFileTool | `engine/src/tools/file_read.rs` | Read files with offset/limit, binary detection | #read-file |
| EditFileTool | `engine/src/tools/file_edit.rs` | Exact-string replacement with position anchoring | #edit-file |
| WriteFileTool | `engine/src/tools/file_write.rs` | Full file write with workspace boundary, atomic rename | #write-file |
| EditFileInput | `engine/src/tools/domain/edit_input.rs` | Input struct: path, old_string, new_string, replace_all | #edit-input |
| EditFileResult | `engine/src/tools/domain/edit_result.rs` | Output: original_file, updated_content, unified_diff, patch hunks | #edit-result |
| StructuredPatchHunk | `engine/src/tools/domain/patch.rs` | Individual diff hunk with old/new line ranges | #patch-hunk |
| SyntaxGate | `engine/src/code_gen/application/syntax_gate.rs` | Post-edit tree-sitter AST validation | #syntax-gate |
| SyntaxGateService | `engine/src/code_gen/application/service.rs` | Service trait for syntax verification | #syntax-service |
| SyntaxGateResult | `engine/src/code_gen/domain/result.rs` | Outcome: Passed, Failed { errors }, Skipped | #syntax-result |
| PathValidator | `engine/src/tools/domain/path_validator.rs` | Workspace boundary enforcement, symlink detection, binary detection | #path-validator |

---

## Component Details

### EditFileTool

**Purpose:** The primary code insertion mechanism. Replaces an exact text string in a file with new content. The `old_string` serves as both the position anchor and the correctness anchor.

**Implementation File:** `engine/src/tools/file_edit.rs`

**Canonical Reference:** `.pi/architecture/modules/code-generation.md#edit-file`

**Algorithm:**
1. Resolve and canonicalize path; validate workspace boundary
2. Read full file contents into memory
3. **Identity check**: if `old_string == new_string`, return `ToolError::InvalidInput`
4. **Existence check**: if `old_string` is not found in the file, return `ToolError::NotFound`
5. Replace first occurrence (or all if `replace_all: true`)
6. Write updated content to disk (atomic write-rename)
7. Compute unified diff between original and updated
8. Optionally run syntax gate
9. Return `EditFileResult` with full before/after/diff

```rust
pub struct EditFileTool;

#[async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &str { "edit_file" }

    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let params: EditFileInput = serde_json::from_value(input.params.clone())?;

        // Gate 1: path validation
        let resolved = PathValidator::validate(&params.path, &input.workspace_root)?;

        // Gate 2: read original
        let original = fs::read_to_string(&resolved)
            .map_err(|e| ToolError::ExecutionFailed(format!("read failed: {e}")))?;

        // Gate 3: identity check
        if params.old_string == params.new_string {
            return Err(ToolError::InvalidInput(
                "old_string and new_string must differ".into()
            ));
        }

        // Gate 4: existence check (THE correctness anchor)
        if !original.contains(&params.old_string) {
            return Err(ToolError::NotFound(
                format!("old_string not found in {}", params.path)
            ));
        }

        // Replace
        let updated = if params.replace_all.unwrap_or(false) {
            original.replace(&params.old_string, &params.new_string)
        } else {
            original.replacen(&params.old_string, &params.new_string, 1)
        };

        // Atomic write
        let tmp = format!("{}.tmp", resolved.display());
        fs::write(&tmp, &updated)?;
        fs::rename(&tmp, &resolved)?;

        // Compute diff
        let diff = compute_unified_diff(&original, &updated, &params.path);

        Ok(ToolResult {
            output: serde_json::to_string(&EditFileResult {
                file_path: params.path.clone(),
                old_string: params.old_string,
                new_string: params.new_string,
                original_file: original,
                updated_content: updated,
                unified_diff: diff,
                replace_all: params.replace_all.unwrap_or(false),
            })?,
            side_effects: vec![SideEffect::FileModified(params.path)],
        })
    }
}
```

### EditFileInput

**Purpose:** The structured input the LLM provides to target an edit

```rust
#[derive(Debug, Deserialize)]
pub struct EditFileInput {
    /// Absolute or workspace-relative file path
    pub path: String,
    /// Exact text to find and replace (position anchor)
    pub old_string: String,
    /// Replacement text (can be larger, smaller, or same length)
    pub new_string: String,
    /// Replace all occurrences (default: first match only)
    pub replace_all: Option<bool>,
}
```

### EditFileResult

**Purpose:** Structured feedback returned to the LLM after every edit — enables self-verification

```rust
#[derive(Debug, Serialize)]
pub struct EditFileResult {
    pub file_path: String,
    pub old_string: String,
    pub new_string: String,
    /// Complete original file content before the edit
    pub original_file: String,
    /// Complete updated file content after the edit
    pub updated_content: String,
    /// Unified diff (human and LLM readable)
    pub unified_diff: String,
    pub replace_all: bool,
}
```

### StructuredPatchHunk

**Purpose:** Individual diff hunk for structured diff output

```rust
#[derive(Debug, Serialize)]
pub struct StructuredPatchHunk {
    pub old_start: usize,
    pub old_lines: usize,
    pub new_start: usize,
    pub new_lines: usize,
    pub lines: Vec<String>,  // lines prefixed with '-' (removed) or '+' (added)
}
```

### ReadFileTool

**Purpose:** Read files with offset/limit paging and binary detection

**Implementation Extensions (beyond current FileReadTool):**
- Binary file detection via NUL byte scan (first 8KB)
- `BinaryDetected` error with clear message
- `FileTooLarge` error (10MB cap)
- Returns `total_lines` for the LLM to understand file scope

### SyntaxGate

**Purpose:** Post-edit tree-sitter syntax verification — uses Rigorix's existing `code_graph` module

**Implementation File:** `engine/src/code_gen/application/syntax_gate.rs`

```rust
pub struct SyntaxGate {
    /// Supported language parsers: Rust, TypeScript, Python
    parsers: HashMap<String, tree_sitter::Parser>,
}

impl SyntaxGate {
    /// Verify that `content` produces a valid AST for the file at `path`.
    /// Language is auto-detected from file extension.
    pub fn verify(&self, path: &str, content: &str) -> SyntaxGateResult {
        let language = detect_language_from_extension(path);
        let Some(parser) = self.parsers.get(&language) else {
            return SyntaxGateResult::Skipped {
                reason: format!("no parser for language: {language}"),
            };
        };

        let tree = parser.parse(content, None)
            .ok_or_else(|| /* error */)?;

        if tree.root_node().has_error() {
            let errors = find_syntax_errors(&tree, content);
            return SyntaxGateResult::Failed { errors };
        }

        SyntaxGateResult::Passed
    }
}
```

**SyntaxGateResult:**

```rust
#[derive(Debug, Serialize)]
pub enum SyntaxGateResult {
    /// File parses without errors
    Passed,
    /// Syntax errors found — returned with error locations
    Failed { errors: Vec<SyntaxError> },
    /// No parser available for this language (not an error)
    Skipped { reason: String },
}

#[derive(Debug, Serialize)]
pub struct SyntaxError {
    pub line: usize,
    pub column: usize,
    pub message: String,
    pub context: String,  // surrounding code for the LLM to understand
}
```

---

## Data Flow

```
LLM issues read_file(path, offset, limit)
        │
        ▼
┌──────────────────────────────┐
│ ReadFileTool                 │
│  1. Binary check (NUL scan)  │
│  2. Size check (10MB cap)    │
│  3. Read lines [offset:limit]│
│  4. Return { content,         │
│       startLine, totalLines } │
└──────────────────────────────┘
        │
        ▼  (LLM reads context, finds insertion point)
        │
LLM issues edit_file(path, old_string, new_string)
        │
        ▼
┌──────────────────────────────────────┐
│ EditFileTool                         │
│  1. Path validation (workspace bnd)  │
│  2. Read original file               │
│  3. IDENTITY CHECK: old ≠ new?       │──── no ──→ ToolError::InvalidInput
│  4. EXISTENCE CHECK: old in file?    │──── no ──→ ToolError::NotFound
│  5. Replace old → new                │
│  6. Atomic write (tmp → rename)      │
│  7. Compute unified diff             │
│        │                             │
│        ▼                             │
│  8. [Configurable] SyntaxGate.verify │
│     ├─ Passed → continue             │
│     ├─ Failed { errors } → warning   │
│     └─ Skipped → continue            │
│        │                             │
│        ▼                             │
│  9. Return EditFileResult {          │
│       original_file,                 │
│       updated_content,               │
│       unified_diff                   │
│     }                                │
└──────────────────────────────────────┘
        │
        ▼  (LLM receives result, inspects diff)
        │
   LLM self-verifies:
   - Is old_string what I intended to replace?
   - Does unified_diff show exactly my change?
   - If wrong → issue corrective edit_file
   - If right → continue
```

**Flow Description:**
1. LLM gathers context via `read_file` (with line numbers) or `grep_search`
2. LLM identifies the exact text to replace and produces `old_string`/`new_string`
3. Engine validates the edit exists (existence check = correctness anchor)
4. Engine writes atomically and computes diff
5. Engine optionally runs syntax gate (tree-sitter AST verification)
6. LLM receives full before/after/diff and can self-verify
7. If the edit was wrong, LLM can issue a corrective edit in the next turn

---

## Comparison: edit_file vs FilePatchTool

Rigorix already has `FilePatchTool` (AST-aware patching). The new `edit_file` serves a different purpose:

| Dimension | edit_file (new) | FilePatchTool (existing) |
|-----------|-----------------|--------------------------|
| **Mechanism** | Exact string matching | AST-aware structural patching |
| **Input** | `old_string` + `new_string` (text) | Patch description + structural intent |
| **Correctness check** | String containment (simple, fast) | AST validation (complex, slower) |
| **Use case** | Targeted insertions, small changes, boilerplate | Large refactors, cross-cutting structural changes |
| **LLM cognitive load** | Low — quote exact text | High — must understand AST structure |
| **Failure mode** | "old_string not found" (clear) | AST parse failure (ambiguous) |
| **Performance** | O(n) string search, no parsing | Parses entire file through tree-sitter |
| **Language support** | All text files | Only tree-sitter-supported languages |

**Recommendation:** Both tools coexist. `edit_file` is the primary workhorse for 90% of edits. `FilePatchTool` is reserved for structural refactors where AST awareness adds value.

---

## Dependencies

### Depends On
- **Code Graph** (tree-sitter): Language parsers for syntax gate verification
- **Risk Gating**: `RiskLevel::Medium` for both `edit_file` and `write_file`
- **Configuration**: Workspace root for path validation
- **Event System**: Edit events for audit trail

### Used By
- **Execution Engine**: Resolves `edit_file` tool from registry
- **Orchestrator**: Registers `EditFileTool` during build
- **Planning Pipeline**: LLM is prompted with `edit_file` schema

---

## Security Considerations

| Concern | Mitigation | Validator |
|---------|------------|-----------|
| Edits outside workspace | Path canonicalization + boundary check (must start with workspace_root) | security-validator |
| Symlink escape | Symlink detection before path validation; canonicalize resolves symlinks | security-validator |
| Binary file corruption | Binary detection via NUL byte scan before read; binary files rejected for edit | security-validator |
| Overwrite of critical files | `edit_file` only replaces matched text, never full-file unless matched; `write_file` can be gated by risk config | security-validator |
| Large file DoS | 10MB cap on both read and write; rejection before allocation | security-validator |
| Race condition (read → edit) | Atomic write-rename pattern; file read is snapshot at edit time | security-validator |

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 90% | `engine/src/tools/file_edit.rs`, `engine/src/tools/file_read.rs` |
| Integration | 85% | `engine/src/tools/tests/edit_file_tests.rs` |
| E2E | 80% | Full edit → verify → correct cycle |

**Key Test Scenarios:**
- `edit_file` with exact `old_string` match → file updated, diff returned
- `edit_file` with `old_string` not in file → `ToolError::NotFound`
- `edit_file` with `old_string == new_string` → `ToolError::InvalidInput`
- `edit_file` with `replace_all: true` → all occurrences replaced
- `edit_file` on path outside workspace → `ToolError::PathDenied`
- `read_file` on binary file → `ToolError::BinaryDetected`
- `read_file` with offset/limit → correct line window returned
- Syntax gate on valid Rust after edit → `Passed`
- Syntax gate on invalid Rust after edit → `Failed { errors: [...] }`
- Syntax gate on Markdown file → `Skipped` (no parser)

---

## Error Handling

```rust
// Errors specific to code generation tools
pub enum CodeGenError {
    #[error("old_string not found in {path}")]
    OldStringNotFound { path: String },
    #[error("old_string and new_string must differ")]
    IdentityEdit,
    #[error("file appears to be binary")]
    BinaryFile,
    #[error("file too large: {size} bytes (max {max})")]
    FileTooLarge { size: u64, max: u64 },
    #[error("path escapes workspace: {path}")]
    WorkspaceEscape { path: String },
    #[error("syntax error at {path}:{line}:{col}: {message}")]
    SyntaxError { path: String, line: usize, col: usize, message: String },
}
```

---

## Tool Schema (JSON for LLM)

The LLM receives this tool definition in its system prompt:

```json
{
    "name": "edit_file",
    "description": "Replace an exact text string in a workspace file. The old_string must exist in the file exactly as provided — this is both the position anchor and correctness check. Returns the original file, updated file, and a unified diff for verification.",
    "input_schema": {
        "type": "object",
        "properties": {
            "path": { "type": "string", "description": "File path relative to workspace root" },
            "old_string": { "type": "string", "description": "Exact text to find and replace — must be present in the file" },
            "new_string": { "type": "string", "description": "Replacement text" },
            "replace_all": { "type": "boolean", "description": "Replace all occurrences (default: first match only)" }
        },
        "required": ["path", "old_string", "new_string"]
    }
}
```

---

*Last updated: 2026-06-19*
*Module version: 1.0.0 (Planned)*

---

**Status:** Planned  
**Implementation priority:** P0 — core code manipulation primitive
