# rigorix-engine

**The core library. It contains every business rule, and it contains no CLI, no
GitHub Action and no server-specific code** — `rigorix-cli`, `rigorix-actions`,
`rigorix-mcp` and `rigorix-server` are all thin adapters over this crate.

If you are evaluating Rigorix, the two things worth knowing are that **the
enforcement path is deterministic Rust, not a model**, and that **approval,
sequence and consequence decisions all evaluate at one choke point** immediately
before a tool is dispatched.

```toml
[dependencies]
rigorix-engine = "1.10"
```

---

## Architecture

**Domain-Driven Design with bounded contexts as independently evolvable modules**
(Modular Monolith, [ADR-001](.pi/architecture/decisions/ADR-001-architecture-pattern.md)).
There are **35 modules** — 32 full Clean Architecture bounded contexts plus 3
cross-cutting modules (`common`, `observability`, `backend`). Each bounded context
has its own `domain/`, `application/` and `infrastructure/` layers and its own
frozen contracts, so a module can be understood, tested and changed without holding
the rest in your head.

```
rigorix-engine/src
├── planning/ ──────────── intent → plan → DAG
├── execution_engine/ ──── the dispatch loop and its single choke point
├── <governance>/ ──────── risk, permission, policy, approval, sequence,
│                         precondition, identity, budget, quality, scoring
└── audit/ ─────────────── signed envelopes, chain, anchor, projection
```

---

## Bounded contexts

### Foundation

| Module | Purpose | Key types |
|---|---|---|
| `configuration` | Multi-source config loading (TOML + env + defaults), secret wrapper | `Config`, `ConfigSource`, `AuditConfig`, `EnforcementPreset` |
| `cancellation` | Graceful-then-immediate shutdown via tokio watch channels | `ShutdownSignal`, `CancellationEvent` |
| `common` | Shared validation helpers | `ValidationResult`, `ValidationError`, `ValidationWarning` |
| `failure_classification` | Failure categorization and retry-strategy selection | `FailureType`, `RetryStrategy`, `BackoffStrategy` |
| `failure_parser` | Parse compiler/test output per language, extract diagnostics | `ParsedFailure`, `FailureSeverity`, `LanguageParser` |

### Planning and compilation

| Module | Purpose | Key types |
|---|---|---|
| `planning` | Intent → classified template → extracted parameters → plan | `Classifier`, `ClassifiedTemplate`, `ExtractedParameters`, `ClarificationPair` |
| `templates` | TOML template parsing, rendering and builtin descriptors | `ParameterDef`, `ParamConstraint`, `RetryConfig`, `BuiltinTemplateDescriptor` |
| `template_generation` | Generate a reusable template from natural language | `ClaudeTemplateGenerator`, `GeneratedTemplate`, `GeneratedTemplateCost` |
| `dag_engine` | Graph construction (`add_unchecked` → seal), topo sort, cycle detection, plan diffing | `TaskGraph`, `TaskNode`, `GatingMode`, `ImpactLevel` |
| `plan_validation` | The self-correcting validate loop: plan → execute → verify → repeat | `ValidationOutcome`, `ValidationIterationReport`, `ValidationLoopConfig` |
| `llm_step` | LLM generation nodes inside the DAG | `LlmGenerateNode`, `LlmGenerateNodeState`, `LlmModelConfig` |
| `orchestrator` | Top-level wiring: planning → execution → persistence → audit | `ExecutionRecord`, `ExecutionStatus`, `ExecutionContext` |

### Execution and tools

| Module | Purpose | Key types |
|---|---|---|
| `tools` | The tool contract and concrete tools (file, git, command, LSP) | `Tool` trait, `ToolResult`, `SideEffect` |
| `execution_engine` | Parallel DAG execution with retry and recovery | `ExecutionResult`, `ExecutionError`, `BackoffStrategy`, `NodeExecutionState` |
| `enforcement` | Execution limits: concurrency, tool calls, budget | `EnforcementConfig`, `ExecutionLimits`, `EnforcementPresetProfile` |
| `recovery_recipes` | Scenario-based recovery with escalation policy | `RecoveryRecipe`, `FailureScenario`, `EscalationPolicy` |
| `code_graph` | Symbol graph built with tree-sitter (Rust/TS/Python) | `CodeGraph`, `GraphMetadata`, `EdgeKind`, `ModuleEdge` |
| `repo_engine` | Workspace-level symbol graph and validation | `SymbolDefinition`, `SharedSymbolGraph`, `SourceLanguage`, `Location` |

