# Rust Enterprise Code Generation — Best Practices

> Canonical skill for generating production-grade Rust code in the rigorix-oss project.
> All code MUST follow these patterns. Validators enforce compliance.
>
> Source: Extracted from rigorix-engine (17 frozen-contract modules) + DDD architecture analysis.

---

## 1. Project Structure — Clean Architecture with DDD

Every bounded context follows the same 4-layer structure:

```
module/
├── domain/           # Pure domain entities, value objects, events
│   ├── mod.rs        # Re-exports + module-level documentation
│   ├── entity.rs     # Aggregate roots and entities
│   ├── value.rs      # Value objects
│   ├── event.rs      # Domain event payloads
│   └── error.rs      # Typed error enum (thiserror)
├── application/      # Service traits, DTOs, factory interfaces
│   ├── mod.rs
│   ├── service.rs    # Service trait definitions
│   ├── factory.rs    # Factory trait interfaces
│   └── dto/          # Input/Output DTOs with validation
├── infrastructure/   # Repository interfaces
│   └── repository/   # Repository trait definitions
└── interfaces/       # API contracts (HTTP, events)
    └── http/         # REST endpoint contracts
```

### Module Header Pattern

Every `mod.rs` and every domain file MUST include a canonical reference header:

```rust
//! Module Purpose — One-line summary of what this module does.
//!
//! @canonical .pi/architecture/modules/[module-name].md#[section]
//! Implements: Contract Freeze — [component names]
//! Issue: #[issue-number]
//!
//! Longer description of the module's purpose, design decisions,
//! and how it fits into the larger architecture.
//!
//! # Architecture
//!
//! ```text
//! module/
//! ├── domain/     ...
//! ├── application/ ...
//! ├── infrastructure/ ...
//! └── interfaces/ ...
//! ```
//!
//! # Contract (Frozen)
//! - [List of frozen contract rules]
//! - No implementation logic beyond constructors and field accessors
//! - All domain types are serializable (Serialize + Deserialize)
```

### Dependency Direction Rule

```
domain → application → infrastructure → interfaces
         ↑                    ↑
         └── inward dependency rule: outer layers depend on inner, never reverse
```

- **domain/** — depends on nothing except serde, chrono, uuid (pure data)
- **application/** — depends on domain
- **infrastructure/** — depends on application
- **interfaces/** — depends on application

---

## 2. Error Handling — thiserror with Aggregation

### Per-Module Error Enum

```rust
use thiserror::Error;

/// Typed error enum for the [Module] bounded context.
///
/// # Contract (Frozen)
/// - Every error variant follows the pattern: `PascalCase { fields }`
/// - `#[error("...")]` Display messages are user-readable
/// - Implement `is_retriable()` for the module's transient failures
/// - Derive `Serialize + Deserialize` for API responses
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum ModuleError {
    /// Resource not found — include what was requested and what's available.
    #[error("Not found: {id}. Available: {available:?}")]
    NotFound { id: String, available: Vec<String> },

    /// Invalid state transition attempt.
    #[error("Invalid state transition: {reason}")]
    InvalidState { reason: String },

    /// Duplicate identifier.
    #[error("Duplicate ID: {id}")]
    DuplicateId { id: Uuid },

    /// Dependency resolution failure — list missing dependencies.
    #[error("Missing dependencies: {missing:?}")]
    MissingDependency { missing: Vec<String> },

    /// Operation was cancelled.
    #[error("Operation cancelled")]
    Cancelled,
}

impl ModuleError {
    /// Returns true if the error represents a transient failure that can be retried.
    pub fn is_retriable(&self) -> bool {
        match self {
            // Only transient failures are retriable
            ModuleError::MissingDependency { .. } => true,
            _ => false,
        }
    }
}
```

### Root Error Aggregation (CoreOrchestratorError Pattern)

```rust
use thiserror::Error;

