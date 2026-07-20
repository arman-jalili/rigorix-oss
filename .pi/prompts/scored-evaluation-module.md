# Scored Evaluation Module — Guardian Agent Prompt

> **Context:** Rigorix is adding a `scored_evaluation` DAG node type — a quality gate primitive that evaluates generated artifacts against multidimensional rubrics before CI. This is a new Clean Architecture module in `engine/src/scored_evaluation/`.

---

## 1. What to Build

A new module `engine/src/scored_evaluation/` following the same Clean Architecture pattern as all 30 existing modules (see `engine/.pi/skills/rust-enterprise-codegen.md` for full pattern reference). The module adds a **scored evaluation node type** to the DAG — when a node runs, it sends an artifact + rubric to a scoring backend and receives a multidimensional score back.

### The Contract (Frozen)

**Input** — what triggers a scored evaluation:
```json
{
  "artifact": "<node output or file content as JSON string>",
  "rubric": {
    "source": "inline | reference",
    "content": "<inline JSON rubric or URL/path to rubric file>",
    "scenario_id": "optional scenario name e.g. de_sql_optimization_v1"
  },
  "context": {
    "execution_id": "<uuid>",
    "node_id": "<uuid>",
    "node_name": "<string>"
  }
}
```

**Output** — what the scoring backend returns:
```json
{
  "passed": true,
  "dimensions": {
    "correctness": { "score": 0.92, "max": 1.0, "label": "Correctness" },
    "completeness": { "score": 0.85, "max": 1.0, "label": "Completeness" },
    "style": { "score": 0.78, "max": 1.0, "label": "Code Style" }
  },
  "summary": "2 of 3 dimensions above threshold. Style dimension at 0.78 (threshold: 0.8).",
  "backend": "runtimeai",
  "duration_ms": 1432,
  "raw": "{ ... backend-specific raw response }"
}
```

**Threshold gating** — policy rules reference score thresholds as u8 percentages (0–100) for `PartialEq, Eq` compatibility on `PolicyCondition`:
```toml
# Example: block merge if any dimension below 80%
[[rules]]
name = "scored-evaluation-gate"
condition = { type = "score_below", dimension = null, threshold = 80 }
action = "block_merge"

# Example: block merge if correctness specifically below 90%
condition = { type = "score_below", dimension = "correctness", threshold = 90 }
```

> **Note:** Backend scoring results use `score: f64` (0.0–1.0) in `ScoreDimension`. At evaluation time, the score is converted to u8 percentage (`(score * 100.0) as u8`) before comparing against the `threshold: u8` field in `PolicyCondition`.

---

## 2. Domain Model

### ScoredEvaluationNode (Value Object)
- `node_id: Uuid` — the DAG node this evaluation is attached to
- `artifact: serde_json::Value` — the generated artifact being evaluated
- `rubric: Rubric` — the evaluation rubric (inline or reference)
- `backend: String` — which scoring backend to use (e.g. "runtimeai", "local", "http")
- `thresholds: HashMap<String, f64>` — per-dimension minimum scores
- `policy: ExecutionPolicy` — retry/fallback behavior

### Rubric (Value Object)
- `source: RubricSource` — enum: `Inline { content: serde_json::Value }` | `Reference { path_or_url: String }`
- `scenario_id: Option<String>` — named scenario from a scoring backend's library

### ScoringResult (Value Object)
- `passed: bool` — overall pass/fail
- `dimensions: HashMap<String, ScoreDimension>` — per-dimension results
- `summary: String` — human-readable summary
- `backend: String` — which backend produced this result
- `duration_ms: u64` — evaluation execution time
- `raw: Option<serde_json::Value>` — backend-specific raw response (for audit)

### ScoreDimension (Value Object)
- `score: f64` — numeric score (0.0–1.0)
- `max: f64` — maximum possible score (typically 1.0)
- `label: String` — human-readable dimension name
- `passed: bool` — whether this specific dimension met its threshold