### Governance — the enforcement surface

These are the modules that make the product's claims. Each is deterministic, and
each has a decision record.

| Module | Purpose | Key types |
|---|---|---|
| `risk_gating` | Per-step risk classification and gating state (`ADR-007`) | `ClassificationResult`, `GatingAction`, `PendingGate` |
| `permission` | Path and command permission enforcement | `PermissionConfig`, `BashClassifier`, `CommandIntent` |
| `policy_engine` | Lane-based policy evaluation over plan and step data | `LaneContext`, `DiffScope`, `LaneBlocker`, `PolicyCondition` |
| `budget_tracking` | **RAII** call/token budget reservations (`ADR-008`) | `LlmBudget`, `LlmBudgetReservationState` |
| `approval` | Human sign-off bound to a canonical `ExecutionIntent`, single-use, re-verified at dispatch (`ADR-011`) | `ExecutionIntent`, `ApprovalRecord`, `ApprovalStatus`, `DecisionContext` |
| `sequence_policy` | **Composed-action gating** over ordered step sequences (`ADR-013`) | `ParamPredicate`, `ParamMatchKind`, `RequirementAction`, `CompanionFinding` |
| `precondition` | **Consequence gating**: dispatch-time checks, argv or external determination service (`ADR-017`) | `Precondition`, `PreconditionConfig`, `AttributionReason`, `FailureAction` |
| `identity` | Attributed, time-bound identity claims; OSS attests, the enterprise authorises (`ADR-012`) | `IdentityClaim`, `IdentityRef`, `IdentitySource`, `Authority` |
| `quality_gates` | Post-execution quality evaluation against a green contract | `GreenContract`, `QualityLevel`, `QualityGateOutcome` |
| `hooks` | Pre/post execution hooks with a filesystem repository | `HookConfig`, `HookDecision`, `HookAbortSignal` |
| `scored_evaluation` | Pluggable multidimensional artifact scoring (`ADR-010`) | `ScoringBackend`, `Rubric`, `ScoreDimension`, `EvaluateOutput` |
| `code_gen` | Code-generation result domain and syntax gating | `SyntaxGateResult`, `SyntaxError`, `CodeGenConfig` |

### Evidence and observability

| Module | Purpose | Key types |
|---|---|---|
| `audit` | HMAC-signed envelopes, the hash-linked chain, the external anchor, the authority-fact projection (`ADR-016`) | `AuditEnvelope`, `AnchorRef`, `AnchorMode`, `AnchorRuntime` |
| `state_persistence` | Crash-safe execution state (write-rename, `ADR-006`) | `ExecutionState`, `ExecutionGraph`, `ExecutionRecord` |
| `event_system` | The execution event bus: broadcast + persisted drain (`ADR-005`) | `EventBusConfig`, `EventBusFactory` |
| `observability` | Metrics, health checks, tracing configuration | `HealthService`, `HealthReport`, `AggregateHealth` |
| `backend` | Enforcement-config provider seam (HTTP or null) | `EnforcementConfigProvider`, `NullConfigProvider` |

---

## The execution model

```
 intent ─► classify ─► template ─► parameters ─► DAG
                                                  │
                          ┌───────────────────────┘
                          ▼
                   PLAN-TIME GATES
                   operator requirements, sequence rules
                   (before the graph is sealed)
                          │
                          ▼
              ┌───────────────────────────────┐
              │  THE DISPATCH CHOKE POINT     │
              │  1. re-derive intent, compare │
              │     to the approval hash      │
              │  2. evaluate preconditions    │
              │     against live authority    │
              │  3. dispatch — or refuse      │
              └───────────────────────────────┘
                          │
                          ▼
             signed envelope (HMAC + chain + anchor)
```

The choke point is the design. Because approval re-verification, sequence prefix
gating and precondition evaluation all evaluate at **one** insertion point, there
is no alternate dispatch path to forget about — and a proofing contract test
asserts none exists (`ADR-011` §2).

**No LLM in the enforcement path.** The model generates content; the plan and the
operator's policy decide what is allowed to happen. This is a deliberate, repeated
decision (`ADR-007`, `ADR-011`, `ADR-013`, `ADR-017`).

### Core data flow

```
OrchestratorService
  run(RunInput) {
    1. generate execution_id (UUIDv7)
    2. publish PlanningStarted
    3. planning_pipeline.plan_with_graph(intent)
    4. publish PlanningCompleted
    5. state_manager.save_state(Pending)
    6. execution_service.execute_graph(task_graph)
    7. state_manager.save_state(Completed | Failed)
    8. event_bus.drain_persisted() → ExecutionRecord
    9. audit_service.build_and_send()   (best-effort)
  }
```

