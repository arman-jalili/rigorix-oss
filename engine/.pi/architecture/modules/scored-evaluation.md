# Scored Evaluation Architecture

<!--
Canonical Reference: .pi/architecture/modules/scored-evaluation.md
Blueprint Source: Guardian Agent Prompt — Scored Evaluation Module
-->

## Overview

The Scored Evaluation system adds a scored quality evaluation primitive to the Rigorix DAG. A `scored_evaluation` node sends generated artifacts to a pluggable scoring backend and receives multidimensional scores back. Policy rules can gate merge on score thresholds.

This complements the Quality Gates system (GreenContract), which evaluates test scope (TargetedTests → MergeReady), by adding output quality scoring as an orthogonal dimension.

## Adoption Rationale

- Multidimensional scoring of AI-generated artifacts before CI
- Pluggable backends (MCP/HTTP/local) — no vendor lock-in
- Policy integration: deny merge if any dimension below threshold
- Audit envelope carries scoring results for compliance provenance
- First backend: RuntimeAI (MCP-native, scoring via check-rides + rubrics)

## Responsibilities

- Define scored evaluation node type for DAG integration
- Define `ScoringBackend` trait for pluggable evaluation backends
- Execute evaluations and capture multidimensional results
- Emit scored evaluation events for audit trail
- Integrate with policy engine for score-based gating
- Integrate with audit envelope for compliance reporting

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| ScoredEvaluationNode | `engine/src/scored_evaluation/domain/node.rs` | DAG node value object with artifact + rubric + thresholds | #node |
| Rubric | `engine/src/scored_evaluation/domain/rubric.rs` | Evaluation rubric (inline or reference) | #rubric |
| ScoringResult | `engine/src/scored_evaluation/domain/result.rs` | Multidimensional score result with passed flag | #result |
| ScoreDimension | `engine/src/scored_evaluation/domain/result.rs` | Single dimension: score, max, label, passed | #dimension |
| ScoringBackend | `engine/src/scored_evaluation/domain/backend.rs` | Trait for pluggable evaluation backends | #backend |
| ScoredEvaluationEvent | `engine/src/scored_evaluation/domain/event.rs` | Domain events for scoring lifecycle | #event |
| ScoredEvaluationError | `engine/src/scored_evaluation/domain/error.rs` | Typed error enum (thiserror) | #error |
| ScoredEvaluationService | `engine/src/scored_evaluation/application/service.rs` | Service trait for evaluation orchestration | #service |
| EvaluateInput | `engine/src/scored_evaluation/application/dto/mod.rs` | Input DTO: artifact + rubric + context | #dto |
| EvaluateOutput | `engine/src/scored_evaluation/application/dto/mod.rs` | Output DTO: ScoringResult + metadata | #dto |
| MCPBackend | `engine/src/scored_evaluation/infrastructure/backends/mcp_backend.rs` | MCP-based scoring backend (RuntimeAI compatible) | #mcp_backend |
| HTTPBackend | `engine/src/scored_evaluation/infrastructure/backends/http_backend.rs` | HTTP-based scoring backend | #http_backend |
| LocalBackend | `engine/src/scored_evaluation/infrastructure/backends/local_backend.rs` | Local script/file-based backend | #local_backend |
| EvaluationRepository | `engine/src/scored_evaluation/infrastructure/repository/evaluation_repository.rs` | Persist evaluation results | #repository |

---

## Component Details

### ScoredEvaluationNode

**Purpose:** DAG node value object holding artifact, rubric, backend config, and thresholds

**Implementation File:** `engine/src/scored_evaluation/domain/node.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoredEvaluationNode {
    pub node_id: Uuid,
    pub artifact: serde_json::Value,
    pub rubric: Rubric,
    pub backend: String,
    pub thresholds: HashMap<String, f64>,
    pub policy: ExecutionPolicy,
}
```

### Rubric

**Purpose:** Evaluation rubric — either inline JSON content or a reference to an external file/URL

**Implementation File:** `engine/src/scored_evaluation/domain/rubric.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rubric {
    pub source: RubricSource,
    pub scenario_id: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum RubricSource {
    Inline { content: serde_json::Value },
    Reference { path_or_url: String },
}
```