/// Root error type that aggregates all domain-specific errors via #[from].
#[derive(Debug, Error)]
pub enum RootError {
    #[error("DAG error: {0}")]
    Dag(#[from] DagError),

    #[error("Planning error: {0}")]
    Planning(#[from] PlanningError),

    #[error("Execution error: {0}")]
    Execution(#[from] ExecutionError),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Operation cancelled: {0}")]
    Cancelled(String),
}

impl RootError {
    /// HTTP status code that best represents this error.
    pub fn http_status(&self) -> u16 {
        match self {
            RootError::Dag(_) => 500,
            RootError::Planning(_) => 400,
            RootError::Execution(_) => 500,
            RootError::Io(_) => 500,
            RootError::Cancelled(_) => 499,
        }
    }

    /// Machine-readable error code.
    pub fn error_code(&self) -> &'static str {
        match self {
            RootError::Dag(_) => "DAG_ERROR",
            RootError::Planning(_) => "PLANNING_ERROR",
            RootError::Execution(_) => "EXECUTION_ERROR",
            RootError::Io(_) => "IO_ERROR",
            RootError::Cancelled(_) => "CANCELLED",
        }
    }
}
```

### Rules

- ✅ Use `thiserror` for ALL library/domain errors
- ✅ Every error has a descriptive `#[error("...")]` message
- ✅ Include context in error fields (what was requested, what's available)
- ✅ Implement `is_retriable()` for each error that might have transient variants
- ✅ Root error aggregates sub-errors via `#[from]` for `?` operator propagation
- ❌ NEVER use `anyhow` in library code — reserved for binary crates only
- ❌ NEVER use `.unwrap()` or `.expect()` in production code
- ❌ NEVER use `String` errors — always typed enums

---

## 3. Secret Handling — Redacted Value Object

```rust
/// A sensitive value (API key, token) that is redacted in all text output.
///
/// # Security
/// - Debug/Display show `[REDACTED]` (never leak)
/// - Only `.expose()` reveals the inner value
/// - Serde serialization is transparent (writes actual value)
/// - Hash/Eq compare by inner value
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The ONLY way to access the inner value.
    pub fn expose(&self) -> &str {
        &self.0
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            write!(f, "<empty>")
        } else {
            write!(f, "[REDACTED]")
        }
    }
}

impl fmt::Display for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Debug::fmt(self, f)
    }
}
```

### Rules
- ✅ Wrap all API keys, tokens, passwords in `Secret`
- ✅ Derive only the traits you need (no accidental Serialize that exposes secrets)
- ✅ Load secrets from environment variables, never from config files
- ✅ Log level filters ensure `#[instrument]` skips secret fields

---

## 4. State Machine Pattern — Typed Enum Lifecycle

Use enums for state machines with controlled transitions. Each state transition is a method, never direct mutation.

```rust
/// Lifecycle status of a node during execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum NodeStatus {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
    Skipped,
}

impl NodeStatus {
    /// Returns true if the node is in a terminal state.
    pub fn is_terminal(&self) -> bool {
        matches!(
            self,
            NodeStatus::Completed | NodeStatus::Failed | NodeStatus::Skipped
        )
    }

    /// Returns true if the node can transition to Running.
    pub fn can_execute(&self) -> bool {
        matches!(self, NodeStatus::Ready)
    }

    /// Canonical snake_case name for serialization.
    pub fn as_str(&self) -> &'static str {
        match self {
            NodeStatus::Pending => "pending",
            NodeStatus::Ready => "ready",
            NodeStatus::Running => "running",
            NodeStatus::Completed => "completed",
            NodeStatus::Failed => "failed",
            NodeStatus::Skipped => "skipped",
        }
    }
}
```

### State Tracking Entity — Methods encapsulate transitions

```rust
pub struct NodeExecutionState {
    pub node_id: Uuid,
    pub node_name: String,
    pub status: NodeStatus,
    pub retry_attempts: u8,
    pub last_duration_ms: Option<u64>,
    pub total_duration_ms: u64,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl NodeExecutionState {
    pub fn new(node_id: Uuid, node_name: impl Into<String>) -> Self { ... }

    /// Transition methods — each encapsulates the state change + side effects.
    pub fn mark_ready(&mut self) {
        self.status = NodeStatus::Ready;
        self.ready_at = Some(Utc::now());
    }

    pub fn mark_completed(&mut self, duration_ms: u64) {
        self.status = NodeStatus::Completed;
        self.last_duration_ms = Some(duration_ms);
        self.total_duration_ms += duration_ms;
        self.completed_at = Some(Utc::now());
    }

    pub fn mark_for_retry(&mut self) {
        self.retry_attempts += 1;
        self.status = NodeStatus::Ready;
        self.last_duration_ms = None;
        self.started_at = None;
    }
}
```

### Rules
- ✅ State transitions are methods, not public field writes
- ✅ Each transition captures timestamp automatically
- ✅ `is_terminal()` on every state enum for pattern matching
- ✅ `as_str()` for canonical serialization names
- ❌ No direct `node.status = NodeStatus::Running` from outside the entity

---

## 5. RAII Reservation Pattern — Resource Guard

For resources that must be released (budgets, locks, file handles), use RAII guards:

```rust
/// RAII guard: reserves budget on creation, auto-returns on Drop.
pub struct LlmBudgetReservation {
    budget_id: Uuid,
    amount: u64,
    released: bool,
}

impl LlmBudgetReservation {
    pub fn new(budget_id: Uuid, amount: u64) -> Self {
        Self { budget_id, amount, released: false }
    }

    /// Manually release the reservation before Drop.
    pub fn release(mut self) {
        self.released = true;
        // Return budget to pool
    }
}

impl Drop for LlmBudgetReservation {
    fn drop(&mut self) {
        if !self.released {
            // Auto-return budget on scope exit
            tracing::warn!("Budget reservation dropped without release");
        }
    }
}
```

### Usage Pattern

```rust
fn execute_llm_call(budget: &mut LlmBudget) -> Result<(), BudgetError> {
    let reservation = budget.reserve(1000)?;  // Reserve 1000 tokens
    // ... make LLM call ...
    // reservation is auto-released on scope exit via Drop
    Ok(())
}
```

---

## 6. Async Patterns — tokio JoinSet for Parallelism

```rust
use tokio::task::JoinSet;

pub async fn execute_parallel(nodes: Vec<TaskNode>, max_concurrent: u32) -> ExecutionResult {
    let mut join_set = JoinSet::new();
    let mut results = HashMap::new();

    for node in nodes.into_iter().take(max_concurrent as usize) {
        join_set.spawn(execute_node(node));
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(task_result)) => {
                results.insert(task_result.node_id, task_result);
            }
            Ok(Err(e)) => {
                // Handle node execution error
            }
            Err(join_error) => {
                // Handle task panic
            }
        }
    }

    ExecutionResult::from_results(results)
}
```

### Cancellation-Aware Sleep

```rust
use tokio_util::sync::CancellationToken;

pub async fn poll_with_cancellation(
    cancel: CancellationToken,
    interval: Duration,
) -> Result<(), Cancelled> {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(interval) => {
                // Do periodic work
            }
            _ = cancel.cancelled() => {
                return Err(Cancelled);
            }
        }
    }
}
```

### Rules
- ✅ Use `tokio::sync::mpsc::channel` (bounded) for cross-task communication
- ✅ Use `tokio::sync::broadcast` for fan-out pub-sub
- ✅ Use `tokio_util::sync::CancellationToken` for cooperative cancellation
- ✅ Use `tokio::select!` for timeout/cancellation-aware waits
- ❌ Never use `std::sync::Mutex` in async context — use `tokio::sync::Mutex` or `tokio::sync::RwLock`
- ❌ Never use unbounded channels (`mpsc::unbounded_channel`) without explicit justification
- ❌ Never block with `std::thread::sleep` in async code — use `tokio::time::sleep`

---

## 7. Domain Event Pattern — Tagged Union Enum

```rust
/// All possible events in the system.
///
/// # Contract (Frozen)
/// - Every variant carries execution_id and timestamp for correlation
/// - Serialized as tagged union with `#[serde(tag = "type")]`
/// - No implementation logic — pure data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ExecutionEvent {
    NodeStarted {
        execution_id: Uuid,
        node_id: String,
        node_name: String,
        timestamp: DateTime<Utc>,
    },
    NodeCompleted {
        execution_id: Uuid,
        node_id: String,
        duration_ms: u64,
        output: serde_json::Value,
        timestamp: DateTime<Utc>,
    },
}

impl ExecutionEvent {
    /// Canonical snake_case name of this variant.
    pub fn event_type_name(&self) -> &'static str {
        match self {
            ExecutionEvent::NodeStarted { .. } => "node_started",
            ExecutionEvent::NodeCompleted { .. } => "node_completed",
        }
    }

    /// Extract the common execution_id field.
    pub fn execution_id(&self) -> &Uuid {
        match self {
            ExecutionEvent::NodeStarted { execution_id, .. }
            | ExecutionEvent::NodeCompleted { execution_id, .. } => execution_id,
        }
    }

    /// Convenience constructors.
    pub fn new_node_started(eid: Uuid, node_id: String, node_name: String) -> Self {
        Self::NodeStarted { execution_id: eid, node_id, node_name, timestamp: Utc::now() }
    }
}
```

### Rules
- ✅ Every event has `execution_id: Uuid` and `timestamp: DateTime<Utc>`
- ✅ Serialized as tagged union: `#[serde(tag = "type", rename_all = "snake_case")]`
- ✅ Provide helper methods: `event_type_name()`, `execution_id()`, `is_terminal()`
- ✅ Provide convenience constructors: `Event::new_*()`
- ✅ Write round-trip serde test for every variant
- ❌ No logic in event types — they are pure data

---

## 8. Configuration Pattern — Multi-Source Merging

```rust
/// Merge order: CLI flags > Environment > Config file > Defaults
pub struct ConfigService {
    /// Parsed rigorix.toml
    file_config: Option<Config>,
    /// Environment variables (RIGORIX_*)
    env_config: Config,
    /// Programmatic defaults
    defaults: Config,
}

impl ConfigService {
    pub fn load(cli_overrides: CliConfig) -> Result<Config, ConfigError> {
        let mut config = Config::default();  // Start with defaults

        // Layer 1: Config file
        if let Some(file) = Self::load_config_file()? {
            config.merge(file);
        }

        // Layer 2: Environment variables
        config.merge(Self::load_from_env()?);

        // Layer 3: CLI flags (highest precedence)
        config.merge(cli_overrides);

        config.validate()?;
        Ok(config)
    }

    fn load_from_env() -> Result<Config, ConfigError> {
        let mut config = Config::default();
        if let Ok(val) = std::env::var("RIGORIX_LOG") {
            config.observability.log_level = val;
        }
        if let Ok(val) = std::env::var("RIGORIX_API_KEY") {
            config.llm.api_key = Secret::new(val);
        }
        Ok(config)
    }
}

/// Merge trait for layered config construction.
pub trait Merge {
    fn merge(&mut self, other: Self);
}
```

### Rules
- ✅ CLI flags override env vars which override config file which override defaults
- ✅ Secrets loaded ONLY from environment variables, never from config files
- ✅ `validate()` runs after merging — fail fast on startup
- ✅ Clear error messages for missing required fields

---

## 9. Atomic File Operations — Write-Rename

```rust
use std::fs;
use std::io::Write;
use std::path::Path;

/// Atomic file write: write to tmp → fsync → rename → fsync parent.
///
/// Guarantees that the target file is never in a partially-written state.
pub fn atomic_write(path: &Path, contents: &str) -> Result<(), IoError> {
    let tmp_path = path.with_extension("tmp");

    // Write to temp file
    let mut file = fs::File::create(&tmp_path)?;
    file.write_all(contents.as_bytes())?;

    // fsync to ensure data hits disk
    file.sync_all()?;

    // Atomic rename (POSIX guarantee within same filesystem)
    fs::rename(&tmp_path, path)?;

    // fsync parent directory to persist the directory entry
    if let Some(parent) = path.parent() {
        if let Ok(dir) = fs::File::open(parent) {
            dir.sync_all()?;
        }
    }

    Ok(())
}

/// Clean up orphan .tmp files on startup (crash recovery).
pub fn clean_orphan_tmp_files(dir: &Path) -> Result<(), IoError> {
    for entry in fs::read_dir(dir)? {
        let entry = entry?;
        if entry.path().extension().map_or(false, |e| e == "tmp") {
            fs::remove_file(entry.path())?;
        }
    }
    Ok(())
}
```

---

## 10. Builder Pattern — Complex Construction

For entities with many optional fields, use a builder:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    pub max_retries: u8,
    pub retry_on: Vec<FailureType>,
    pub retry_strategy: RetryStrategy,
    pub fallback_node: Option<Uuid>,
    pub backoff_ms: u64,
    pub backoff_multiplier: f64,
    pub max_backoff_ms: u64,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self {
            max_retries: 3,
            retry_on: vec![FailureType::Transient, FailureType::LspConflict],
            retry_strategy: RetryStrategy::SameOperation,
            fallback_node: None,
            backoff_ms: 100,
            backoff_multiplier: 2.0,
            max_backoff_ms: 30_000,
        }
    }
}