---

## Scored evaluation

`scored_evaluation` adds quality scoring of generated artifacts to the DAG. It is a
Clean Architecture bounded context with pluggable transport backends.

```
Domain          ScoringBackend (trait) · ScoringResult · ScoreDimension · Rubric
Application     ScoredEvaluationService · EvaluateInput / EvaluateOutput
Infrastructure  EvaluationRepository — McpBackend | HttpBackend | LocalBackend
```

### Backends

**Rigorix defines the scoring protocol** — external systems adopt it by
implementing the server side.

| Backend | Transport | Protocol |
|---|---|---|
| `McpBackend` | MCP (JSON-RPC) | `rigorix_evaluate_artifact` + `rigorix_ping` |
| `HttpBackend` | REST `POST` | Scoring JSON to a configurable endpoint |
| `LocalBackend` | Subprocess (stdin/stdout) | Local script with env vars |

```rust
#[async_trait]
pub trait ScoringBackend: Send + Sync {
    async fn evaluate(&self, artifact: &Value, rubric: &Rubric)
        -> Result<ScoringResult, ScoredEvaluationError>;
    fn backend_name(&self) -> &'static str;
    async fn health_check(&self) -> Result<bool, ScoredEvaluationError>;
}
```

### Result shape

```rust
pub struct ScoringResult {
    pub passed: bool,                                 // all dimensions passed
    pub dimensions: HashMap<String, ScoreDimension>,  // per-dimension scores
    pub summary: String,
    pub backend: String,
    pub duration_ms: u64,
    pub raw: Option<Value>,
}

pub struct ScoreDimension { pub score: f64, pub max: f64, pub label: String, pub passed: bool }
```

### Retry and failure policy

```rust
pub struct ExecutionPolicy {
    pub max_retries: u32,          // default 3
    pub on_failure: FailureAction, // Retry | FlagForReview | Block
}
```

Transient errors (backend error, unavailable, timeout) retry 3× with 200 ms base
backoff. Non-transient errors (invalid rubric, misconfiguration) fail immediately.

### In a template

```toml
[[template.nodes]]
id = "score_output"
name = "Score Code Quality"
action = { type = "scored_evaluation", backend = "runtimeai",
           rubric_source = "inline",
           rubric = { correctness = { threshold = 0.8 },
                      completeness = { threshold = 0.8 } } }
depends_on = ["generate_code"]
```

### Policy-engine integration (merge gating)

```toml
[[rules]]
name = "scored-evaluation-gate"
condition = { type = "score_below", dimension = null, threshold = 80 }
action = "block_merge"
```

Conditions: `ScoreAbove { dimension, threshold }` (all dimensions) and
`ScoreBelow { dimension, threshold }` (any dimension). Scores land in
`LaneContext.scoring_scores: HashMap<String, u8>` — the f64 0.0–1.0 value
converted via `(score * 100.0) as u8`.

### Audit integration

Scoring results flow into the signed envelope for enterprise visibility:

```
ScoredEvaluationService → Orchestrator → BuildEnvelopeInput
  → AuditEnvelope.scoring_results: HashMap<String, ScoringResultRef>
  → POST /api/v1/audit/oss-envelope
  → mv_team_scoring materialized view
  → Enterprise Dashboard (Reports > Scoring)
```

### Errors

| Variant | Retriable | Category |
|---|---|---|
| `BackendNotFound(name)` | No | misconfiguration |
| `BackendError(msg)` | Yes | backend |
| `InvalidRubric(msg)` | No | validation |
| `InvalidArtifact(msg)` | No | validation |
| `BackendUnavailable(msg)` | Yes | infrastructure |
| `Timeout(ms)` | Yes | timeout |
| `Internal(msg)` | No | internal |

---

## Error handling

`CoreOrchestratorError` (`engine/src/error.rs`) aggregates the module error types —
19 via `#[from]`, plus `Io`, `Json` and `Cancelled` (22 variants total):

```rust
pub enum CoreOrchestratorError {
    Dag(#[from] DagError),
    Planning(#[from] PlanningError),
    Enforcement(#[from] EnforcementError),
    // … 16 more module errors, plus Io / Json / Cancelled
}
```

Every error exposes three things:

