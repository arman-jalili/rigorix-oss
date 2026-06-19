# LLM Step Architecture

<!--
Canonical Reference: .pi/architecture/modules/llm-step.md
Blueprint Source: Rigorix design session (2026-06-19)
Rationale: Separate LLM-generated content from deterministic template steps for reusability and self-correction
-->

## Overview

The LLM Step module introduces a new execution primitive — a node type that calls the LLM **during execution** rather than during planning. This decouples generative content (test code, method bodies, documentation) from deterministic infrastructure (file reads, compiles, test runs).

The key insight: current templates bake LLM output into static strings. When the LLM hallucinates, the entire template is wrong. With `llm_generate` nodes, only the generative step is retried — the deterministic steps (file_read, compile-check, run-tests) are always correct and reusable.

## Philosophy

Rigorix templates should be **workflows**, not scripts. They should separate:

| Layer | Example | Retriable? |
|-------|---------|:----------:|
| **Deterministic infrastructure** | `file_read`, `file_patch` (AST-anchored), `run_command "npx tsc --noEmit"`, `run_command "npx jest"` | No — always correct |
| **Generative content** | Test code, method bodies, documentation, config files | Yes — retry with augmented context |

A template that succeeds once becomes a **reusable workflow**. The `llm_generate` prompt is the only LLM-dependent part — and it can be fine-tuned over time as the model improves or as the codebase evolves.

## Responsibilities

- Define `llm_generate` node type for execution engine
- Accept a prompt template with access to source file context
- Call the configured LLM provider during execution (not planning)
- Return generated content as structured output
- Support retry with context augmentation (failure analysis from previous attempts)
- Enforce budget constraints (max tokens, max calls per step)
- Cache generated content for replay and audit

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| LlmGenerateNode | `engine/src/llm_step/domain/node.rs` | Node type: prompt + context + budget | #node |
| LlmGenerateInput | `engine/src/llm_step/domain/input.rs` | LLM prompt with attached file contexts | #input |
| LlmGenerateOutput | `engine/src/llm_step/domain/output.rs` | Generated content + token usage | #output |
| LlmStepContext | `engine/src/llm_step/domain/context.rs` | Source file contents + previous failure analysis | #context |
| LlmStepService | `engine/src/llm_step/application/service.rs` | Service trait: generate, retry_with_context | #service |
| LlmStepConfig | `engine/src/llm_step/domain/config.rs` | Provider, model, max_tokens, retry budget | #config |
| LlmStepError | `engine/src/llm_step/domain/error.rs` | Typed errors: ProviderFailure, BudgetExhausted, InvalidPrompt | #error |
| LlmStepEvent | `engine/src/llm_step/domain/event.rs` | Events: GenerationStarted, GenerationCompleted, RetryAttempted | #event |

---

## Component Details

### LlmGenerateNode

**Purpose:** The execution engine sees this as just another node type — but internally it calls the LLM

```rust
/// An execution node that generates content by calling an LLM during execution.
///
/// Unlike planning-phase LLM calls (which produce templates), llm_generate
/// nodes produce content that feeds into subsequent deterministic nodes.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmGenerateNode {
    /// The prompt template sent to the LLM.
    /// Can reference context variables: {{source_files}}, {{previous_failure}}
    pub prompt: String,

    /// Maximum tokens for the LLM response (default: 4096).
    pub max_tokens: u32,

    /// Maximum retries with context augmentation before failure.
    pub max_retries: u32,

    /// Whether to cache generated output for replay.
    pub cache_output: bool,
}
```

### LlmStepContext

**Purpose:** Provides the LLM with source code context and previous failure analysis

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LlmStepContext {
    /// Source file contents formatted with // === path === headers.
    pub source_files: String,

    /// Previous failure analysis if this is a retry.
    pub previous_failure: Option<FailureAnalysis>,

    /// The original user intent for traceability.
    pub intent: String,

    /// Execution ID for correlation.
    pub execution_id: String,
}

/// Structured analysis of a previous execution failure.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FailureAnalysis {
    /// The typed failure classification.
    pub failure_type: String,

    /// Specific errors with location and suggestions.
    pub errors: Vec<FailureDetail>,

    /// Suggested fix from the failure parser.
    pub suggested_fix: Option<String>,
}
```

### LlmStepService

**Purpose:** Application service for LLM generation during execution

```rust
#[async_trait]
pub trait LlmStepService: Send + Sync {
    /// Generate content from a prompt with source context.
    async fn generate(
        &self,
        input: LlmGenerateInput,
    ) -> Result<LlmGenerateOutput, LlmStepError>;

    /// Retry generation with augmented context (failure analysis appended).
    /// Consumes the previous failure to inform the LLM what went wrong.
    async fn retry_with_context(
        &self,
        original_input: LlmGenerateInput,
        failure: FailureAnalysis,
    ) -> Result<LlmGenerateOutput, LlmStepError>;

    /// Get remaining budget for this step.
    fn remaining_budget(&self) -> LlmStepBudget;