impl ExecutionPolicy {
    pub fn builder() -> ExecutionPolicyBuilder {
        ExecutionPolicyBuilder::default()
    }
}

#[derive(Default)]
pub struct ExecutionPolicyBuilder {
    max_retries: u8,
    retry_on: Vec<FailureType>,
    retry_strategy: RetryStrategy,
    fallback_node: Option<Uuid>,
    backoff_ms: u64,
    backoff_multiplier: f64,
    max_backoff_ms: u64,
}

impl ExecutionPolicyBuilder {
    pub fn with_max_retries(mut self, val: u8) -> Self {
        self.max_retries = val;
        self
    }

    pub fn with_retry_strategy(mut self, val: RetryStrategy) -> Self {
        self.retry_strategy = val;
        self
    }

    pub fn build(self) -> ExecutionPolicy {
        ExecutionPolicy {
            max_retries: self.max_retries,
            retry_on: self.retry_on,
            retry_strategy: self.retry_strategy,
            fallback_node: self.fallback_node,
            backoff_ms: self.backoff_ms,
            backoff_multiplier: self.backoff_multiplier,
            max_backoff_ms: self.max_backoff_ms,
        }
    }
}
```

Also consider named constructors for common configurations:

```rust
impl ExecutionPolicy {
    /// No retries — only one attempt.
    pub fn no_retry() -> Self {
        Self { max_retries: 0, ..Default::default() }
    }