- **`error_code()`** — a machine-readable string (e.g. `"DAG_ERROR"`)
- **`is_retriable()`** — whether retrying may succeed
- **`http_status()`** — the recommended HTTP status for a host that maps it

Conventions are fixed by [ADR-009](.pi/architecture/decisions/ADR-009-error-handling.md).

---

## What it does not do

Honest boundaries, because a security reader should not have to discover them:

- **It is not a workflow engine.** The precondition primitive is *match a step, run
  a check, refuse on failure, record the outcome* — no loops, no conditionals, no
  policy DSL.
- **It has no domain semantics.** It does not know what "beneficiary eligible"
  means; identity resolution and authority live in your domain (`ADR-013/014/015`).
- **A precondition narrows a window, it does not close it.** There is still a gap
  between the check at T_check and the action at T_action; the downstream system
  must enforce at its own commit point (`ADR-017` §Honest boundary).
- **The local audit cache is tamper-evident, not tamper-proof.** It is not trusted
  for decisions; the external ledger tier exists for that (`ADR-016`).
- **Approval binding strength depends on the tool.** Deterministic tools are bound
  at the payload; `llm_generate` is bound at the input, and its output is verified
  *post hoc* (`ADR-011` §3).

---

## Testing

```bash
cargo test -p rigorix-engine                     # unit + integration
cargo test -p rigorix-engine --lib               # unit only
cargo test -p rigorix-engine --features live-tests   # requires an API key
cargo bench -p rigorix-engine
cargo clippy -p rigorix-engine -- -D warnings
```

Per-crate proofing validators:

```bash
cd engine
bash .pi/scripts/validate-architecture.sh   # module layering
bash .pi/scripts/validate-canonical.sh      # @canonical references
bash .pi/scripts/validate-ubiquitous-language.sh
```

---

## Key design decisions

| ADR | Decision |
|---|---|
| [ADR-001](.pi/architecture/decisions/ADR-001-architecture-pattern.md) | DDD with bounded contexts (modular monolith) |
| [ADR-002](.pi/architecture/decisions/ADR-002-toml-template-format.md) | TOML templates for DAG definitions |
| [ADR-003](.pi/architecture/decisions/ADR-003-llm-provider-traits.md) | Async trait-based LLM provider abstraction |
| [ADR-004](.pi/architecture/decisions/ADR-004-autonomy-presets.md) | Autonomy presets |
| [ADR-005](.pi/architecture/decisions/ADR-005-event-bus-persistence.md) | Event bus: broadcast + drain persistence |
| [ADR-006](.pi/architecture/decisions/ADR-006-atomic-write-rename.md) | Atomic write-rename for state persistence |
| [ADR-007](.pi/architecture/decisions/ADR-007-risk-gating-model.md) | Risk gating with Low/Medium/High classification |
| [ADR-008](.pi/architecture/decisions/ADR-008-raii-budget-reservation.md) | RAII budget reservation |
| [ADR-009](.pi/architecture/decisions/ADR-009-error-handling.md) | Error-handling conventions |
| [ADR-010](.pi/architecture/decisions/ADR-010-scored-evaluation.md) | Scored evaluation: protocol ownership, backends, audit, policy |
| [ADR-011](.pi/architecture/decisions/ADR-011-approval-binding.md) | Approval binding — consequence-bound human sign-off |
| [ADR-012](.pi/architecture/decisions/ADR-012-identity-attestation.md) | Identity attestation — OSS attests, enterprise authorizes |
| [ADR-013](.pi/architecture/decisions/ADR-013-sequence-policy.md) | Sequence policy — composed-action gating |
| [ADR-014](.pi/architecture/decisions/ADR-014-effect-identity-matching.md) | Effect identity matching |
| [ADR-015](.pi/architecture/decisions/ADR-015-operator-step-requirements.md) | Operator step requirements |
| [ADR-016](.pi/architecture/decisions/ADR-016-audit-integrity-anchor.md) | Audit integrity anchor — ledger / projection / cache |
| [ADR-017](.pi/architecture/decisions/ADR-017-consequence-gating.md) | Consequence gating — dispatch-time preconditions |

Architecture lives with the crate:
[`.pi/architecture/modules/`](.pi/architecture/modules/) (one document per bounded
context, each with its `@canonical` reference) and
[`.pi/architecture/decisions/`](.pi/architecture/decisions/).

---

## License

Licensed under either of MIT ([LICENSE-MIT](../LICENSE-MIT)) or Apache-2.0
([LICENSE-APACHE](../LICENSE-APACHE)), at your option.