### ScoringBackend (Trait — in domain, implemented in infrastructure)
```rust
pub trait ScoringBackend: Send + Sync {
    fn evaluate(&self, artifact: &serde_json::Value, rubric: &Rubric) -> Result<ScoringResult, ScoredEvaluationError>;
    fn backend_name(&self) -> &'static str;
    fn health_check(&self) -> Result<bool, ScoredEvaluationError>;
}
```

### ScoredEvaluationEvent (Domain Events)
- `ScoredEvaluationStarted { node_id: String, execution_id: Uuid, backend: String, timestamp }` (node_id is String to match existing `ExecutionEvent` convention)
- `ScoredEvaluationCompleted { node_id: String, execution_id: Uuid, result: ScoringResult, timestamp }`
- `ScoredEvaluationFailed { node_id: String, execution_id: Uuid, error: String, timestamp }`

---

## 3. Clean Architecture Structure

```
engine/src/scored_evaluation/
├── mod.rs                    # Module root, re-exports, contract freeze header
├── domain/
│   ├── mod.rs                # Re-exports + module-level documentation
│   ├── node.rs               # ScoredEvaluationNode value object
│   ├── rubric.rs             # Rubric value object + RubricSource enum
│   ├── result.rs             # ScoringResult + ScoreDimension value objects
│   ├── backend.rs            # ScoringBackend trait (domain interface)
│   ├── event.rs              # ScoredEvaluationEvent domain events
│   └── error.rs              # ScoredEvaluationError (thiserror)
├── application/
│   ├── mod.rs
│   ├── service.rs            # ScoredEvaluationService trait
│   └── dto/
│       └── mod.rs            # EvaluateInput, EvaluateOutput DTOs with validation
└── infrastructure/
    ├── mod.rs
    ├── repository/
    │   ├── mod.rs
    │   └── evaluation_repository.rs   # Persist evaluation results
    └── backends/
        ├── mod.rs
        ├── mcp_backend.rs             # MCP-based backend (RuntimeAI compatible)
        ├── http_backend.rs            # HTTP-based backend
        └── local_backend.rs           # Local script/file-based backend
```

**Note:** No `interfaces/` directory initially — the module exposes its API through the application service trait. HTTP/MCP interfaces for direct invocation live in the MCP crate. This follows the same pattern as `quality_gates` which has no interfaces layer.

---

## 4. Integration Points (Modify Existing Modules)

### 4.1 `dag_engine` — New Node Tool Type
- `TaskNode.tool: String` uses raw string values (e.g. `"cargo build"`, `"npm test"`). Add `"scored_evaluation"` as a recognized tool string in TaskNode validation — validated via string matching at node construction time.
- If a `NodeTool` enum is desired in a future refactor, `ScoredEvaluation` would be a variant. Until then, `tool == "scored_evaluation"`.
- Template YAML syntax:
```yaml
- id: score_output
  action: scored_evaluation
  depends_on: [generate_patch]
  params:
    artifact: "{{ generate_patch.output }}"
    rubric_source: inline
    rubric:
      correctness:
        description: "Does the code do what was intended?"
        threshold: 0.8
      completeness:
        description: "Are all requirements covered?"
        threshold: 0.8
    backend: runtimeai
  policy:
    on_failure: flag_for_review
```

### 4.2 `policy_engine` — New Condition Variant
`PolicyCondition` derives `PartialEq, Eq`. Since `f64` does not implement `Eq`, thresholds use `u8` percentage (0–100), matching the existing `GreenAt { level: u8 }` convention.