### ScoringResult

**Purpose:** Multidimensional scoring result returned by a scoring backend

**Implementation File:** `engine/src/scored_evaluation/domain/result.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringResult {
    pub passed: bool,
    pub dimensions: HashMap<String, ScoreDimension>,
    pub summary: String,
    pub backend: String,
    pub duration_ms: u64,
    pub raw: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoreDimension {
    pub score: f64,
    pub max: f64,
    pub label: String,
    pub passed: bool,
}
```

### ScoringBackend (Trait)

**Purpose:** Pluggable scoring backend interface — domain-level contract for any evaluation backend

**Implementation File:** `engine/src/scored_evaluation/domain/backend.rs`

```rust
#[async_trait]
pub trait ScoringBackend: Send + Sync {
    async fn evaluate(&self, artifact: &serde_json::Value, rubric: &Rubric) -> Result<ScoringResult, ScoredEvaluationError>;
    fn backend_name(&self) -> &'static str;
    async fn health_check(&self) -> Result<bool, ScoredEvaluationError>;
}
```

### ScoredEvaluationEvent

**Purpose:** Domain events for the evaluation lifecycle — started, completed, failed

**Implementation File:** `engine/src/scored_evaluation/domain/event.rs`

Uses `node_id: String` to match the existing `ExecutionEvent` pattern (see `engine/src/event_system/domain/event.rs` where all node-related variants use `node_id: String` rather than `uuid::Uuid`).

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ScoredEvaluationEvent {
    ScoredEvaluationStarted {
        node_id: String,
        execution_id: Uuid,
        backend: String,
        timestamp: DateTime<Utc>,
    },
    ScoredEvaluationCompleted {
        node_id: String,
        execution_id: Uuid,
        result: ScoringResult,
        timestamp: DateTime<Utc>,
    },
    ScoredEvaluationFailed {
        node_id: String,
        execution_id: Uuid,
        error: String,
        timestamp: DateTime<Utc>,
    },
}
```

### ScoredEvaluationError

**Purpose:** Typed error enum for all scored evaluation failure modes

**Implementation File:** `engine/src/scored_evaluation/domain/error.rs`

Follows the same pattern as `QualityGateError` (`engine/src/quality_gates/domain/error.rs`): `use thiserror::Error;` import + `#[derive(Debug, Error)]` derive, with an `is_retriable()` method for execution policy integration.

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScoredEvaluationError {
    #[error("Backend not found: {0}")]
    BackendNotFound(String),
    #[error("Backend error: {0}")]
    BackendError(String),
    #[error("Invalid rubric: {0}")]
    InvalidRubric(String),
    #[error("Invalid artifact: {0}")]
    InvalidArtifact(String),
    #[error("Backend health check failed: {0}")]
    BackendUnavailable(String),
    #[error("Timeout: backend did not respond within {0}ms")]
    Timeout(u64),
    #[error("Internal error: {0}")]
    Internal(String),
}

