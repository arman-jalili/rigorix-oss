# Enforcement Architecture

<!--
Canonical Reference: .pi/architecture/modules/enforcement.md
Blueprint Source: Domain Exploration Session 63c25384
-->

## Overview

Enforces hard caps on execution behavior: retries per node, total retries, tool calls, dynamic nodes, execution time, and parallel tasks. Supports three autonomy presets (Default, Advanced, Aggressive) with configurable limits and absolute safety hard-caps.

## Responsibilities

- Define EnforcementConfig with all hard cap parameters
- Track tool calls, dynamic nodes, retries (per-node and total), and execution time atomically
- Validate all actions against configured limits before allowing them
- Validate config against absolute safety hard-caps at startup
- Provide three autonomy presets: Default (0 dynamic), Advanced (50), Aggressive (200)
- Expose runtime counters for observability and BudgetWarning events

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| EnforcementConfig | `rigorix/src/enforcement.rs` | Hard cap configuration with 3 presets | #config |
| ExecutionEnforcer | `rigorix/src/enforcement.rs` | Atomic runtime tracker and gate | #enforcer |
| EnforcementPreset | `rigorix/src/config.rs` | Enum: Default, Advanced, Aggressive | #preset |

---

## Component Details

### EnforcementConfig

**Purpose:** Hard cap configuration with validated safety limits

**Implementation File:** `rigorix/src/enforcement.rs`

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EnforcementConfig {
    pub max_retries_per_node: u8,   // default: 3
    pub max_total_retries: u32,     // default: 10
    pub max_time_seconds: u64,      // default: 300 (5 min)
    pub max_tool_calls: u32,        // default: 100
    pub max_dynamic_nodes: u32,     // default: 0 (Default mode)
    pub max_parallel_tasks: u32,    // default: 4
    pub max_llm_calls: u32,         // default: 5
    pub max_llm_tokens: u32,        // default: 10,000
}

impl EnforcementConfig {
    pub fn default_mode() -> Self;     // 0 dynamic nodes, strict limits
    pub fn advanced_mode() -> Self;    // 50 dynamic nodes, relaxed limits
    pub fn aggressive_mode() -> Self;  // 200 dynamic nodes, max limits
    pub fn validate(&self) -> Result<(), EnforcementError>;
}
```

**Safety Hard-Caps** (validated by `validate()`):
- `max_dynamic_nodes` ≤ 1,000
- `max_time_seconds` ≤ 7,200 (2 hours)
- `max_parallel_tasks` ≤ 64

### ExecutionEnforcer

**Purpose:** Atomic runtime enforcer that gates every action against limits

**Implementation File:** `rigorix/src/enforcement.rs`

```rust
pub struct ExecutionEnforcer {
    config: EnforcementConfig,
    start_time: Instant,
    tool_calls: AtomicU32,
    dynamic_nodes: AtomicU32,
    total_retries: AtomicU32,
    node_retries: RwLock<HashMap<Uuid, AtomicU8>>,
}