Add to `engine/src/policy_engine/domain/condition.rs`:
```rust
/// Score gating — all dimensions above a threshold (percentage 0–100).
ScoreAbove {
    /// Optional: only check this specific dimension. None = all dimensions.
    dimension: Option<String>,
    /// Minimum score threshold as percentage (0–100). E.g., 80 = 80%.
    threshold: u8,
},
/// Score gating — any dimension below a threshold (percentage 0–100).
ScoreBelow {
    /// Optional: only check this specific dimension. None = any dimension.
    dimension: Option<String>,
    /// Minimum score threshold as percentage (0–100). E.g., 80 = 80%.
    threshold: u8,
},
```

**Conversion:** Backend `ScoreDimension.score: f64` (0.0–1.0) is converted to u8 percentage at evaluation time: `(score * 100.0) as u8`.

### 4.3 `audit` — Envelope Extension
Add to `engine/src/audit/domain/envelope.rs`:
```rust
/// Scoring results from scored_evaluation nodes, keyed by node_id.
#[serde(default, skip_serializing_if = "HashMap::is_empty")]
pub scoring_results: HashMap<String, ScoringResultRef>,
```
Where `ScoringResultRef` is a lightweight reference with dimensions, passed flag, and backend name.

### 4.4 `event_system` — Event Type Extension
Add to `engine/src/event_system/domain/event.rs`. Uses `node_id: String` to match the existing `ExecutionEvent` convention where all node-related variants (`NodeStarted`, `NodeCompleted`, `NodeFailed`, `NodeRetrying`, `ToolExecuted`) use `node_id: String`:
```rust
ScoredEvaluationStarted {
    execution_id: uuid::Uuid,
    node_id: String,
    backend: String,
    timestamp: chrono::DateTime<chrono::Utc>,
},
ScoredEvaluationCompleted {
    execution_id: uuid::Uuid,
    node_id: String,
    node_name: String,
    passed: bool,
    dimensions_count: usize,
    timestamp: chrono::DateTime<chrono::Utc>,
},
ScoredEvaluationFailed {
    execution_id: uuid::Uuid,
    node_id: String,
    error: String,
    timestamp: chrono::DateTime<chrono::Utc>,
},
```

### 4.5 `quality_gates` — Cross-Reference (No Changes Needed)
Scored evaluation is orthogonal to GreenContract — it evaluates output quality, not test scope. They complement each other:
- GreenContract: "did we run broadly enough?" (scope)
- ScoredEvaluation: "did we generate correctly?" (quality)
Both feed into the policy engine independently.

---

## 5. Research Context — RuntimeAI

The first scoring backend is RuntimeAI by Vantage (`vantageai.cc/runtimeai`). Their MCP tools:

| Tool | Purpose |
|------|---------|
| `runtimeai_suggest_scenario` | Map a file/context to best library scenario |
| `runtimeai_generate_scenario` | Draft custom check-ride JSON from code context |
| `runtimeai_run_checkride` | Execute evaluation, return pass/fail + scores |
| `runtimeai_forecast_cost` | Estimate cost before running |

Integration flow:
1. Template author defines a scored_evaluation node with `backend: runtimeai`
2. At execution time, the node calls `runtimeai_run_checkride` via MCP with the artifact + rubric
3. RuntimeAI returns scored result
4. The result is persisted, emitted as domain event, gated by policy, and embedded in the audit envelope

The MCP backend adapter translates the `ScoringBackend` trait into `runtimeai_run_checkride` MCP calls.

---

## 6. Guardian Build Checklist

After scaffolding with Guardian, ensure:

