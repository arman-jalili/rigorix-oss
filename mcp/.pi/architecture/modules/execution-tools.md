# Execution Tools

## Module Status

**Status:** Planned
**Last reviewed:** 2026-07-12
**Source session:** d19b7a21-8f4c-4b3e-9a1d-5e6f7c8b9a0d

## Description

Bridges MCP tool calls to rigorix-engine: plan execution (`rigorix_execute`), pre-flight validation (`rigorix_validate_plan`), and enforcement status checks (`rigorix_check_enforcement`). This is the **primary value-add** of the MCP Gateway — AI assistants get deterministic execution through Rigorix.

## Architecture

This module follows **Domain-Driven Design** with Clean Architecture layers:

| Layer | Responsibility | Path |
|-------|---------------|------|
| **Domain** | EngineFacade trait, PlanTemplate, StepDefinition, execution result types | `src/execution-tools/domain/` |
| **Application** | Use cases for execute, validate, check enforcement | `src/execution-tools/application/` |
| **Infrastructure** | EngineFacade implementation (rigorix-engine client), timeout enforcement | `src/execution-tools/infrastructure/` |
| **Interfaces** | MCP tool handlers exposing rigorix_execute, rigorix_validate_plan, rigorix_check_enforcement | `src/execution-tools/interfaces/` |

## Related ADRs

| ADR | Relevance |
|-----|-----------|
| [ADR-001](./decisions/ADR-001-architecture-pattern.md) | Execution Tools is one of the 5 bounded contexts in the modular monolith |
| [ADR-003](./decisions/ADR-003-cross-context-communication.md) | Defines EngineFacade trait pattern — execution tools implement the primary facade interface that all handler contexts use |
| [ADR-006](./decisions/ADR-006-cost-tracking-usage-metering.md) | Defines cost tracking delegation — execution tools query engine for enforcement status and costs |
| [ADR-007](./decisions/ADR-007-compliance-engine-architecture.md) | Defines compliance read-only principle — execution tools generate execution IDs but never modify audit state |

## Diagrams

### Data Flow

```mermaid
flowchart LR
    subgraph "MCP Client (AI Tool)"
        CLI["MCP Client\n(rigorix_execute call)"]
    end

    subgraph "Execution Tools"
        HANDLER["ExecuteHandler"]
        VALIDATOR["ValidatePlanHandler"]
        ENFORCER["CheckEnforcementHandler"]
        FACADE["EngineFacade\n(interface)"]
    end

    subgraph "rigorix-engine (local)"
        EXEC_ENG["Execution Engine\n(DAG Executor)"]
        POL_ENG["Policy Engine\n(Enforcement)"]
        AUDIT_ENG["Audit Engine\n(Storage)"]
    end

    subgraph "Output"
        RESULT["ExecutionResult\n{ execution_id, status,\n steps, audit_uri }"]
        VAL_RESULT["ValidationResult\n{ valid, warnings, errors,\n estimated_cost }"]
        ENF_STATUS["EnforcementStatus\n{ active, preset,\n budget_remaining }"]
    end

    CLI -->|"JSON-RPC: tools/call"| HANDLER
    CLI -->|"JSON-RPC: tools/call"| VALIDATOR
    CLI -->|"JSON-RPC: tools/call"| ENFORCER

    HANDLER --> FACADE
    VALIDATOR --> FACADE
    ENFORCER --> FACADE

    FACADE -->|"execute_plan(plan)"| EXEC_ENG
    FACADE -->|"validate_plan(plan)"| POL_ENG
    FACADE -->|"check_enforcement()"| POL_ENG
    EXEC_ENG -->|"plan result"| AUDIT_ENG

    FACADE -->|"returns"| RESULT
    FACADE -->|"returns"| VAL_RESULT
    FACADE -->|"returns"| ENF_STATUS
```

### Entity Relationship

