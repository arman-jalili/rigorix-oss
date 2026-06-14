# Runbook: planning-pipeline Module

<!--
Canonical Reference: .pi/architecture/modules/planning-pipeline.md
Last Updated: 2026-06-14
-->

## Overview

The `planning-pipeline` module orchestrates the LLM-based planning flow from user intent
to validated plan. It executes a 6-phase pipeline: budget check → intent classification →
parameter extraction → TaskGraph generation → plan validation → deterministic hash computation.

## Components

| Component | Type | Description |
|-----------|------|-------------|
| `PlanningPipelineService` | Application trait | 6-phase orchestration: plan, plan_with_graph, classify, extract, validate |
| `PlanningPipelineImpl` | Service impl | Concrete orchestrator with classifier, extractor, template engine |
| `PlanningPipelineFactory` | Application trait | Constructs pipeline with classifier, extractor, generator, validator |
| `PlanningPipelineFactoryImpl` | Factory impl | Creates pipeline instances with injected dependencies |
| `Classifier` | Domain trait | LLM-based intent-to-template classification |
| `MockClassifier` | Test double | Deterministic classifier for CI/testing |
| `ClaudeClassifier` | LLM impl | Anthropic Claude Messages API classifier |
| `OpenaiClassifier` | LLM impl | OpenAI Chat Completions API classifier |
| `ParameterExtractor` | Domain trait | LLM-based parameter extraction from intent |
| `MockParameterExtractor` | Test double | Deterministic extractor for CI/testing |
| `TemplateGenerator` | Domain trait | Fallback template generation for low-confidence intents |
| `CompositeValidator` | Application trait | Optional plan validation before execution |
| `UserIntent` | Domain entity | Raw user input with clarification history |
| `PlanningResult` | Domain entity | Deterministic output: template, confidence, params, hash |
| `PlanningHash` | Domain value | SHA-256 deterministic replay identifier |
| `PlanningError` | Domain error | Typed errors: BudgetExhausted, NoMatchingTemplate, MissingParameter, ValidationFailed, ClassificationError, ExtractionError, Cancelled |

### Pipeline Phases

| Phase | Method | Description |
|-------|--------|-------------|
| 1. Budget Pre-check | `check_budget()` | Ensures ≥2 LLM calls remain |
| 2. Intent Classification | `classify_intent()` | Matches intent to template via Classifier |
| 3. Parameter Extraction | `extract_parameters()` | Fills template parameters via ParameterExtractor |
| 4. Graph Generation | `generate_graph()` | Produces TaskGraph via TemplateEngine |
| 5. Plan Validation | `validate_plan()` | Validates via CompositeValidator |
| 6. Hash Computation | `compute_planning_hash()` | SHA-256 of (template + params + intent) |

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| TemplateEngineService | Yes | Template registry for graph generation |
| Classifier | Yes | LLM-based intent classification (Mock/Claude/OpenAI) |
| ParameterExtractor | Yes | LLM-based parameter extraction |
| TemplateGenerator | No | Optional fallback for low-confidence intents |
| CompositeValidator | No | Optional plan validation |
| LlmBudget | Yes | LLM call/token budget tracking |

### Initialization

```rust
// 1. Create classifier
let classifier = ClaudeClassifier::new(api_key, None);

// 2. Create parameter extractor
let extractor = MockParameterExtractor::new() // or ClaudeParameterExtractor
    .with_default("target", "/tmp/default.txt");

// 3. Create template engine
let parser = TemplateParserImpl::new(repository);
let engine = TemplateEngineImpl::new(parser);

// 4. Build pipeline (via factory)
let factory = PlanningPipelineFactoryImpl::new();
let pipeline = factory.create_default(
    Box::new(classifier),
    Box::new(extractor),
    Box::new(engine),
).await?;
```

## Graceful Shutdown

The PlanningPipelineImpl is stateless (all dependencies are injected at construction).
No explicit shutdown is required. The underlying HTTP clients (reqwest) handle
connection draining automatically on Drop.

For active planning operations during shutdown:
1. Outstanding LLM calls will complete or timeout based on their timeout config
2. The `Cancelled` error variant is returned when the pipeline detects a cancellation signal
3. Budget reservations (via budget_tracking) are rolled back on Drop

## Common Failure Modes