    /// Aggressive retry for transient failures.
    pub fn aggressive_retry() -> Self {
        Self {
            max_retries: 5,
            retry_on: vec![FailureType::Transient],
            backoff_ms: 50,
            backoff_multiplier: 1.5,
            ..Default::default()
        }
    }
}
```

---

## 11. Retry/Backoff Pattern

```rust
/// Strategy for computing delay between retry attempts.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BackoffStrategy {
    Fixed { base_delay_ms: u64 },
    Exponential { base_delay_ms: u64, multiplier: f64, max_delay_ms: u64 },
    Linear { base_delay_ms: u64, step_ms: u64, max_delay_ms: u64 },
    Immediate,
}

impl Default for BackoffStrategy {
    fn default() -> Self {
        Self::Exponential {
            base_delay_ms: 100,
            multiplier: 2.0,
            max_delay_ms: 30_000,
        }
    }
}

impl BackoffStrategy {
    /// Compute delay in milliseconds for a given retry attempt (0-indexed).
    pub fn compute_delay_ms(&self, attempt: u8) -> u64 {
        match self {
            Self::Fixed { base_delay_ms } => *base_delay_ms,
            Self::Exponential { base_delay_ms, multiplier, max_delay_ms } => {
                let delay = (*base_delay_ms as f64 * multiplier.powi(attempt as i32)) as u64;
                delay.min(*max_delay_ms)
            }
            Self::Linear { base_delay_ms, step_ms, max_delay_ms } => {
                let delay = *base_delay_ms + (*step_ms * attempt as u64);
                delay.min(*max_delay_ms)
            }
            Self::Immediate => 0,
        }
    }
}