```mermaid
classDiagram
    class EngineFacade {
        <<interface>>
        +execute(plan: PlanTemplate) Result~ExecutionResult~
        +validate_plan(plan: PlanTemplate) Result~ValidationResult~
        +check_enforcement() Result~EnforcementStatus~
        +get_execution_cost(id: ExecutionId) Result~CostBreakdown~
    }

    class EngineFacadeImpl {
        -orchestrator: Arc~Orchestrator~
        -enforcer: Arc~ExecutionEnforcer~
        -config: EngineFacadeConfig
        -timeout_duration: Duration
    }

    class ExecuteHandler {
        +handle(input: ExecuteInput) Result~ExecutionResult~
        +validate_input(input) Result
        +format_result(engine_output) ExecutionResult
    }

    class ValidatePlanHandler {
        +handle(input: ValidateInput) Result~ValidationResult~
        +format_validation(engine_output) ValidationResult
    }

    class CheckEnforcementHandler {
        +handle() Result~EnforcementStatus~
        +format_status(engine_status) EnforcementStatus
    }

    class PlanTemplate {
        <<value object>>
        +name: String
        +description: String
        +steps: Vec~StepDefinition~
        +constraints: Constraints
        +metadata: HashMap~String, String~
    }

    class StepDefinition {
        <<value object>>
        +name: String
        +tool: String
        +parameters: HashMap~String, Value~
        +requires_approval: bool
        +description: String
        +timeout_secs: Option~u64~
    }

    class ExecutionResult {
        <<value object>>
        +execution_id: Uuid
        +status: ExecutionStatus
        +steps: Vec~StepResult~
        +duration_ms: u64
        +tokens_used: Option~u64~
        +audit_uri: String
    }

    class ValidationResult {
        <<value object>>
        +valid: bool
        +warnings: Vec~String~
        +errors: Vec~String~
        +estimated_cost: Option~CostEstimate~
    }

    class EnforcementStatus {
        <<value object>>
        +active: bool
        +preset: String
        +budget: BudgetStatus
        +circuit_breakers: Vec~CircuitBreakerStatus~
    }

    class ExecuteInput {
        <<value object>>
        +plan: PlanTemplate
        +template_name: Option~String~
        +execution_id: Option~Uuid~
    }

    class ValidateInput {
        <<value object>>
        +plan: PlanTemplate
    }

    EngineFacade <|.. EngineFacadeImpl
    ExecuteHandler --> EngineFacade
    ValidatePlanHandler --> EngineFacade
    CheckEnforcementHandler --> EngineFacade

    ExecuteHandler ..> ExecuteInput
    ValidatePlanHandler ..> ValidateInput

    ExecuteHandler ..> ExecutionResult
    ValidatePlanHandler ..> ValidationResult
    CheckEnforcementHandler ..> EnforcementStatus

    PlanTemplate *-- StepDefinition
```

### Aggregate State

```mermaid
stateDiagram-v2
    [*] --> Idle

    Idle --> Validating: rigorix_validate_plan called
    Validating --> Passed: validation succeeds
    Validating --> Rejected: validation fails
    Passed --> Idle: return result

    Idle --> Executing: rigorix_execute called
    Executing --> Completed: all steps succeed
    Executing --> Failed: step fails (non-retryable)
    Executing --> PartialFailed: some steps fail, partial success
    Executing --> Cancelled: timeout or cancellation
    Executing --> EnforcementBlocked: budget exceeded mid-execution

    Completed --> Idle: return ExecutionResult
    Failed --> Idle: return ExecutionResult with error
    PartialFailed --> Idle: return ExecutionResult with partial results
    Cancelled --> Idle: return ExecutionResult with cancelled status
    EnforcementBlocked --> Idle: return ExecutionResult with enforcement error

    Rejected --> Idle: return ValidationResult

    Idle --> CheckingEnforcement: rigorix_check_enforcement called
    CheckingEnforcement --> Idle: return EnforcementStatus
```

### Key Use Case Sequence: Execute Plan

```mermaid
sequenceDiagram
    participant Client as MCP Client
    participant MCP as MCP Server Router
    participant Handler as ExecuteHandler
    participant Facade as EngineFacadeImpl
    participant Engine as rigorix-engine
    participant Enforcer as Execution Enforcer

    Client->>MCP: tools/call { name: "rigorix_execute", arguments: { plan } }
    MCP->>Handler: route to ExecuteHandler

    Handler->>Handler: validate input schema

    alt Invalid Input
        Handler-->>MCP: ToolError::InvalidArguments
        MCP-->>Client: JSON-RPC error
    end

    Handler->>Facade: execute(plan)

    Facade->>Enforcer: check_enforcement()
    Enforcer-->>Facade: enforcement check result

    alt Budget Exceeded
        Facade-->>Handler: EnforcementError::BudgetExceeded
        Handler-->>MCP: ToolResult { is_error: true, content: "Budget exceeded" }
        MCP-->>Client: error response
    end

    Facade->>Engine: execute_plan(plan, execution_id)
    Note over Engine: Deterministic execution\nstep by step
    Engine-->>Facade: engine result

    Facade->>Facade: map engine result to ExecutionResult
    Facade-->>Handler: ExecutionResult { execution_id, status, steps, audit_uri }

    Handler->>Handler: format as MCP content
    Handler-->>MCP: ToolResult { content: [formatted result] }
    MCP-->>Client: success response with execution result
```