impl ExecutionEnforcer {
    pub fn new(config: EnforcementConfig) -> Self;
    pub fn check_time_limit(&self) -> Result<(), EnforcementError>;
    pub fn record_tool_call(&self) -> Result<(), EnforcementError>;
    pub fn record_dynamic_node(&self) -> Result<(), EnforcementError>;
    pub fn can_retry(&self, node_id: Uuid) -> Result<(), EnforcementError>;
    pub fn record_retry(&self, node_id: Uuid) -> Result<(), EnforcementError>;
    pub fn total_retries_used(&self) -> u32;
    pub fn tool_calls_used(&self) -> u32;
}
```

---

## Three Autonomy Presets

| Limit | Default | Advanced | Aggressive | Absolute Cap |
|-------|---------|----------|------------|-------------|
| max_dynamic_nodes | 0 | 50 | 200 | 1,000 |
| max_time_seconds | 300 (5m) | 1,800 (30m) | 3,600 (1h) | 7,200 (2h) |
| max_tool_calls | 100 | 500 | 2,000 | — |
| max_llm_calls | 5 | 20 | 50 | — |
| max_llm_tokens | 10,000 | 100,000 | 500,000 | — |
| max_parallel_tasks | 4 | 8 | 16 | 64 |
| max_total_retries | 10 | 30 | 100 | — |
| max_retries_per_node | 3 | 3 | 5 | — |

---

## Dependencies

### Depends On
- **Configuration**: EnforcementPreset selected via Config or CLI flag

### Used By
- **Execution Engine**: ParallelExecutor checks enforcer before each retry/tool call
- **Budget Tracking**: LlmBudget shares max_llm_calls/max_llm_tokens from config

---

## Data Flow

```mermaid
flowchart TB
    CALL["Execution Engine
requests action"] --> CHECK{"Which limit?"]
    
    CHECK -->|retry| CR["ExecutionEnforcer
.can_retry(node_id)"]
    CHECK -->|tool call| TC["ExecutionEnforcer
.record_tool_call()"]
    CHECK -->|dynamic node| DN["ExecutionEnforcer
.record_dynamic_node()"]
    CHECK -->|time check| TL["ExecutionEnforcer
.check_time_limit()"]
    
    CR -->|OK| RECR[".record_retry(node_id)
per-node + total counters"]
    CR -->|exceeded| ER1["EnforcementError
MaxRetriesExceeded"]
    
    TC -->|OK + ≤max| TC_OK["tool_calls_used +1"]
    TC -->|>max| ER2["EnforcementError
ToolCallLimitExceeded"]
    
    DN -->|OK + ≤max| DN_OK["dynamic_nodes_used +1"]
    DN -->|>max| ER3["EnforcementError
DynamicNodeLimitExceeded"]
    
    TL -->|elapsed < max| TL_OK["OK"]
    TL -->|elapsed > max| ER4["EnforcementError
TimeLimitExceeded"]
    
    subgraph Config Validation
        V1["EnforcementConfig::validate()
max_dynamic_nodes ≤ 1000
max_time_seconds ≤ 7200
max_parallel_tasks ≤ 64"]
    end
```

**Flow Description:**
1. ExecutionEnforcer provides thread-safe atomic counters for all limits
2. Each action checks limits before proceeding (fail-fast)
3. Per-node retry tracking via RwLock<HashMap<Uuid, AtomicU8>>
4. EnforcementConfig::validate() runs at startup against absolute safety caps

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 95% | `rigorix/src/enforcement.rs` (inline tests) |

**Key Test Scenarios:**
- Default mode: max_dynamic_nodes=0, max_parallel_tasks=4
- Advanced mode: max_dynamic_nodes=50, max_parallel_tasks=8
- Aggressive mode: max_dynamic_nodes=200, max_parallel_tasks=16
- can_retry within limits → Ok
- can_retry after max → Err
- validate rejects > 1000 dynamic nodes
- ExecutionEnforcer tracks total_retries correctly

---

## Error Handling

```rust
#[derive(Debug, Error)]
pub enum EnforcementError {
    #[error("Max retries exceeded: {0}")]
    MaxRetriesExceeded(String),
    #[error("Total retries exceeded: {0}")]
    TotalRetriesExceeded(String),
    #[error("Time limit exceeded: {0}")]
    TimeLimitExceeded(String),
    #[error("Tool call limit exceeded: {0}")]
    ToolCallLimitExceeded(String),
    #[error("Dynamic node limit exceeded: {0}")]
    DynamicNodeLimitExceeded(String),
    #[error("Invalid enforcement configuration: {0}")]
    InvalidConfig(String),
    #[error("Lock poisoned")]
    LockPoisoned,
}
```

---

*Last updated: 2026-06-13*
*Module version: 1.0.0*
