# Plan Validation Architecture

<!--
Canonical Reference: .pi/architecture/modules/plan-validation.md
Blueprint Source: Rigorix design session (2026-06-19)
Rationale: Self-correcting plan→execute→verify→fix loop for template reliability
-->

## Overview

The Plan Validation module orchestrates the self-correcting loop around the planning→execution→verification pipeline. When a template execution fails (compile error, test failure, quality gate unsatisfied), the validation loop parses the failure, augments the planning context with structured feedback, and re-executes — targeting only the LLM-generated content steps.

The core rule: **deterministic steps are never retried; only `llm_generate` nodes are retried with augmented context.** This preserves the reusability of the template infrastructure while enabling self-correction of generative content.

## Philosophy

Rigorix values **repeatability** and **reusable templates**. The Plan Validation loop achieves both:

1. **Repeatability**: The same deterministic steps (file_read, file_patch with AST anchors, compile-check, test-run) execute identically every time
2. **Reusability**: When a template succeeds through the validation loop, the resulting template is a reusable asset — the `llm_generate` prompt is the only retried component
3. **Self-correction**: Failures are analyzed structurally, not just reported. The LLM receives precise instructions on what to fix

A template that passes validation three times in a row for different inputs is **production-grade**. A template that fails validation is automatically diagnosed and retired with a structured failure report.

## Responsibilities

- Wrap the plan→execute→verify pipeline in a retry loop
- On failure: parse errors via FailureParser, augment context, re-plan only LLM steps
- On success: mark template as validated, cache the llm_generate prompt
- Enforce max iterations (default: 3 — one attempt + two retries)
- Track cumulative budget across validation attempts
- Produce structured validation reports for audit
- Integrate with quality gates for go/no-go decisions
- Separate deterministic from generative retry targets

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| ValidationLoop | `engine/src/plan_validation/domain/loop_config.rs` | Config: max_iterations, budget, quality_threshold | #config |
| ValidationState | `engine/src/plan_validation/domain/state.rs` | Per-iteration state: attempt, failures, augmented intent | #state |
| ValidationOutcome | `engine/src/plan_validation/domain/outcome.rs` | Result: Validated, Failed, ExhaustedRetries | #outcome |
| ValidationReport | `engine/src/plan_validation/domain/report.rs` | Structured report: iterations, failures, fixes, final template | #report |
| ValidationLoopService | `engine/src/plan_validation/application/service.rs` | Service trait: validate, retry_target_nodes | #service |
| ValidationLoopImpl | `engine/src/plan_validation/application/loop_impl.rs` | Concrete implementation of the validation loop | #impl |
| ContextAugmenter | `engine/src/plan_validation/application/context_augmenter.rs` | Augments planning context with failure analysis | #augmenter |
| ValidationLoopError | `engine/src/plan_validation/domain/error.rs` | Typed error enum | #error |
| ValidationEvent | `engine/src/plan_validation/domain/event.rs` | Events: IterationStarted, IterationFailed, Validated | #event |

---

## Component Details

### ValidationLoop Config

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationLoopConfig {
    /// Maximum validation iterations (1 initial + N retries).
    /// Default: 3 (one attempt + two retries with augmented context).
    pub max_iterations: u32,

    /// Quality level required for validation to be considered successful.
    /// Default: Package (at least crate-level tests must pass).
    pub required_quality: QualityLevel,

    /// Maximum cumulative LLM tokens across all validation iterations.
    /// Default: 50,000.
    pub max_cumulative_tokens: u64,

    /// Whether to cache successful templates for replay.
    pub cache_successful_templates: bool,
}
```

### ValidationState

```rust
#[derive(Debug, Clone)]
pub struct ValidationState {
    /// Current iteration number (1-indexed).
    pub iteration: u32,

    /// The current augmented intent (original + failure context).
    pub current_intent: UserIntent,

    /// All failures encountered so far, ordered by iteration.
    pub failure_history: Vec<Vec<TemplateFailure>>,

    /// The template being validated.
    pub template: Option<Template>,

    /// Cumulative LLM token usage across all iterations.
    pub cumulative_tokens: u64,

    /// Whether the validation has succeeded.
    pub succeeded: bool,
}
```

### ValidationLoopService

```rust
#[async_trait]
pub trait ValidationLoopService: Send + Sync {
    /// Execute the full validation loop for a given intent.
    ///
    /// Flow: plan → execute → verify → [if fail: parse → augment → plan → execute → verify] → return
    ///
    /// Returns `ValidationOutcome::Validated` with the final template on success,
    /// or `ValidationOutcome::Failed` with the failure history on exhaustion.
    async fn validate(
        &self,
        intent: UserIntent,
        config: ValidationLoopConfig,
    ) -> Result<ValidationOutcome, ValidationLoopError>;