- [ ] Module follows Clean Architecture: domain → application → infrastructure
- [ ] All domain types derive `Debug, Clone, Serialize, Deserialize`
- [ ] `ScoredEvaluationError` uses `thiserror::Error` (codebase convention: `use thiserror::Error;` + `#[derive(Debug, Error)]`)
- [ ] `ScoredEvaluationError` implements `is_retriable()` matching `QualityGateError` pattern
- [ ] `ScoringBackend` trait is in domain, implementations in infrastructure
- [ ] Every `mod.rs` has canonical reference header
- [ ] Module spec written to `engine/.pi/architecture/modules/scored-evaluation.md`
- [ ] Contract freeze annotations on all public types
- [ ] Serde round-trip tests for `ScoringResult`, `Rubric`, `ScoreDimension`
- [ ] Contract tests for `ScoringBackend` trait implementations
- [ ] Proofing scripts: `check_scored-evaluation_contracts.sh` + `check_scored-evaluation_coverage.sh`
- [ ] Integration tests for policy condition `ScoreAbove`/`ScoreBelow`
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` zero warnings

---

## 7. Module Spec Template

Write the module spec to `engine/.pi/architecture/modules/scored-evaluation.md`:

```markdown
# Scored Evaluation Architecture

## Overview
The Scored Evaluation system adds a scored quality evaluation primitive to the Rigorix DAG.
A `scored_evaluation` node sends generated artifacts to a pluggable scoring backend and
receives multidimensional scores back. Policy rules can gate merge on score thresholds.

This complements the Quality Gates system (GreenContract), which evaluates test scope
(TargetedTests → MergeReady), by adding output quality scoring as an orthogonal dimension.

## Adoption Rationale
- Multidimensional scoring of AI-generated artifacts before CI
- Pluggable backends (MCP/HTTP/local) — no vendor lock-in
- Policy integration: deny merge if any dimension below threshold
- Audit envelope carries scoring results for compliance provenance
- First backend: RuntimeAI (MCP-native, scoring via check-rides + rubrics)

## Responsibilities
- Define scored evaluation node type for DAG integration
- Define ScoringBackend trait for pluggable evaluation backends
- Execute evaluations and capture multidimensional results
- Emit scored evaluation events for audit trail
- Integrate with policy engine for score-based gating
- Integrate with audit envelope for compliance reporting

## Components
| Component | File Path | Purpose |
|-----------|-----------|---------|
| ScoredEvaluationNode | domain/node.rs | DAG node value object with artifact + rubric + thresholds |
| Rubric | domain/rubric.rs | Evaluation rubric (inline or reference) |
| ScoringResult | domain/result.rs | Multidimensional score result |
| ScoringBackend | domain/backend.rs | Trait for pluggable evaluation backends |
| ScoredEvaluationEvent | domain/event.rs | Domain events for scoring lifecycle |
| ScoredEvaluationError | domain/error.rs | Typed error enum |
| ScoredEvaluationService | application/service.rs | Service trait for evaluation orchestration |
| MCPBackend | infrastructure/backends/mcp_backend.rs | MCP-based scoring backend (RuntimeAI compatible) |

## Dependencies
- Depends on: dag_engine, execution_engine, policy_engine, audit, event_system
- Used by: orchestrator (node execution), policy_engine (score gating), audit (envelope)
```

---

## 8. Additional Context

### Existing patterns in the codebase this module should follow:
- **Quality Gates** (`engine/src/quality_gates/`) — same 4-tier pattern, domain-first, trait-based service
- **Policy Engine** (`engine/src/policy_engine/`) — condition enum with `#[serde(tag = "type")]` for TOML serialization
- **Audit** (`engine/src/audit/`) — envelope value object with `#[serde(default, skip_serializing_if)]` for optional fields
- **DAG Engine** (`engine/src/dag_engine/`) — TaskNode with `tool: String` field, validation rules

### The MCP crate integration:
The MCP crate (`mcp/src/`) wraps engine capabilities for MCP protocol exposure. The `mcp_backend.rs` infrastructure adapter in this module calls the MCP server's tool dispatch. It does NOT live in the MCP crate — it's an infrastructure implementation of the `ScoringBackend` trait from the engine's domain layer.

### Test expectations:
- Minimum 50 tests per module (per existing proofing standards)
- Unit tests for all value objects, error types, event types
- Serde round-trip tests for every serialized type
- Contract tests for `ScoringBackend` trait
- Integration tests for policy condition variants
- Mock backend for testing without external dependencies