/// Decision about whether and how to retry a failed operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum RetryDecision {
    Retry { strategy: RetryStrategy, attempt: u8, backoff_ms: u64, reason: String },
    Fallback { fallback_node_id: Uuid, reason: String },
    Skip { reason: String },
    Abort { reason: String },
}
```

---

## 12. EventBus — Pub-Sub with Broadcast Channel

```rust
use tokio::sync::broadcast;

pub struct EventBus {
    tx: broadcast::Sender<ExecutionEvent>,
    /// In-memory append-only log for drain-at-end persistence.
    log: Vec<PersistedEvent>,
    sequence: u64,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx, log: Vec::new(), sequence: 0 }
    }

    pub fn publish(&mut self, event: ExecutionEvent) -> Result<(), EventSystemError> {
        self.sequence += 1;
        let persisted = PersistedEvent {
            sequence: self.sequence,
            event,
        };

        // Store in memory log
        self.log.push(persisted.clone());

        // Broadcast to subscribers (non-blocking — drops if no receivers)
        let _ = self.tx.send(persisted.event.clone());

        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ExecutionEvent> {
        self.tx.subscribe()
    }

    /// Drain log for persistence/audit at end of execution.
    pub fn drain(&mut self) -> Vec<PersistedEvent> {
        self.log.drain(..).collect()
    }
}
```

---

## 13. Testing Patterns

### Unit Tests — Inline with `#[cfg(test)]`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // AAA Pattern: Arrange → Act → Assert
    #[test]
    fn test_policy_compute_delay_exponential() {
        // Arrange
        let policy = BackoffStrategy::Exponential {
            base_delay_ms: 100,
            multiplier: 2.0,
            max_delay_ms: 30_000,
        };

        // Act
        let delay = policy.compute_delay_ms(2);  // 3rd attempt

        // Assert
        assert_eq!(delay, 400);  // 100 * 2^2 = 400
    }

    #[test]
    fn test_node_status_terminal() {
        assert!(NodeStatus::Completed.is_terminal());
        assert!(NodeStatus::Failed.is_terminal());
        assert!(!NodeStatus::Running.is_terminal());
    }

    #[test]
    fn test_node_execution_state_transitions() {
        let mut state = NodeExecutionState::new(Uuid::new_v4(), "test");

        assert_eq!(state.status, NodeStatus::Pending);

        state.mark_ready();
        assert_eq!(state.status, NodeStatus::Ready);

        state.mark_completed(100);
        assert_eq!(state.status, NodeStatus::Completed);
        assert!(state.is_terminal());
    }
}
```

### Serde Round-Trip Tests — For Every Serialized Type

```rust
#[test]
fn test_serde_roundtrip_node_completed() {
    let eid = Uuid::new_v4();
    let event = ExecutionEvent::new_node_completed(
        eid, "n1".into(), 250, serde_json::json!("done"),
    );

    let json = serde_json::to_string(&event).unwrap();
    let deserialized: ExecutionEvent = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.event_type_name(), "node_completed");
    assert_eq!(*deserialized.execution_id(), eid);
}
```

### Property-Based Tests

```rust
#[cfg(test)]
mod property_tests {
    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_backoff_never_exceeds_max(
            base in 1..1000u64,
            mult in 1.0..10.0f64,
            max in 1000..100_000u64,
            attempt in 0..10u8,
        ) {
            let strategy = BackoffStrategy::Exponential {
                base_delay_ms: base,
                multiplier: mult,
                max_delay_ms: max,
            };
            let delay = strategy.compute_delay_ms(attempt);
            assert!(delay <= max, "Delay {delay} exceeds max {max}");
        }
    }
}
```

### Concurrency Tests

```rust
#[tokio::test]
async fn test_concurrent_execution() {
    let config = ParallelExecutorConfig::default();
    let (tx, mut rx) = tokio::sync::mpsc::channel(100);

    let handle = tokio::spawn(async move {
        // Run executor...
    });

    // Collect events within timeout
    let mut events = Vec::new();
    while let Some(event) = tokio::time::timeout(
        Duration::from_secs(5),
        rx.recv(),
    ).await.unwrap_or(None) {
        events.push(event);
    }
}
```

---

## 14. Documentation Standards

### Module-Level Docs (Every `mod.rs`)

```rust
//! [Module Name] — One-line purpose.
//!
//! @canonical .pi/architecture/modules/[module-name].md
//! Implements: Contract Freeze — [component list]
//! Issue: #[issue-number]
//!
//! [2-3 paragraph description of what this module does and how it works]
//!
//! # Architecture
//!
//! [Optional ASCII art or description of sub-module structure]
//!
//! # Dependencies
//!
//! - Depends on: [other modules]
//! - Used by: [other modules]
//!
//! # Contract (Frozen)
//!
//! - [List of frozen contract rules]
//! - No implementation logic beyond constructors and field accessors
```

### Public API Docs — Every Function and Type

```rust
/// Description of what this type/function does.
///
/// # Contract (Frozen)
/// - [Specific contract rules for this type]
///
/// # Examples
/// ```ignore
/// let policy = ExecutionPolicy::default();
/// ```
///
/// # Errors
/// - Returns `Error::InvalidConfig` if max_retries > 100
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPolicy { ... }
```

---

## 15. Anti-Patterns — NEVER DO

```rust
// ❌ anyhow in library code
use anyhow::Result;  // BAD — use thiserror