### Budget Exhaustion
**Symptom:** `PlanningError::BudgetExhausted`
**Cause:** No LLM call capacity remaining.
**Recovery:**
1. Check the budget tracking module for current usage
2. Increase budget limits if appropriate
3. Retry after budget reset

### No Matching Template
**Symptom:** `PlanningError::NoMatchingTemplate`
**Cause:** Classifier couldn't match intent to any registered template.
**Recovery:**
1. Ensure templates are registered in the TemplateEngine
2. Enable generator fallback (`enable_generator_fallback: true`)
3. Check classifier configuration (API key, model, endpoint)

### Classification Error
**Symptom:** `PlanningError::ClassificationError`
**Cause:** LLM API failure (network, auth, rate limit).
**Recovery:**
1. Check API key validity and permissions
2. Verify network connectivity to LLM endpoint
3. Check rate limits and retry with backoff
4. Fall back to MockClassifier for offline/CI mode

### Missing Parameter
**Symptom:** `PlanningError::MissingParameter`
**Cause:** Required template parameter could not be extracted from intent.
**Recovery:**
1. User should provide more context in their intent
2. Check ParameterExtractor configuration
3. Ensure template parameter definitions have clear descriptions

### LLM API Timeout
**Symptom:** ClassificationError wrapping a reqwest timeout
**Cause:** LLM endpoint is slow or unreachable.
**Recovery:**
1. Increase `timeout_secs` in classifier config
2. Check network connectivity
3. Use MockClassifier for offline environments

## Configuration Reference

### ClaudeClassifierConfig

| Field | Default | Description |
|-------|---------|-------------|
| `api_url` | `https://api.anthropic.com/v1/messages` | Anthropic API endpoint |
| `model` | `claude-sonnet-4-20250514` | Claude model ID |
| `max_tokens` | 1024 | Maximum response tokens |
| `timeout_secs` | 30 | Request timeout |
| `temperature` | 0.1 | Classification temperature (low = deterministic) |

### OpenaiClassifierConfig

| Field | Default | Description |
|-------|---------|-------------|
| `api_url` | `https://api.openai.com/v1/chat/completions` | OpenAI API endpoint |
| `model` | `gpt-4o` | OpenAI model ID |
| `max_tokens` | 1024 | Maximum response tokens |
| `timeout_secs` | 30 | Request timeout |
| `temperature` | 0.1 | Classification temperature (low = deterministic) |

### PlanInput

| Field | Default | Description |
|-------|---------|-------------|
| `intent` | required | UserIntent with input and clarifications |
| `execution_id` | auto-generated | Correlation ID |
| `enable_generator_fallback` | true | Whether to use TemplateGenerator on low confidence |
| `skip_validation` | false | Skip composite validation (development only) |

### Generator Fallback Controls

| Setting | Default | Description |
|---------|---------|-------------|
| `MAX_GENERATOR_ATTEMPTS` | 3 | Maximum generator fallback retries per plan() call |
| `enable_generator_fallback` | true | Enable/disable generator fallback path |

## LLM Call Budget

The pipeline consumes LLM calls minimally:

| Phase | Calls | Tokens (est.) |
|-------|-------|---------------|
| Budget check | 0 | 0 |
| Classify | 1 | ~200 |
| Extract | 1 | ~100 |
| Generate (fallback) | 1 | ~200 |

Minimum budget required: **2 calls** for classify + extract.
With generator fallback: **3 calls**.

## CI Integration

- **Stage 23** in hardening pipeline: `stage_planning-pipeline_proofing.sh`
- `check_planning-pipeline_contracts.sh`: Validates all 30+ contract points
- `check_planning-pipeline_coverage.sh`: Enforces minimum 80% coverage
- All scripts exit 0 on pass, 1 on fail

## Testing

| Test Type | Count | Coverage Target |
|-----------|-------|-----------------|
| Unit tests | 57 | ≥ 90% (module), ≥ 80% (overall) |
| Integration tests | — | TBD (end-to-end pipeline) |

### Key Test Scenarios
- **Happy path:** classify → extract → validate → PlanningResult
- **Low confidence without generator:** returns ClassificationError
- **Low confidence with generator:** generates and re-runs classification
- **Budget exhaustion:** returns PlanningError::BudgetExhausted
- **Missing parameter:** returns MissingParameter error
- **Ambiguous intent:** triggers clarification path