    /// Identify which nodes in a template are retriable (llm_generate nodes)
    /// and which are deterministic (everything else).
    fn classify_nodes(&self, template: &Template) -> NodeClassification;

    /// Retry only the llm_generate nodes with augmented context,
    /// reusing deterministic node outputs from the previous iteration.
    async fn retry_generative_nodes(
        &self,
        previous_template: &Template,
        failures: &[TemplateFailure],
        source_context: &SourceContext,
    ) -> Result<Template, ValidationLoopError>;
}

/// Classification of template nodes for retry targeting.
#[derive(Debug, Clone)]
pub struct NodeClassification {
    /// Nodes that produce generative content (llm_generate).
    pub generative: Vec<NodeId>,
    /// Nodes that are deterministic (file_read, file_patch, run_command, etc.).
    pub deterministic: Vec<NodeId>,
}
```

### ContextAugmenter

```rust
/// Transforms failure analysis into augmented planning context.
pub struct ContextAugmenter;

impl ContextAugmenter {
    /// Augment an intent with failure analysis for re-planning.
    ///
    /// Produces an intent like:
    /// "Original intent: Add getActiveTasks method...
    ///
    ///  PREVIOUS EXECUTION FAILED. Fix the following errors:
    ///  1. tests/tasklist.test.ts:3:10 — 'addTask' does not exist on TaskList.
    ///     SUGGESTED FIX: Use 'add' instead of 'addTask'.
    ///     Available methods: add, list, complete, count, activeCount."
    pub fn augment_intent(
        intent: &UserIntent,
        failures: &[TemplateFailure],
        failure_history: &[Vec<TemplateFailure>],
        parser: &dyn FailureParserService,
    ) -> UserIntent {
        let mut augmented = intent.input.clone();
        augmented.push_str("\n\n--- PREVIOUS EXECUTION FAILED ---\n");
        augmented.push_str(&parser.format_for_llm(failures));

        if failure_history.len() > 1 {
            augmented.push_str(&format!(
                "\n\nThis is attempt {}. Previous attempts also failed. \
                 Ensure this fix addresses ALL previously reported errors.",
                failure_history.len() + 1
            ));
        }

        augmented.push_str("\n\nGenerate corrected content. Do NOT repeat the same mistakes.");

        UserIntent::new(augmented, intent.execution_id)
    }

    /// Check if a failure is a repeat of a previous failure.
    /// Repeated failures suggest the LLM didn't understand the fix.
    fn is_repeated_failure(
        &self,
        failure: &TemplateFailure,
        history: &[Vec<TemplateFailure>],
    ) -> bool {
        history.iter().any(|prev| prev.contains(failure))
    }
}
```

---

## Data Flow

```
User Intent
    │
    ▼
┌─────────────────────────────────────────────────────┐
│ ValidationLoopService::validate(intent, config)      │
│                                                       │
│  iteration = 1                                        │
│  ┌───────────────────────────────────────────────┐   │
│  │ 1. PlanningPipeline::plan_with_graph(intent)   │   │
│  │    → Template with llm_generate nodes          │   │
│  │                                                 │   │
│  │ 2. ExecutionEngine::execute_graph(template)     │   │
│  │    → llm_generate calls LLM, produces content   │   │
│  │    → file_write writes content to disk          │   │
│  │    → compile-check runs tsc --noEmit           │   │
│  │                                                 │   │
│  │ 3. QualityGateService::evaluate_gate()          │   │
│  │    ├─ Satisfied? → return Validated ✅          │   │
│  │    └─ Unsatisfied? → continue to step 4         │   │
│  │                                                 │   │
│  │ 4. FailureParserService::parse(compiler_output) │   │
│  │    → Vec<TemplateFailure>                        │   │
│  │                                                 │   │
│  │ 5. If iteration >= max_iterations:               │   │
│  │    → return Failed(history) ❌                   │   │
│  │                                                 │   │
│  │ 6. ContextAugmenter::augment_intent(             │   │
│  │      intent, failures, history)                  │   │
│  │    → augmented intent with failure context       │   │
│  │                                                 │   │
│  │ 7. iteration++ → goto step 1                     │   │
│  └───────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────┘
```

---

## Selective Retry: Only LLM Steps

The validation loop is smarter than a full re-execution. It **only retries generative nodes**:

```
Iteration 1:
  [read-source]      ✅ completed (cached)
  [patch-tasklist]   ✅ completed (cached — deterministic, always correct)
  [generate-test]    ❌ produced hallucinated content
  [write-test]       ✅ completed (writes whatever generate-test produced)
  [compile-check]    ❌ failed