// ❌ Blocking in async context
async fn bad() {
    let data = std::fs::read_to_string("file");  // BAD — use tokio::fs
}

// ❌ Unbounded channels
let (tx, rx) = tokio::sync::mpsc::unbounded_channel();  // BAD — no backpressure

// ❌ unwrap/expect in production
let value = result.unwrap();  // BAD — use ? or proper error handling

// ❌ std::sync::Mutex in async code
let data = std::sync::Mutex::new(vec![]);  // BAD — use tokio::sync::Mutex

// ❌ Direct field mutation of state
node.status = NodeStatus::Running;  // BAD — use transition methods

// ❌ Hardcoded constants without justification
const MAX_RETRIES: u8 = 3;  // OK if documented. BAD if magic number.

// ❌ Mixed responsibilities in one module
// A module should cover ONE bounded context

// ❌ Stringly-typed errors
Err("something went wrong".into())  // BAD — use typed error enums

// ❌ Logging secrets
info!("API key: {}", secret.expose());  // BAD — Secret::Debug is redacted

// ❌ Direct thread::sleep in async
std::thread::sleep(Duration::from_secs(1));  // BAD — use tokio::time::sleep
```

---

## 16. Cargo.toml Conventions

```toml
[package]
name = "rigorix-module-name"
version = "0.1.0"
edition = "2024"
description = "One-line description of this crate"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["full"] }
tokio-util = "0.7"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