impl ScoredEvaluationError {
    /// Returns `true` if this error represents a transient condition
    /// that might succeed on retry.
    pub fn is_retriable(&self) -> bool {
        matches!(
            self,
            ScoredEvaluationError::BackendError(_)
                | ScoredEvaluationError::BackendUnavailable(_)
                | ScoredEvaluationError::Timeout(_)
        )
    }
}
```

### ScoredEvaluationService

**Purpose:** Service trait orchestrating the evaluation lifecycle — validate input, delegate to backend, capture result, emit events

**Implementation File:** `engine/src/scored_evaluation/application/service.rs`

```rust
#[async_trait]
pub trait ScoredEvaluationService: Send + Sync {
    async fn evaluate(&self, input: EvaluateInput) -> Result<EvaluateOutput, ScoredEvaluationError>;
    async fn get_evaluation(&self, execution_id: Uuid, node_id: Uuid) -> Result<Option<EvaluateOutput>, ScoredEvaluationError>;
    async fn list_evaluations(&self, execution_id: Uuid) -> Result<Vec<EvaluateOutput>, ScoredEvaluationError>;
}
```

### EvaluateInput / EvaluateOutput

**Purpose:** Typed DTOs for the evaluation service boundary

**Implementation File:** `engine/src/scored_evaluation/application/dto/mod.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateInput {
    pub artifact: serde_json::Value,
    pub rubric: Rubric,
    pub context: EvaluationContext,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluationContext {
    pub execution_id: Uuid,
    pub node_id: Uuid,
    pub node_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvaluateOutput {
    pub result: ScoringResult,
    pub execution_id: Uuid,
    pub node_id: Uuid,
    pub node_name: String,
    pub timestamp: DateTime<Utc>,
}
```

### MCPBackend

**Purpose:** MCP-based scoring backend implementation — translates `ScoringBackend::evaluate` into `runtimeai_run_checkride` MCP calls

**Implementation File:** `engine/src/scored_evaluation/infrastructure/backends/mcp_backend.rs`

**Key behavior:**
- Calls `runtimeai_run_checkride` MCP tool with artifact + rubric
- Parses MCP response into `ScoringResult`
- Calls `runtimeai_forecast_cost` before evaluation for cost-aware routing (optional)
- Falls back to `runtimeai_generate_scenario` if no scenario_id provided
- Implements health check via `runtimeai_suggest_scenario` with a known test input

### HTTPBackend

**Purpose:** Generic HTTP-based scoring backend for custom evaluation services

**Implementation File:** `engine/src/scored_evaluation/infrastructure/backends/http_backend.rs`

**Key behavior:**
- POSTs artifact + rubric to configurable URL
- Expects `ScoringResult`-compatible JSON response
- Configurable timeout, headers, and auth
- Implements health check via HEAD or GET to health endpoint

### LocalBackend

**Purpose:** Local script/file-based backend for development and testing

**Implementation File:** `engine/src/scored_evaluation/infrastructure/backends/local_backend.rs`

**Key behavior:**
- Executes a local script with artifact + rubric as environment variables
- Reads scoring result from stdout (JSON)
- Configurable script path and timeout
- Implements health check by checking script file existence

---

## Data Flow

```
DAG Execution reaches scored_evaluation node
        │
        ▼
ScoredEvaluationService::evaluate(EvaluateInput)
  ─ Validates artifact + rubric
  ─ Resolves backend by name (MCPBackend / HTTPBackend / LocalBackend)
        │
        ▼
Emits ScoredEvaluationStarted event
        │
        ▼
ScoringBackend::evaluate(artifact, rubric)
  ─ MCPBackend: runtimeai_run_checkride via MCP
  ─ HTTPBackend: POST to scoring service endpoint
  ─ LocalBackend: execute local script
        │
        ├── Success →
        │   ─ Parse response into ScoringResult
        │   ─ Evaluate thresholds against dimensions
        │   ─ Emit ScoredEvaluationCompleted
        │   ─ Persist result via EvaluationRepository
        │   ─ Embed scoring result in audit envelope
        │   ─ Return EvaluateOutput
        │
        └── Failure →
            ─ Emit ScoredEvaluationFailed
            ─ Apply ExecutionPolicy (retry / fallback / flag_for_review)
            ─ Return ScoredEvaluationError
        │
        ▼
Policy Engine evaluates ScoreAbove / ScoreBelow conditions
  ─ If gating rule matches → block_merge / flag_for_review
```

**Flow Description:**
1. DAG execution reaches a `scored_evaluation` node; the orchestrator calls `ScoredEvaluationService::evaluate()`
2. Input is validated and the appropriate `ScoringBackend` is resolved by name
3. A `ScoredEvaluationStarted` event is emitted for audit tracing
4. The backend evaluates the artifact against the rubric:
   - **MCPBackend**: calls `runtimeai_run_checkride` via MCP tool dispatch
   - **HTTPBackend**: POSTs to a configurable evaluation endpoint
   - **LocalBackend**: executes a local script with artifact + rubric
5. On success, the `ScoringResult` is parsed, thresholds are evaluated against each dimension, domain events are emitted, and the result is persisted
6. On failure, retry/fallback policy is applied and errors are propagated
7. The audit envelope is extended with scoring results for compliance provenance
8. The policy engine evaluates `ScoreAbove`/`ScoreBelow` conditions for merge gating

---

## Dependencies

### Depends On
- **DAG Engine**: TaskNode receives `scored_evaluation` as a recognized tool/action string
- **Execution Engine**: Invokes `ScoredEvaluationService` during node execution
- **Policy Engine**: New `ScoreAbove` / `ScoreBelow` condition variants for score-based gating
- **Audit**: Envelope extended with `scoring_results` field for compliance
- **Event System**: `ScoredEvaluationEvent` variants published via EventBus
- **Configuration**: Backend URLs, thresholds, default policies

### Used By
- **Orchestrator**: Invokes scored evaluation during DAG node execution
- **Policy Engine**: Evaluates score-based gating conditions
- **Audit**: Embeds scoring results in audit envelope

---

## Integration with Existing Modules

### DAG Engine — New Node Tool Type

`TaskNode.tool: String` currently uses raw string values (e.g., `"cargo build"`, `"npm test"`). Add `"scored_evaluation"` as a recognized tool string in `TaskNode` validation within `engine/src/dag_engine/domain/graph.rs`. No enum refactoring needed — the tool field is validated against a known set of strings at node construction time.

If a `NodeTool` enum is desired in a future refactor, the `scored_evaluation` variant would be:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum NodeTool {
    // ... existing tools ...
    ScoredEvaluation,
}
```

Until then, validation checks `tool == "scored_evaluation"` as a string match.

Template YAML syntax:
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

### Policy Engine — New Condition Variants

`PolicyCondition` derives `PartialEq, Eq` (`engine/src/policy_engine/domain/condition.rs`). Since `f64` does not implement `Eq`, score thresholds use `u8` percentage (0–100), matching the existing `GreenAt { level: u8 }` convention.

Add to `engine/src/policy_engine/domain/condition.rs`:

```rust
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
// ... existing variants ...

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

TOML configuration:
```toml
# Example: block merge if any dimension below 80%
[[rules]]
name = "scored-evaluation-gate"
condition = { type = "score_below", dimension = null, threshold = 80 }
action = "block_merge"

# Example: block merge if correctness specifically below 90%
condition = { type = "score_below", dimension = "correctness", threshold = 90 }
```

**Conversion:** Backend scoring result dimensions contain `score: f64` (0.0–1.0). At evaluation time, the score is converted to u8 percentage (`(score * 100.0) as u8`) before comparing against the threshold field in `PolicyCondition`. This keeps the policy condition enum Eq-compatible while the raw f64 scores live in `ScoreDimension` in the domain layer.

### Audit — Envelope Extension

Add to `engine/src/audit/domain/envelope.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScoringResultRef {
    pub passed: bool,
    pub backend: String,
    pub dimensions: HashMap<String, ScoreDimension>,
    pub duration_ms: u64,
}

pub struct AuditEnvelope {
    // ... existing fields ...
    /// Scoring results from scored_evaluation nodes, keyed by node_id.
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub scoring_results: HashMap<String, ScoringResultRef>,
}
```

### Event System — Event Type Extension

Add to `engine/src/event_system/domain/event.rs`. Uses `node_id: String` to match the existing `ExecutionEvent` convention — all node-related variants (`NodeStarted`, `NodeCompleted`, `NodeFailed`, `NodeRetrying`, `ToolExecuted`) use `node_id: String`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionEvent {
    // ... existing 11 variants ...

    /// Scored evaluation started
    ScoredEvaluationStarted {
        execution_id: uuid::Uuid,
        node_id: String,
        backend: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Scored evaluation completed successfully
    ScoredEvaluationCompleted {
        execution_id: uuid::Uuid,
        node_id: String,
        node_name: String,
        passed: bool,
        dimensions_count: usize,
        timestamp: chrono::DateTime<chrono::Utc>,
    },

    /// Scored evaluation failed
    ScoredEvaluationFailed {
        execution_id: uuid::Uuid,
        node_id: String,
        error: String,
        timestamp: chrono::DateTime<chrono::Utc>,
    },
}
```

### Quality Gates — Cross-Reference (No Changes Needed)

Scored evaluation is orthogonal to GreenContract — it evaluates output quality, not test scope. They complement each other:
- **GreenContract**: "did we run broadly enough?" (scope)
- **ScoredEvaluation**: "did we generate correctly?" (quality)
Both feed into the policy engine independently.

---

## Configuration

```toml
# .rigorix/scored_evaluation.toml
[scored_evaluation]
default_backend = "runtimeai"

[scored_evaluation.backends.runtimeai]
type = "mcp"
timeout_ms = 30_000

[scored_evaluation.backends.custom_http]
type = "http"
url = "https://evaluate.internal.example.com/api/v1/score"
timeout_ms = 60_000
auth_header = "Bearer ${SCORING_API_KEY}"

[scored_evaluation.backends.local_dev]
type = "local"
script_path = "./scripts/evaluate.sh"
timeout_ms = 10_000

[scored_evaluation.defaults]
# Default score threshold as percentage (0–100). Matches PolicyCondition's u8 threshold.
# Backend scores (0.0–1.0) are converted at evaluation time: (score * 100.0) as u8.
threshold = 80
on_failure = "flag_for_review"
```

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 90% | `engine/src/scored_evaluation/` — per-component test modules |
| Integration | 80% | `engine/src/scored_evaluation/tests/` |

**Key Test Scenarios:**
- Serde round-trip: `ScoringResult`, `Rubric`, `ScoreDimension`, `ScoredEvaluationNode`
- `ScoreDimension.passed` calculation against threshold
- `EvaluateInput` validation (missing artifact, invalid rubric)
- `ScoringBackend` trait contract enforcement (all backends pass same test suite)
- `MCPBackend` behavior with mock MCP server
- `HTTPBackend` behavior with mock HTTP server
- `LocalBackend` behavior with mock script
- `ScoredEvaluationService` orchestrates: validate → backend → event → persist
- `ScoredEvaluationEvent` serialization: all three variants
- `ScoreAbove` / `ScoreBelow` policy condition evaluation
- `AuditEnvelope` scoring_results serialization (empty, populated, round-trip)
- Error propagation: `BackendNotFound`, `BackendError`, `InvalidRubric`, `Timeout`
- Retry policy: `on_failure: retry` vs `flag_for_review` vs `block`
- Health check: all backends report `health_check()` correctly
- Threshold evaluation: all dimensions pass, one fails, multiple fail

**Mock Backend for Testing:**

```rust
// engine/src/scored_evaluation/tests/mock_backend.rs
pub struct MockBackend {
    pub result: ScoringResult,
    pub health: bool,
}

#[async_trait]
impl ScoringBackend for MockBackend {
    async fn evaluate(&self, _: &serde_json::Value, _: &Rubric) -> Result<ScoringResult, ScoredEvaluationError> {
        Ok(self.result.clone())
    }
    fn backend_name(&self) -> &'static str { "mock" }
    async fn health_check(&self) -> Result<bool, ScoredEvaluationError> { Ok(self.health) }
}
```

---

## Security Considerations

| Concern | Mitigation | Validator |
|---------|------------|-----------|
| Remote backend tampering | HMAC-signed payloads for MCP/HTTP backends | security-validator |
| Sensitive data in rubric | Rubric content reviewed; no secrets in `RubricSource::Inline` | security-validator |
| Local script injection | `LocalBackend` validates script path against allowlist | security-validator |
| Backend credential leakage | Auth tokens read from environment, never logged | security-validator |
| Denial of service via long evaluations | Configurable `timeout_ms` per backend, default 30s | operations-validator |

---

## Performance Considerations

| Metric | Target | Strategy |
|--------|--------|----------|
| Evaluation latency (MCP) | < 5s (includes backend round-trip) | Configurable timeout, concurrent execution |
| Evaluation latency (HTTP) | < 10s (includes network + backend) | Configurable timeout, concurrent execution |
| Evaluation latency (Local) | < 2s (subprocess overhead) | Configurable timeout |
| Memory per evaluation | < 1MB (artifact + rubric + result) | Streaming for large artifacts |
| Backend resolution | O(1) | HashMap-backed registry |

---

## Error Handling

```rust
use thiserror::Error;

#[derive(Debug, Error)]
pub enum ScoredEvaluationError {
    #[error("Backend not found: {0}")]
    BackendNotFound(String),
    #[error("Backend error: {0}")]
    BackendError(String),
    #[error("Invalid rubric: {0}")]
    InvalidRubric(String),
    #[error("Invalid artifact: {0}")]
    InvalidArtifact(String),
    #[error("Backend health check failed: {0}")]
    BackendUnavailable(String),
    #[error("Timeout: backend did not respond within {0}ms")]
    Timeout(u64),
    #[error("Internal error: {0}")]
    Internal(String),
}
```

**Recovery (matches `is_retriable()` classification):**
- `BackendNotFound`: Not retriable — misconfiguration; escalate to user
- `BackendError`: Retriable — retry up to `max_retries` with exponential backoff; flag for review on exhaustion
- `InvalidRubric` / `InvalidArtifact`: Not retriable — fail fast; user must fix template or input
- `BackendUnavailable`: Retriable — circuit breaker pattern (see Audit module); queue and retry later
- `Timeout`: Retriable — retry with increased timeout (capped at `max_timeout_ms`); escalate if persistent
- `Internal`: Not retriable — unexpected; escalate

---

## Module Structure

```
engine/src/scored_evaluation/
├── mod.rs                          # Module root: re-exports, contract freeze header
├── domain/
│   ├── mod.rs                      # Re-exports + module-level documentation
│   ├── node.rs                     # ScoredEvaluationNode value object
│   ├── rubric.rs                   # Rubric value object + RubricSource enum
│   ├── result.rs                   # ScoringResult + ScoreDimension value objects
│   ├── backend.rs                  # ScoringBackend trait (domain interface)
│   ├── event.rs                    # ScoredEvaluationEvent domain events
│   └── error.rs                    # ScoredEvaluationError (thiserror)
├── application/
│   ├── mod.rs
│   ├── service.rs                  # ScoredEvaluationService trait
│   └── dto/
│       └── mod.rs                  # EvaluateInput, EvaluateOutput, EvaluationContext DTOs
└── infrastructure/
    ├── mod.rs
    ├── repository/
    │   ├── mod.rs
    │   └── evaluation_repository.rs    # Persist evaluation results
    └── backends/
        ├── mod.rs
        ├── mcp_backend.rs              # MCP-based backend (RuntimeAI compatible)
        ├── http_backend.rs             # HTTP-based backend
        └── local_backend.rs            # Local script/file-based backend
```

**Note:** No `interfaces/` directory initially — the module exposes its API through the application service trait. HTTP/MCP interfaces for direct invocation live in the MCP crate. This follows the same pattern as `quality_gates` which has no interfaces layer.

---

## Guardian Build Checklist

- [ ] Module follows Clean Architecture: domain → application → infrastructure
- [ ] All domain types derive `Debug, Clone, Serialize, Deserialize`
- [ ] `ScoredEvaluationError` uses `thiserror`
- [ ] `ScoringBackend` trait is in domain, implementations in infrastructure
- [ ] Every `mod.rs` has canonical reference header
- [ ] Module spec written to `engine/.pi/architecture/modules/scored-evaluation.md`
- [ ] Contract freeze annotations on all public types
- [ ] Serde round-trip tests for `ScoringResult`, `Rubric`, `ScoreDimension`
- [ ] Contract tests for `ScoringBackend` trait implementations
- [ ] Proofing scripts: `check_scored_evaluation_contracts.sh` + `check_scored_evaluation_coverage.sh`
- [ ] Integration tests for policy condition `ScoreAbove`/`ScoreBelow`
- [ ] `cargo test --workspace` passes
- [ ] `cargo clippy --workspace` zero warnings

---

Last updated: 2026-07-15
*Module version: 1.0.0 (Planned)*

---

**Status:** Planned  
**Implementation priority:** P1 — quality evaluation primitive