    /// Cache a successful generation for replay.
    async fn cache_generation(
        &self,
        input_hash: &str,
        output: &LlmGenerateOutput,
    ) -> Result<(), LlmStepError>;
}
```

---

## Template Structure — Before and After

### Before (brittle — test content baked into template)

```toml
[[nodes]]
id = "write-test"
type = "file_write"
path = "tests/tasklist.test.ts"
content = """import { TaskList } from '../src/task';
describe(...) { ... }"""   # ← LLM output baked in during planning
```

### After (robust — LLM step separated from deterministic steps)

```toml
[[nodes]]
id = "read-source"
type = "file_read"
path = "src/task.ts"

[[nodes]]
id = "generate-test"
type = "llm_generate"
prompt = """
Generate a Jest test file for the TaskList class method.
Use ONLY the API signatures shown in the source files above.
Write a test file with proper imports and a describe/it block.
"""
max_tokens = 2048
max_retries = 2
depends_on = ["read-source"]

[[nodes]]
id = "write-test"
type = "file_write"
path = "tests/tasklist.test.ts"
content = "{{ generate-test.output }}"
depends_on = ["generate-test"]

[[nodes]]
id = "compile-check"
type = "run_command"
command = "npx tsc --noEmit"
depends_on = ["write-test"]

[[nodes]]
id = "run-tests"
type = "run_command"
command = "npx jest"
depends_on = ["compile-check"]
```

The deterministic nodes (`read-source`, `file_write`, `compile-check`, `run-tests`) are **identical across all TaskList templates**. Only the `llm_generate` prompt varies. When this template succeeds once, it becomes a reusable asset.

---

## Data Flow

```
Execution Engine encounters llm_generate node
        │
        ▼
LlmStepContext assembled:
  - Source files from read-source node output
  - Previous failure analysis (if retry)
  - Original user intent
        │
        ▼
LlmStepService::generate(input)
        │
        ▼
LLM Provider called with:
  System: "You are generating code for a template step."
  User: [prompt] + [source file context] + [failure analysis if retry]
        │
        ▼
LlmGenerateOutput {
    content: "import { TaskList } from '../src/task'; ...",
    token_usage: { input: 1500, output: 800 },
    model: "deepseek-v4-flash",
}
        │
        ▼
Output stored in execution context
        │
        ▼
Subsequent nodes reference {{ generate-test.output }}
        │
        ▼
If compile-check or run-tests fails:
  → FailureParser::parse(compiler_output)
  → PlanValidation loop retries llm_generate with augmented context
```

---

## Dependencies

### Depends On
- **Template System**: Template rendering to resolve `{{ node.output }}` references
- **Budget Tracking**: LLM token budget enforcement per step
- **Event System**: Generation lifecycle events

### Used By
- **Execution Engine**: New node type dispatch
- **Plan Validation**: Retry orchestration with context augmentation
- **Template Generation**: Templates emit `llm_generate` nodes instead of baked content

---

## Integration with Plan Validation

The `llm_generate` node is the **only retriable node type** in the validation loop:

```
PlanValidation loop:
  execute dag → [llm_generate node] → LLM generates content
                [file_write node]    → writes generated content
                [compile-check node] → ❌ fails

  FailureParser::parse("tsc output")
    → MissingSymbol { symbol: "addTask", suggestion: "add" }

  Context Augmentation:
    → "Your previous test used 'addTask' which doesn't exist. Use 'add' instead."

  Re-execute: only the llm_generate node + downstream dependencies
    → [llm_generate node] RETRY → generates corrected content
    → [file_write node]          → writes corrected content
    → [compile-check node]       → ✅ passes
    → [run-tests node]           → ✅ passes
```

---

## Security Considerations

| Concern | Mitigation |
|---------|------------|
| LLM generates malicious code | Generated content goes through same permission/risk gating as any write |
| Infinite retry loop | `max_retries` per node; overall validation loop iteration cap |
| Budget exhaustion | `LlmStepBudget` tracked per step; aborts when exhausted |
| Prompt injection via source files | Source content is read from disk, not user-provided |

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 90% | `engine/src/llm_step/` — per-component test modules |
| Integration | 85% | Mock LLM provider, simulated retry cycle |

**Key Test Scenarios:**
- `llm_generate` produces valid content → downstream nodes consume it
- `llm_generate` with `max_retries: 2` retries on failure context
- Source file context correctly injected into prompt
- `{{ node.output }}` template reference resolves correctly
- Budget exhaustion returns `LlmStepError::BudgetExhausted`

---

*Last updated: 2026-06-19*
*Module version: 1.0.0 (Implemented)*

---

**Status:** Implemented ✅
**Implementation:** LlmStepService, LlmContextBuilderService, LlmGenerateNode — all 36 tests passing, 53 contract validations passed
**Proofing:** CI stage 31 (llm-step_proofing) added to hardening pipeline
**Docs:** runbook-llm-step.md, dr-plan-llm-step.md