[dev-dependencies]
tempfile = "3"
criterion = { version = "0.5", features = ["html_reports"] }
proptest = "1"

[features]
live-tests = []  # Flag for tests that hit real APIs (LLM, network)
```

---

## 17. Retry on Transient Errors — HTTP Client Pattern

```rust
async fn make_api_call(client: &reqwest::Client, api_key: &str) -> Result<Response, ApiError> {
    let max_retries = 3;
    let mut last_error = String::new();

    for attempt in 0..max_retries {
        let response = client
            .post("https://api.example.com/v1/messages")
            .header("x-api-key", api_key)
            .body(request_body)
            .send()
            .await
            .map_err(|e| ApiError::transient(e.to_string()))?;

        let status = response.status();

        if status.is_success() {
            return Ok(response);
        }

        if status.as_u16() == 429 || status.as_u16() >= 500 {
            // Rate limited or server error — retry with backoff
            last_error = format!("Status {}", status.as_u16());
            let backoff_ms = 1000 * 2u64.pow(attempt as u32);
            tokio::time::sleep(Duration::from_millis(backoff_ms)).await;
            continue;
        }

        // Client error (4xx) — fatal, don't retry
        return Err(ApiError::fatal(status.as_u16(), response.text().await?));
    }

    Err(ApiError::max_retries_exhausted(max_retries, last_error))
}
```

---

*Version: 2.0.0*
*Last updated: 2026-06-16*
*Source: rigorix-engine codebase patterns + DDD architecture analysis*
*Validated against: ADR-001 through ADR-012*