## Components

### EngineFacade (Aggregate Root)

Thin async facade over rigorix-engine that all execution/audit/template tools call through.

**Invariants:**
- All method calls are governed by configurable timeout
- Never panics — errors are wrapped in `EngineFacadeError`
- Enforcement status is always fresh (never cached — queries engine on each call)
- Execution IDs are generated by rigorix-engine, not the gateway

**Key Methods:**
```rust
#[async_trait]
pub trait EngineFacade: Send + Sync {
    async fn execute(&self, plan: PlanTemplate) -> Result<ExecutionResult, EngineFacadeError>;
    async fn validate_plan(&self, plan: PlanTemplate) -> Result<ValidationResult, EngineFacadeError>;
    async fn check_enforcement(&self) -> Result<EnforcementStatus, EngineFacadeError>;
    async fn get_execution_cost(&self, execution_id: &ExecutionId) -> Result<CostBreakdown, EngineFacadeError>;
}

pub struct EngineFacadeImpl {
    orchestrator: Arc<Orchestrator>,
    enforcer: Arc<ExecutionEnforcer>,
    audit_query: Arc<dyn AuditQuery>,
    config: EngineFacadeConfig,
}

impl EngineFacadeImpl {
    pub fn new(
        orchestrator: Arc<Orchestrator>,
        enforcer: Arc<ExecutionEnforcer>,
        audit_query: Arc<dyn AuditQuery>,
        config: EngineFacadeConfig,
    ) -> Self;
}
```

### ExecuteHandler (Domain Service)

Handles `rigorix_execute` tool calls: validates input, delegates to engine, formats result.

```rust
pub struct ExecuteHandler {
    engine: Arc<dyn EngineFacade>,
    timeout_duration: Duration,
}

impl ExecuteHandler {
    pub fn new(engine: Arc<dyn EngineFacade>, timeout_duration: Duration) -> Self;

    pub async fn handle(&self, input: ExecuteInput) -> Result<ToolCallResult, HandlerError> {
        // 1. Validate input schema
        let plan = PlanTemplate::from_json(input.plan)?;

        // 2. Execute with timeout
        let result = tokio::time::timeout(
            self.timeout_duration,
            self.engine.execute(plan),
        )
        .await
        .map_err(|_| HandlerError::Timeout)??;

        // 3. Format as MCP content
        Ok(self.format_execution_result(result))
    }
}
```

### ValidatePlanHandler (Domain Service)

Handles `rigorix_validate_plan` tool calls: checks plan against enforcement policies.

```rust
pub struct ValidatePlanHandler {
    engine: Arc<dyn EngineFacade>,
}

impl ValidatePlanHandler {
    pub fn new(engine: Arc<dyn EngineFacade>) -> Self;

    pub async fn handle(&self, input: ValidateInput) -> Result<ToolCallResult, HandlerError> {
        let plan = PlanTemplate::from_json(input.plan)?;
        let result = self.engine.validate_plan(plan).await?;
        Ok(self.format_validation_result(result))
    }
}
```

### CheckEnforcementHandler (Domain Service)

Handles `rigorix_check_enforcement` tool calls: queries engine for current budget/limits.

```rust
pub struct CheckEnforcementHandler {
    engine: Arc<dyn EngineFacade>,
}

impl CheckEnforcementHandler {
    pub fn new(engine: Arc<dyn EngineFacade>) -> Self;

    pub async fn handle(&self) -> Result<ToolCallResult, HandlerError> {
        let status = self.engine.check_enforcement().await?;
        Ok(self.format_enforcement_status(status))
    }
}
```

## Domain Events