Iteration 2 (SELECTIVE RETRY):
  [read-source]      ⏭️ SKIP (cached from iteration 1)
  [patch-tasklist]   ⏭️ SKIP (cached from iteration 1)
  [generate-test]    🔄 RETRY with augmented context + failure analysis
  [write-test]       🔄 RERUN (depends on generate-test output)
  [compile-check]    🔄 RERUN (depends on write-test)
  [run-tests]        🔄 RERUN (depends on compile-check)
```

The `read-source` and `patch-tasklist` nodes are deterministic — their output is cached and reused. Only the generative chain is re-executed.

This means:
- **No wasted LLM calls** for steps that were correct
- **Faster iterations** — only the failing sub-graph re-executes
- **Template stability** — the AST-anchored `file_patch` insertion doesn't risk corruption on retry

---

## Dependencies

### Depends On
- **Planning Pipeline**: Re-plans with augmented intent
- **Execution Engine**: Executes templates (or partial sub-graphs)
- **Quality Gates**: Evaluates whether validation succeeded
- **Failure Parser**: Parses compiler/test output into typed failures
- **LLM Step**: Provides `llm_generate` node type for generative content
- **Recovery Recipes**: Pattern to follow (one-attempt-before-escalation, typed scenarios)
- **Budget Tracking**: Cumulative token budget across iterations

### Used By
- **Orchestrator**: Invokes validation loop instead of raw plan→execute
- **Template Generation**: Successful templates are cached as validated assets

---

## Validation Report

After validation completes (success or failure), a structured report is produced:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationReport {
    pub execution_id: Uuid,
    pub outcome: ValidationOutcome,
    pub iterations: u32,
    pub total_duration_ms: u64,
    pub cumulative_tokens: u64,
    pub failure_history: Vec<ValidationIterationReport>,

    /// The final validated template (if successful).
    pub validated_template: Option<Template>,

    /// The validated llm_generate prompt (reusable for future executions).
    pub reusable_prompt: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ValidationIterationReport {
    pub iteration: u32,
    pub failures: Vec<TemplateFailure>,
    pub llm_tokens_used: u64,
    pub duration_ms: u64,

    /// What was fixed from the previous iteration.
    pub fixes_applied: Vec<String>,
}
```

---

## Integration with Orchestrator

The orchestrator's `run()` method uses the validation loop instead of raw plan→execute:

```rust
// Before (current):
let plan = planning_pipeline.plan_with_graph(...).await?;
let result = execution_service.execute_graph(...).await?;

// After (with validation):
let validation_outcome = validation_loop.validate(
    user_intent,
    ValidationLoopConfig {
        max_iterations: 3,
        required_quality: QualityLevel::Package,
        ..Default::default()
    },
).await?;

match validation_outcome {
    ValidationOutcome::Validated { report, template } => {
        // Template is production-grade — cache it
        template_service.cache_validated_template(&template).await?;
    }
    ValidationOutcome::Failed { report, .. } => {
        // All retries exhausted — report to user with failure history
        return Err(OrchestratorError::ValidationFailed {
            iterations: report.iterations,
            failures: report.failure_history,
        });
    }
}
```

---

## Security Considerations

| Concern | Mitigation |
|---------|------------|
| Infinite validation loop | `max_iterations` cap (default: 3); cumulative budget exhaustion |
| LLM generates malicious code on retry | Same permission/risk gating applies to all llm_generate outputs |
| Context augmentation reveals sensitive source | Source context is workspace-scoped; no external file access |
| Cached template replay with stale context | Cache is invalidated when source files change (hash-based) |

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 90% | `engine/src/plan_validation/` — per-component test modules |
| Integration | 85% | Mock planning + execution + failure parser cycle |
| E2E | 80% | Full TypeScript demo: intent → validate → corrected template |

**Key Test Scenarios:**
- First iteration succeeds → `Validated` returned immediately
- Compile error → parsed, augmented, retried → second iteration succeeds
- Test failure → parsed, augmented, retried → second iteration succeeds
- Three iterations all fail → `Failed` with full failure history
- Selective retry only re-executes generative sub-graph
- Deterministic node outputs cached across iterations
- Cumulative token budget exhaustion → abort
- Repeated identical failures → escalate (LLM not learning)

---

*Last updated: 2026-06-19*
*Module version: 1.0.0*

---

**Status:** Implemented
**Last verified:** 2026-06-19
**Module version:** 1.0.0
