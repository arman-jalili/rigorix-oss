# ADR-010: Scored Evaluation Architecture — Pluggable Quality Scoring Backends

**Status:** Accepted
**Date:** 2026-07-15

**Tech Stack:** Rust

## Context

The Rigorix DAG produces artifacts (code patches, generated files, LLM outputs) that must be evaluated for quality before merge. The existing Quality Gates system (ADR-001, GreenContract) evaluates *test scope* — whether tests ran broadly enough — but does not evaluate *output quality* — whether generated artifacts meet correctness, completeness, or style standards.

Key requirements:
- Multidimensional scoring (correctness, completeness, style, safety, etc.)
- Pluggable backends — no vendor lock-in
- Policy integration for merge gating based on score thresholds
- Audit trail for compliance provenance
- Rigorix-defined scoring protocol with external backend adoption

## Decision

### 0. Rigorix Defines the Scoring Protocol

Rigorix owns the scoring protocol — the `ScoringBackend` domain trait defines the contract, and the MCP/HTTP operations (`rigorix_evaluate_artifact`, `rigorix_ping`, etc.) are Rigorix-defined. External systems like RuntimeAI adopt this protocol by implementing the server side.

The initial protocol design is informed by RuntimeAI's conceptual model (checkrides, scenarios, rubrics) since they are the first planned backend adopter, but **the protocol belongs to Rigorix**.

### 1. `ScoredEvaluationNode` as a DAG Node Type

A new `scored_evaluation` node type is added to the DAG as a recognized tool string. It is validated at `TaskNode` construction time by checking `tool == "scored_evaluation"`. This avoids an enum refactor of `NodeTool` while keeping the door open for a typed enum later.

The node carries:
- The artifact to evaluate (as `serde_json::Value`)
- A rubric (inline JSON or reference to external file/URL)
- A backend selector string
- Per-dimension thresholds
- An `ExecutionPolicy` for retry/fallback behavior

### 2. `ScoringBackend` Trait in the Domain Layer

The `ScoringBackend` trait lives in the domain layer as a pure interface:

```rust
#[async_trait]
pub trait ScoringBackend: Send + Sync {
    async fn evaluate(&self, artifact: &Value, rubric: &Rubric) -> Result<ScoringResult, ScoredEvaluationError>;
    fn backend_name(&self) -> &'static str;
    async fn health_check(&self) -> Result<bool, ScoredEvaluationError>;
}
```

Three implementations live in the infrastructure layer:

| Backend | Transport | Protocol | Primary Use |
|---------|-----------|----------|-------------|
| **MCPBackend** | MCP `rigorix_evaluate_artifact` request | Rigorix Scoring Protocol over MCP | External systems adopting Rigorix protocol (e.g., RuntimeAI) |
| **HTTPBackend** | HTTP POST to scoring API endpoint | Rigorix Scoring Protocol (JSON) over REST | Custom evaluation services |
| **LocalBackend** | Subprocess execution | Rigorix Scoring Protocol (stdin/stdout JSON) | Development/testing |

### 3. `f64` → `u8` Score Conversion for Policy Compatibility

`PolicyCondition` derives `PartialEq, Eq`. Since `f64` does not implement `Eq`, score thresholds use `u8` percentage (0–100), matching the existing `GreenAt { level: u8 }` convention.

- Backend scores are `f64` in `ScoreDimension.score` (0.0–1.0)
- At evaluation time: `(score * 100.0) as u8`
- Policy conditions: `ScoreAbove { dimension: Option<String>, threshold: u8 }`, `ScoreBelow { ... }`

### 4. Domain Events for Audit Trail

Three domain events track the scoring lifecycle:
- `ScoredEvaluationStarted` — emitted when evaluation begins
- `ScoredEvaluationCompleted` — emitted on success, carries `ScoringResult`
- `ScoredEvaluationFailed` — emitted on failure, carries error string

These are published via the EventBus and consumed by the audit system.

### 5. Audit Envelope Extension

`AuditEnvelope` gains a `scoring_results: HashMap<String, ScoringResultRef>` field keyed by `node_id`, enabling compliance provenance.

### 6. Policy Integration