| Event | Description | Trigger | Payload | Published By |
|-------|-------------|---------|---------|-------------|
| PlanExecutionStarted | A plan execution was initiated via `rigorix_execute` | ExecuteHandler on receiving valid plan | `{ execution_id, template_name, step_count, started_at }` | ExecuteHandler |
| PlanExecutionCompleted | A plan execution completed (success, failure, or partial) | ExecuteHandler after engine returns result | `{ execution_id, status, duration_ms, token_count }` | ExecuteHandler |
| PlanValidated | A plan was validated via `rigorix_validate_plan` | ValidatePlanHandler | `{ session_id, is_valid, warning_count, error_count, estimated_cost }` | ValidatePlanHandler |
| EnforcementChecked | Enforcement status was queried via `rigorix_check_enforcement` | CheckEnforcementHandler | `{ session_id, preset, budget_remaining, circuit_breaker_count }` | CheckEnforcementHandler |

## API Endpoints (MCP Tool Schemas)

| Method | Path (tool name) | Handler | Input | Output | Auth |
|--------|-----------------|---------|-------|--------|------|
| `rigorix_execute` | `tools/call` | ExecuteHandler | `{ plan: PlanTemplate, template_name?: string, execution_id?: string }` | `{ execution_id, status, steps, duration_ms, tokens_used?, audit_uri }` | Session-bound |
| `rigorix_validate_plan` | `tools/call` | ValidatePlanHandler | `{ plan: PlanTemplate }` | `{ valid, warnings, errors, estimated_cost? }` | Session-bound |
| `rigorix_check_enforcement` | `tools/call` | CheckEnforcementHandler | `{}` | `{ active, preset, budget: { tool_calls_total, tool_calls_remaining, tokens_total, tokens_remaining }, circuit_breakers }` | Session-bound |

## Ubiquitous Language

Terms specific to this context from `.pi/domain/ubiquitous-language.md`:

| Term | Definition |
|------|-----------|
| **EngineFacade** | Aggregate root providing a thin async facade over rigorix-engine public APIs for all MCP tool handlers |
| **ExecuteHandler** | Domain service that handles `rigorix_execute` tool calls: validates input, executes via engine, returns results |
| **ValidatePlanHandler** | Domain service that handles `rigorix_validate_plan` tool calls: checks plan against enforcement policies |
| **CheckEnforcementHandler** | Domain service that handles `rigorix_check_enforcement` tool calls: queries engine for current budget and limits |
| **PlanTemplate** | Value object shared between execution and template contexts: a structured plan with steps, constraints, and metadata |
| **StepDefinition** | Value object representing a single step in a plan: tool name, parameters, approval requirement, description |
| **ExecutionResult** | Value object returned by `rigorix_execute`: execution_id, status, per-step results, duration, tokens, audit_uri |
| **ValidationResult** | Value object returned by `rigorix_validate_plan`: valid flag, warnings, errors, estimated cost |
| **EnforcementStatus** | Value object returned by `rigorix_check_enforcement`: active, preset, remaining budget, circuit breaker states |

## Dependencies

### Depends On
- **MCP Server**: Receives tool calls routed from RequestRouter; registers tool schemas and handlers via ToolRegistry
- **rigorix-engine (external)**: All execution, validation, and enforcement queries delegated via EngineFacade trait

### Used By
- **Audit Tools**: Shares EngineFacade trait for audit queries (reads from rigorix-engine)
- **Template Tools**: Shares EngineFacade trait for template validation against enforcement policies
- **Binary Compose**: All three handler modules are wired together at the composition root

## Implementation Sequence

1. **Phase 1.1 — EngineFacade Contract**: Define `EngineFacade` trait, `PlanTemplate` value object, `StepDefinition`, execution result types
2. **Phase 1.2 — EngineFacade Implementation**: Implement `EngineFacadeImpl` wrapping rigorix-engine APIs
3. **Phase 1.3 — ExecuteHandler**: Implement `rigorix_execute` handler with input validation, timeout enforcement, result formatting
4. **Phase 1.4 — ValidatePlanHandler**: Implement `rigorix_validate_plan` handler delegating to engine validation
5. **Phase 1.5 — CheckEnforcementHandler**: Implement `rigorix_check_enforcement` handler for budget/status queries
6. **Phase 1.6 — MCP Schema Registration**: Register tool schemas in ToolRegistry, wire handlers via RequestRouter
7. **Phase 1.7 — SSE Progress Notifications**: Add streaming progress updates for long-running `rigorix_execute` calls (SSE only)

**depends:** MCP Server (Phase 0)
