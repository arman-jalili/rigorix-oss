# Disaster Recovery Plan: planning-pipeline Module

<!--
Canonical Reference: .pi/architecture/modules/planning-pipeline.md
Last Updated: 2026-06-14
-->

## Scope

This DR plan covers the `planning-pipeline` module — the LLM-based orchestrator
that converts user intent into validated execution plans. The planning pipeline
is stateless at construction and fully in-memory during planning operations.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 30 seconds | Pipeline is stateless — constructed fresh with injected dependencies |
| RPO (Recovery Point Objective) | 0 (in-memory only) | Planning state is ephemeral per execution; no persistent state |

## Backup Strategy

**No backups required — planning pipeline state is ephemeral per execution.**

The planning pipeline operates entirely in memory:
1. `PlanningPipelineImpl` is stateless after construction — all dependencies injected
2. `UserIntent`, `ClassificationResult`, `PlanningResult` exist only during `plan()` call
3. Planning results are persisted (optional) via `PlanningResultRepository`
4. At execution end, all planning state is discarded

### What Gets Recreated

On pipeline creation, the following is built fresh:

| Component | Source | Recovery Method |
|-----------|--------|-----------------|
| Classifier | Constructor arg | Provided by caller |
| ParameterExtractor | Constructor arg | Provided by caller |
| TemplateEngineService | Constructor arg | Provided by caller |
| TemplateGenerator | Optional constructor arg | Provided by caller |
| CompositeValidator | Optional constructor arg | Provided by caller |
| Execution ID | `Uuid::new_v4()` | Generated fresh per pipeline instance |

### What Can Be Persisted (Optional)

| Artifact | Repository | Recovery Use |
|----------|------------|--------------|
| `PlanningResult` | `PlanningResultRepository` | Audit replay via deterministic hash lookup |
| Planning events | EventBus | Audit trail and debugging |

## Restore Procedure

### Scenario: Pipeline Service Failure During Planning

**Symptoms:**
- `plan()` or `plan_with_graph()` returns an error (ClassificationError, ExtractionError)
- Planning result is incomplete or lost

**Recovery Steps:**

1. **Preserve diagnostic information:**
   - Log the error, execution_id, and intent input
   - Capture the pipeline phase where failure occurred

2. **Reconstruct pipeline:**
   - Create a fresh `PlanningPipelineImpl` with the same dependencies
   - Generate a new execution_id

3. **Resubmit intent:**
   - Re-create the `UserIntent` from the original input
   - Call `plan()` or `plan_with_graph()` again

4. **Verify:**
   - Check the new execution produces a valid `PlanningResult`
   - Verify the planning_hash matches expected value for same inputs

### Scenario: LLM API Outage

**Symptoms:**
- ClassificationError wrapping a reqwest error (timeout, connection refused, 5xx)

**Recovery Steps:**

1. **Verify API endpoint availability:**
   ```bash
   curl -I https://api.anthropic.com/v1/messages  # for Claude
   curl -I https://api.openai.com/v1/chat/completions  # for OpenAI
   ```

2. **Fallback options (in order of preference):**
   a. Switch to alternate LLM provider (Claude → OpenAI or vice versa)
   b. Increase `timeout_secs` in classifier config
   c. Use `MockClassifier` for offline/CI mode with deterministic responses

3. **Reconfigure and retry:**
   ```rust
   let classifier = if use_mock {
       Box::new(MockClassifier::new().with_match("read", "template-read", 0.95))
   } else {
       Box::new(ClaudeClassifier::new(backup_api_key, Some(config)))
   };
   let pipeline = factory.create_default(classifier, extractor, engine)?;
   pipeline.plan(plan_input).await?;
   ```

### Scenario: Corrupted Template Registry

**Symptoms:**
- TemplateEngineError during `generate_graph()` phase
- Template lookup failures

**Recovery Steps:**

1. **Verify template registration:**
   - Check `list_templates()` returns expected templates
   - Re-register built-in templates via `load_builtins()`

2. **Rebuild template engine:**
   ```rust
   let parser = TemplateParserImpl::new(repository);
   let engine = TemplateEngineImpl::new(parser);
   engine.load_builtins(LoadBuiltinsInput::default()).await?;
   ```

3. **Retry planning with fresh engine**

## Failover Plan

### Primary ↔ Failover Strategy

| Component | Primary | Failover | Switch Time |
|-----------|---------|----------|-------------|
| Classifier | ClaudeClassifier → Claude API | OpenaiClassifier → OpenAI API | Instant (re-create pipeline) |
| Classifier | Any LLM | MockClassifier | Instant (no network needed) |
| ParameterExtractor | MockParameterExtractor | MockParameterExtractor | Instant |
| TemplateEngine | Built-in templates | Re-loaded from disk | < 100ms |
| Generator | TemplateGenerator | Disabled (no fallback) | Instant |

### Health Checks

The following conditions indicate a healthy pipeline:

1. **Construction succeeds:** `factory.create_default(...)` returns Ok
2. **Classification works:** `classifier.classify_with_alternatives(...)` returns Ok
3. **Template listing works:** `template_service.list_templates()` returns Ok
4. **Plan executes:** `pipeline.plan()` returns Ok for valid input

### Monitoring

| Metric | Source | Alert Threshold |
|--------|--------|-----------------|
| Planning failure rate | Pipeline errors | > 5% over 5 minutes |
| Classification latency | LLM API calls | > 30s average |
| Budget exhaustion rate | Budget tracking | > 1 per minute |
| No-matching-template rate | Pipeline errors | > 10% over 5 minutes |

## Testing

### DR Drill Schedule

| Drill | Frequency | Success Criteria |
|-------|-----------|-----------------|
| LLM API failure simulation | Monthly | Pipeline falls back to MockClassifier successfully |
| Pipeline reconstruction | Monthly | Fresh pipeline produces same result for same inputs |
| Budget exhaustion | Monthly | Pipeline returns BudgetExhausted error gracefully |
| Generator fallback | Quarterly | Generator fallback path produces valid plan |

### DR Drill Procedure

```bash
# 1. Run full test suite
cargo test --lib planning::tests

# 2. Verify contract checks pass
bash .pi/scripts/ci/check_planning-pipeline_contracts.sh

# 3. Verify hardening stage runs
bash .pi/scripts/ci/stage_planning-pipeline_proofing.sh
```