New `PolicyCondition` variants:
- `ScoreAbove { dimension: Option<String>, threshold: u8 }` — all dimensions (or a specific one) must be above threshold
- `ScoreBelow { dimension: Option<String>, threshold: u8 }` — any dimension (or a specific one) below threshold triggers action

## Alternatives Considered

| Alternative | Pros | Cons | Reason Rejected |
|-------------|------|------|-----------------|
| **Pluggable backends with domain-layer trait (chosen)** | Clean separation, testable, no vendor lock-in | More code for trait + implementations | **Chosen** |
| Rigorix implements RuntimeAI's proprietary protocol | Easier initial integration with RuntimeAI | Vendor lock-in; protocol not owned by Rigorix | Rejected — Rigorix must own the protocol |
| Single MCP-only backend | Simpler, fewer files to maintain | No HTTP or local fallback option | Rejected — violates pluggability principle |
| `f64` in PolicyCondition directly | No conversion needed | `f64` doesn't implement `Eq`; would need custom PartialEq | Rejected — breaks existing PolicyCondition derives |
| Binary pass/fail scoring | Simplest possible model | No multidimensional nuance; can't express "correctness 80%, completeness 60%" | Rejected — insufficient expressiveness |
| Scoring as part of Quality Gates module | Consolidates quality concerns | Different concerns: scope vs output quality; would violate single responsibility | Rejected — orthogonal dimensions |

## Consequences

### Positive
- **Protocol ownership**: Rigorix defines the scoring contract; external systems adopt it
- Clean domain boundary: `ScoringBackend` trait in domain, implementations in infrastructure
- No vendor lock-in: MCP, HTTP, and local backends are all first-class
- Policy-native: score thresholds integrate directly into existing `PolicyCondition` enum
- Audit-compatible: scoring results extend `AuditEnvelope` for compliance
- Event-driven: scoring lifecycle events stream through existing EventBus
- No breaking changes: `NodeTool` remains a string (no enum refactor), `PolicyCondition` gets new variants (backward-compatible addition)
- RuntimeAI conceptual alignment: protocol design informed by RuntimeAI's proven model (checkrides, scenarios), reducing adoption friction

### Negative
- f64→u8 conversion introduces minor precision loss (1% granularity)
- Three backend implementations must all pass the same trait contract test suite
- No `interfaces/` layer initially — direct MCP/HTTP exposure lives in the MCP crate

### Neutral
- Quality Gates (GreenContract) and Scored Evaluation remain independent modules that both feed the Policy Engine
- Module follows Clean Architecture with 3 layers (no interfaces initially)

## Implementation

**Affected Modules:**
- `.pi/architecture/modules/scored-evaluation.md` — new module spec
- `.pi/architecture/modules/dag-engine.md` — add `scored_evaluation` as recognized tool string
- `.pi/architecture/modules/policy-engine.md` — add `ScoreAbove`/`ScoreBelow` condition variants
- `.pi/architecture/modules/audit.md` — add `ScoringResultRef` to envelope
- `.pi/architecture/modules/event-system.md` — add `ScoredEvaluation*` execution event variants

**Files to Update/Create:**
- `engine/src/scored_evaluation/` — entire new module (domain/application/infrastructure)
- `engine/src/policy_engine/domain/condition.rs` — add `ScoreAbove`, `ScoreBelow`
- `engine/src/audit/domain/envelope.rs` — add `ScoringResultRef`
- `engine/src/event_system/domain/event.rs` — add `ScoredEvaluation*` variants

**Canonical References:**
Implementation files should reference: `.pi/architecture/decisions/ADR-010-scored-evaluation.md`

## Validation

**Validators Required:**
- architecture-validator: Verify Clean Architecture layer compliance (trait in domain, impl in infrastructure)
- security-validator: Verify HMAC signing, secret handling, script injection guards
- integration-validator: Verify MCP/HTTP/Local backends pass same contract tests
- operations-validator: Verify timeout config, circuit breaker patterns, retry behavior
- test-validator: Verify 90% unit coverage, 80% integration coverage, all acceptance criteria

## References

- Related ADRs: ADR-001 (DDD with Bounded Contexts), ADR-007 (Risk Gating), ADR-009 (Error Handling)
- Related modules: `.pi/architecture/modules/quality-gates.md` (orthogonal quality dimension)

---

*Decision date: 2026-07-15*
*Decision makers: Architecture Team*
