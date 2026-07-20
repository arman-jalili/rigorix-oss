---
name: rust-enterprise-codegen
description: Full reference for Rust enterprise code generation with DDD + Clean Architecture. 17 sections covering module structure, tactical patterns, error handling, async, testing, and anti-patterns. Loaded on-demand — never inline. Use via agents/rust-codegen.md skill.
---

# Rust Enterprise Code Generation — DDD + Clean Architecture

> Canonical skill for generating production-grade Rust code.
> All code MUST follow these patterns. Validators enforce compliance.
>
> Source: rigorix-engine (17 frozen-contract modules) + DDD architecture analysis + Clean Architecture principles.

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
│       └── mod.rs
├── infrastructure/   # Repository implementations, external adapters
│   ├── mod.rs
│   ├── repository/   # Repository trait definitions
│   │   ├── mod.rs
│   │   └── [entity]_repository.rs
│   └── persistence/  # ORM/database implementations
└── interfaces/       # API contracts (HTTP, events)
    ├── mod.rs
    └── http/         # REST endpoint contracts
        ├── mod.rs
        ├── routes.rs
        └── dto.rs    # Request/Response DTOs
```

### Dependency Direction Rule (Inward Dependency)

```
domain → application → infrastructure → interfaces
         ↑                    ↑
         └── interior layers never depend on outer layers
```

- **domain/** — depends on nothing except serde, chrono, uuid (pure data)
- **application/** — depends on domain
- **infrastructure/** — depends on application (implements domain/application traits)
- **interfaces/** — depends on application (translates HTTP/events to domain calls)

### Module Header Pattern

Every `mod.rs` MUST include a canonical reference header:

```rust
//! Module Purpose — One-line summary of what this module does.
//!
//! @canonical .pi/architecture/modules/[module-name].md#[section]
//! Implements: Contract Freeze — [component names]
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

---

## 2. DDD Tactical Patterns

### Aggregate Root

The aggregate root is the entry point for all operations within its boundary. It enforces invariants and coordinates entity state changes.

```rust
/// Aggregate root for the [Module] bounded context.
///
/// # Contract (Frozen)
/// - Aggregate roots are the ONLY way to modify entities within their boundary
/// - Methods return Result<(), Error> instead of panicking
/// - All mutations go through aggregate methods, never direct field access
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ModuleAggregate {
    id: Uuid,
    status: ModuleStatus,
    entities: Vec<ChildEntity>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

impl ModuleAggregate {
    pub fn new(id: Uuid) -> Self {
        Self {
            id,
            status: ModuleStatus::Pending,
            entities: Vec::new(),
            created_at: Utc::now(),
            updated_at: Utc::now(),
        }
    }

    /// Execute a state transition — returns events for side effects.
    pub fn execute(&mut self, command: ModuleCommand) -> Result<Vec<DomainEvent>, ModuleError> {
        match command {
            ModuleCommand::Start => self.start(),
            ModuleCommand::Complete { result } => self.complete(result),
            ModuleCommand::Cancel { reason } => self.cancel(reason),
        }
    }

    fn start(&mut self) -> Result<Vec<DomainEvent>, ModuleError> {
        if !self.status.can_start() {
            return Err(ModuleError::InvalidState {
                current: self.status,
                expected: ModuleStatus::Ready,
            });
        }
        self.status = ModuleStatus::Running;
        self.updated_at = Utc::now();
        Ok(vec![DomainEvent::ModuleStarted { aggregate_id: self.id, timestamp: Utc::now() }])
    }

    fn complete(&mut self, result: serde_json::Value) -> Result<Vec<DomainEvent>, ModuleError> {
        if !self.status.can_complete() {
            return Err(ModuleError::InvalidState {
                current: self.status,
                expected: ModuleStatus::Running,
            });
        }
        self.status = ModuleStatus::Completed;
        self.updated_at = Utc::now();
        Ok(vec![DomainEvent::ModuleCompleted { aggregate_id: self.id, result, timestamp: Utc::now() }])
    }

    fn cancel(&mut self, reason: String) -> Result<Vec<DomainEvent>, ModuleError> {
        if self.status.is_terminal() {
            return Err(ModuleError::InvalidState {
                current: self.status,
                expected: ModuleStatus::Pending,
            });
        }
        self.status = ModuleStatus::Cancelled;
        self.updated_at = Utc::now();
        Ok(vec![DomainEvent::ModuleCancelled { aggregate_id: self.id, reason, timestamp: Utc::now() }])
    }
}
```

### Value Object

Value objects are immutable, interchangeable, and defined by their attributes, not identity.

```rust
/// Value object — identified by structural equality, not identity.
///
/// # Contract (Frozen)
/// - Immutable: all fields are read-only after construction
/// - Self-validating: constructor validates invariants
/// - Eq + Hash based on ALL fields
/// - No setters — create a new instance to change
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Money {
    amount: i64,  // Stored in smallest currency unit (cents, pennys)
    currency: Currency,
}

impl Money {
    pub fn new(amount: i64, currency: Currency) -> Result<Self, ModuleError> {
        if amount < 0 {
            return Err(ModuleError::ValidationError {
                field: "amount",
                message: "Amount must be non-negative",
            });
        }
        Ok(Self { amount, currency })
    }

    pub fn amount(&self) -> i64 { self.amount }
    pub fn currency(&self) -> &Currency { &self.currency }

    pub fn add(&self, other: &Self) -> Result<Self, ModuleError> {
        if self.currency != other.currency {
            return Err(ModuleError::CurrencyMismatch {
                left: self.currency.clone(),
                right: other.currency.clone(),
            });
        }
        Ok(Self { amount: self.amount + other.amount, currency: self.currency.clone() })
    }
}
```

### Repository Pattern

Repositories provide collection-like access to aggregates. The interface is defined in `domain/` or `application/`, implemented in `infrastructure/`.

```rust
/// Repository trait — defined in domain, implemented in infrastructure.
///
/// # Contract (Frozen)
/// - Interface defined in domain/, implementation in infrastructure/
/// - Methods return domain types (not ORM entities)
/// - Repository methods express the ubiquitous language
pub trait ModuleRepository: Send + Sync {
    fn find_by_id(&self, id: Uuid) -> Result<Option<ModuleAggregate>, ModuleError>;
    fn save(&mut self, aggregate: &ModuleAggregate) -> Result<(), ModuleError>;
    fn delete(&mut self, id: Uuid) -> Result<(), ModuleError>;
    fn find_by_status(&self, status: ModuleStatus) -> Result<Vec<ModuleAggregate>, ModuleError>;
}
```

### Domain Event

Domain events capture something meaningful that happened in the domain.

```rust
/// Domain events for the [Module] bounded context.
///
/// # Contract (Frozen)
/// - Every event carries aggregate_id and timestamp for correlation
/// - Serialized as tagged union with `#[serde(tag = "type")]`
/// - Events are facts — immutable and append-only
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainEvent {
    ModuleStarted {
        aggregate_id: Uuid,
        timestamp: DateTime<Utc>,
    },
    ModuleCompleted {
        aggregate_id: Uuid,
        result: serde_json::Value,
        timestamp: DateTime<Utc>,
    },
    ModuleCancelled {
        aggregate_id: Uuid,
        reason: String,
        timestamp: DateTime<Utc>,
    },
}

impl DomainEvent {
    /// Canonical snake_case name of this event variant.
    pub fn event_type(&self) -> &'static str {
        match self {
            DomainEvent::ModuleStarted { .. } => "module_started",
            DomainEvent::ModuleCompleted { .. } => "module_completed",
            DomainEvent::ModuleCancelled { .. } => "module_cancelled",
        }
    }

    /// Extract the common aggregate_id for correlation.
    pub fn aggregate_id(&self) -> &Uuid {
        match self {
            DomainEvent::ModuleStarted { aggregate_id, .. }
            | DomainEvent::ModuleCompleted { aggregate_id, .. }
            | DomainEvent::ModuleCancelled { aggregate_id, .. } => aggregate_id,
        }
    }
}
```

### DDD Rules

- ✅ Aggregate roots are the **only** way to modify entities within their boundary
- ✅ Value objects are **immutable** after construction
- ✅ Repositories are interfaces in `domain/`, implementations in `infrastructure/`
- ✅ Domain events are **facts** — never modified after creation
- ✅ Every aggregate method returns `Result<Vec<DomainEvent>, Error>` for side effects
- ✅ Ubiquitous language in method and type names (not technology terms)
- ❌ No anemic domain models (entities with just getters/setters)
- ❌ No infrastructure concerns leaking into domain
- ❌ No `pub` fields on aggregates — always encapsulate
- ❌ No cross-aggregate references — use IDs, not object references

---

## 3. Error Handling — thiserror with Aggregation

### Per-Module Error Enum

```rust
use thiserror::Error;

/// Typed error enum for the [Module] bounded context.
///
/// # Contract (Frozen)
/// - Every error variant follows the pattern: `PascalCase { fields }`
/// - `#[error("...")]` Display messages are user-readable
/// - Implement `is_retriable()` for transient failures
/// - Derive `Serialize + Deserialize` for API responses
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum ModuleError {
    /// Resource not found — include what was requested and what's available.
    #[error("Not found: {id}. Available: {available:?}")]
    NotFound { id: String, available: Vec<String> },

    /// Invalid state transition attempt.
    #[error("Invalid state transition: {current} → {expected}")]
    InvalidState { current: ModuleStatus, expected: ModuleStatus },

    /// Duplicate identifier.
    #[error("Duplicate ID: {id}")]
    DuplicateId { id: Uuid },

    /// Validation failure — field-level error details.
    #[error("Validation failed: {message}")]
    ValidationError { field: &'static str, message: String },

    /// Operation was cancelled.
    #[error("Operation cancelled")]
    Cancelled,
}

impl ModuleError {
    /// Returns true if the error represents a transient failure.
    pub fn is_retriable(&self) -> bool {
        false // Default: no transient errors unless explicitly marked
    }
}
```

### Root Error Aggregation

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

### Error Source Chains

```rust
#[derive(Error, Debug)]
#[error("planning failed: {context}")]
pub struct PlanningFailure {
    context: String,
    #[source]
    cause: PlanningError,
}
```

### Rules

- ✅ Use `thiserror` for ALL library/domain errors
- ✅ Every error has a descriptive `#[error("...")]` message
- ✅ Include context in error fields (what was requested, what's available)
- ✅ Root error aggregates sub-errors via `#[from]` for `?` operator propagation
- ❌ NEVER use `anyhow` in library code — reserved for binary crates only
- ❌ NEVER use `.unwrap()` or `.expect()` in production code
- ❌ NEVER use `String` errors — always typed enums

---

## 4. Secret Handling — Redacted Value Object

```rust
/// A sensitive value (API key, token) that is redacted in all text output.
///
/// # Security
/// - Debug/Display show `[REDACTED]` (never leak)
/// - Only `.expose()` reveals the inner value
/// - Does NOT derive Serialize — secrets must not be serialized
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: impl Into<String>) -> Self { Self(value.into()) }
    pub fn expose(&self) -> &str { &self.0 }
    pub fn is_empty(&self) -> bool { self.0.is_empty() }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() { write!(f, "<empty>") }
        else { write!(f, "[REDACTED]") }
    }
}

impl fmt::Display for Secret { fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { fmt::Debug::fmt(self, f) } }
```

### Rules
- ✅ Wrap all API keys, tokens, passwords in `Secret`
- ✅ Load secrets from environment variables, never from config files
- ❌ Never derive Serialize on types that contain secrets
- ❌ Never log secrets — `Secret::Debug` is redacted

---

## 5. State Machine Pattern — Typed Enum Lifecycle

```rust
/// Lifecycle status of a [domain entity].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ModuleStatus {
    Pending,
    Ready,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl ModuleStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, ModuleStatus::Completed | ModuleStatus::Failed | ModuleStatus::Cancelled)
    }

    pub fn can_start(&self) -> bool { matches!(self, ModuleStatus::Ready) }
    pub fn can_complete(&self) -> bool { matches!(self, ModuleStatus::Running) }

    pub fn as_str(&self) -> &'static str {
        match self {
            ModuleStatus::Pending => "pending",
            ModuleStatus::Ready => "ready",
            ModuleStatus::Running => "running",
            ModuleStatus::Completed => "completed",
            ModuleStatus::Failed => "failed",
            ModuleStatus::Cancelled => "cancelled",
        }
    }
}
```

### State Tracking Entity

```rust
pub struct ModuleExecutionState {
    pub module_id: Uuid,
    pub status: ModuleStatus,
    pub retry_attempts: u8,
    pub started_at: Option<DateTime<Utc>>,
    pub completed_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

impl ModuleExecutionState {
    pub fn new(module_id: Uuid) -> Self { Self { module_id, status: ModuleStatus::Pending, retry_attempts: 0, started_at: None, completed_at: None, last_error: None } }

    pub fn mark_ready(&mut self) { self.status = ModuleStatus::Ready; self.started_at = None; }
    pub fn mark_running(&mut self) { self.status = ModuleStatus::Running; self.started_at = Some(Utc::now()); }
    pub fn mark_completed(&mut self) { self.status = ModuleStatus::Completed; self.completed_at = Some(Utc::now()); }
    pub fn mark_failed(&mut self, error: String) { self.status = ModuleStatus::Failed; self.last_error = Some(error); }
    pub fn mark_for_retry(&mut self) { self.retry_attempts += 1; self.status = ModuleStatus::Ready; }
}
```

### Rules
- ✅ State transitions are **methods**, not public field writes
- ✅ Each transition captures timestamp automatically
- ✅ `is_terminal()` on every state enum
- ❌ No direct field mutation from outside the entity

---

## 6. RAII Reservation Pattern — Resource Guard

```rust
/// RAII guard: reserves budget on creation, auto-returns on Drop.
pub struct BudgetReservation {
    budget_id: Uuid,
    amount: u64,
    released: bool,
}

impl BudgetReservation {
    pub fn new(budget_id: Uuid, amount: u64) -> Self { Self { budget_id, amount, released: false } }
    pub fn release(mut self) { self.released = true; /* Return budget to pool */ }
}

impl Drop for BudgetReservation {
    fn drop(&mut self) {
        if !self.released {
            tracing::warn!("Budget reservation dropped without release");
        }
    }
}
```

---

## 7. Async Patterns — tokio JoinSet for Parallelism

```rust
use tokio::task::JoinSet;

pub async fn execute_parallel(tasks: Vec<Task>, max_concurrent: u32) -> Result<Vec<TaskResult>, Error> {
    let mut join_set = JoinSet::new();
    let mut results = Vec::new();
    let mut iter = tasks.into_iter();

    for _ in 0..max_concurrent {
        if let Some(task) = iter.next() { join_set.spawn(execute_task(task)); }
    }

    while let Some(result) = join_set.join_next().await {
        match result {
            Ok(Ok(output)) => results.push(output),
            Ok(Err(e)) => return Err(e),
            Err(join_error) => return Err(Error::TaskPanicked(join_error.to_string())),
        }
        if let Some(task) = iter.next() { join_set.spawn(execute_task(task)); }
    }

    Ok(results)
}
```

### Cancellation-Aware Sleep

```rust
use tokio_util::sync::CancellationToken;

pub async fn poll_with_cancellation(cancel: CancellationToken, interval: Duration) -> Result<(), Error> {
    loop {
        tokio::select! {
            _ = tokio::time::sleep(interval) => { /* Do periodic work */ }
            _ = cancel.cancelled() => { return Err(Error::Cancelled("Polling cancelled".into())); }
        }
    }
}
```

### Rules
- ✅ Use `tokio::sync::mpsc::channel` (bounded) for cross-task communication
- ✅ Use `tokio::sync::broadcast` for fan-out pub-sub
- ✅ Use `tokio_util::sync::CancellationToken` for cooperative cancellation
- ✅ Use `std::sync::Mutex` for short critical sections that don't cross `.await` points
- ✅ Use `tokio::sync::Mutex` only when the lock must be held across `.await` points
- ❌ Never hold `std::sync::Mutex` across `.await` points
- ❌ Never use unbounded channels without explicit justification
- ❌ Never block with `std::thread::sleep` in async code

---

## 8. Domain Event Pattern — Tagged Union Enum

```rust
/// All possible events in the bounded context.
///
/// # Contract (Frozen)
/// - Every variant carries aggregate_id and timestamp for correlation
/// - Serialized as tagged union with `#[serde(tag = "type")]`
/// - No implementation logic — pure data
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum DomainEvent {
    Started { aggregate_id: Uuid, timestamp: DateTime<Utc> },
    Completed { aggregate_id: Uuid, result: serde_json::Value, timestamp: DateTime<Utc> },
}

impl DomainEvent {
    pub fn event_type(&self) -> &'static str {
        match self {
            DomainEvent::Started { .. } => "started",
            DomainEvent::Completed { .. } => "completed",
        }
    }
    pub fn aggregate_id(&self) -> &Uuid {
        match self {
            DomainEvent::Started { aggregate_id, .. }
            | DomainEvent::Completed { aggregate_id, .. } => aggregate_id,
        }
    }
}
```

### Rules
- ✅ Every event carries `aggregate_id` and `timestamp`
- ✅ Serialized as tagged union: `#[serde(tag = "type", rename_all = "snake_case")]`
- ✅ Provide helper methods: `event_type()`, `aggregate_id()`
- ✅ Write round-trip serde test for every variant
- ❌ No logic in event types — they are pure data

---

## 9. Configuration Pattern — Multi-Source Merging

```rust
/// Merge order: CLI flags > Environment > Config file > Defaults
pub trait Merge {
    fn merge(&mut self, other: Self);
}

pub struct ConfigService;

impl ConfigService {
    pub fn load(cli_overrides: CliConfig) -> Result<Config, ConfigError> {
        let mut config = Config::default();

        // Layer 1: Config file
        if let Some(file) = Self::load_config_file()? { config.merge(file); }

        // Layer 2: Environment variables (RIGORIX_*)
        config.merge(Self::load_from_env()?);

        // Layer 3: CLI flags (highest precedence)
        config.merge(cli_overrides);

        config.validate()?;
        Ok(config)
    }

    fn load_from_env() -> Result<Config, ConfigError> {
        let mut config = Config::default();
        if let Ok(val) = std::env::var("APP_LOG") { config.log_level = val; }
        if let Ok(val) = std::env::var("APP_API_KEY") { config.api_key = Secret::new(val); }
        Ok(config)
    }
}
```

### Rules
- ✅ CLI flags override env vars which override config file which override defaults
- ✅ Secrets loaded ONLY from environment variables
- ✅ `validate()` runs after merging — fail fast on startup

---

## 10. Atomic File Operations — Write-Rename

```rust
use std::fs;
use std::io::Write;
use std::path::Path;

/// Atomic file write: write to tmp → fsync → rename → fsync parent.
pub fn atomic_write(path: &Path, contents: &str) -> Result<(), IoError> {
    let tmp_path = path.with_extension("tmp");
    let mut file = fs::File::create(&tmp_path)?;
    file.write_all(contents.as_bytes())?;
    file.sync_all()?;
    fs::rename(&tmp_path, path)?;
    if let Some(parent) = path.parent() {
        if let Ok(dir) = fs::File::open(parent) { dir.sync_all()?; }
    }
    Ok(())
}

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

## 11. Builder Pattern — Complex Construction

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExecutionPolicy {
    pub max_retries: u8,
    pub retry_on: Vec<FailureType>,
    pub retry_strategy: RetryStrategy,
    pub backoff_ms: u64,
    pub backoff_multiplier: f64,
    pub max_backoff_ms: u64,
}

impl Default for ExecutionPolicy {
    fn default() -> Self {
        Self { max_retries: 3, retry_on: vec![FailureType::Transient], retry_strategy: RetryStrategy::SameOperation, backoff_ms: 100, backoff_multiplier: 2.0, max_backoff_ms: 30_000 }
    }
}

impl ExecutionPolicy {
    pub fn builder() -> ExecutionPolicyBuilder { ExecutionPolicyBuilder::default() }
    pub fn no_retry() -> Self { Self { max_retries: 0, ..Default::default() } }
    pub fn aggressive_retry() -> Self { Self { max_retries: 5, retry_on: vec![FailureType::Transient], backoff_ms: 50, backoff_multiplier: 1.5, ..Default::default() } }
}

#[derive(Default)]
pub struct ExecutionPolicyBuilder {
    max_retries: u8,
    retry_on: Vec<FailureType>,
    retry_strategy: RetryStrategy,
    backoff_ms: u64,
    backoff_multiplier: f64,
    max_backoff_ms: u64,
}

impl ExecutionPolicyBuilder {
    pub fn with_max_retries(mut self, val: u8) -> Self { self.max_retries = val; self }
    pub fn with_backoff(mut self, base_ms: u64, multiplier: f64, max_ms: u64) -> Self { self.backoff_ms = base_ms; self.backoff_multiplier = multiplier; self.max_backoff_ms = max_ms; self }
    pub fn build(self) -> ExecutionPolicy { ExecutionPolicy { max_retries: self.max_retries, retry_on: self.retry_on, retry_strategy: self.retry_strategy, backoff_ms: self.backoff_ms, backoff_multiplier: self.backoff_multiplier, max_backoff_ms: self.max_backoff_ms } }
}
```

---

## 12. Retry/Backoff Pattern

```rust
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BackoffStrategy {
    Fixed { base_delay_ms: u64 },
    Exponential { base_delay_ms: u64, multiplier: f64, max_delay_ms: u64 },
    Linear { base_delay_ms: u64, step_ms: u64, max_delay_ms: u64 },
    Immediate,
}

impl Default for BackoffStrategy {
    fn default() -> Self { Self::Exponential { base_delay_ms: 100, multiplier: 2.0, max_delay_ms: 30_000 } }
}

impl BackoffStrategy {
    /// Compute delay in milliseconds for retry attempt `n` (0-indexed).
    pub fn delay_ms(&self, attempt: u8) -> u64 {
        match self {
            Self::Fixed { base } => *base,
            Self::Exponential { base, mult, max } => (*base as f64 * mult.powi(attempt as i32)) as u64 - max,
            Self::Linear { base, step, max } => (*base + *step * attempt as u64).min(*max),
            Self::Immediate => 0,
        }
    }
}
```

---

## 13. EventBus — Pub-Sub with Broadcast Channel

```rust
use tokio::sync::broadcast;

pub struct EventBus {
    tx: broadcast::Sender<DomainEvent>,
    log: Vec<PersistedEvent>,
    sequence: u64,
}

impl EventBus {
    pub fn new(capacity: usize) -> Self {
        let (tx, _) = broadcast::channel(capacity);
        Self { tx, log: Vec::new(), sequence: 0 }
    }

    pub fn publish(&mut self, event: DomainEvent) -> Result<(), EventBusError> {
        self.sequence += 1;
        let persisted = PersistedEvent { sequence: self.sequence, event: event.clone() };
        self.log.push(persisted);
        let _ = self.tx.send(event);
        Ok(())
    }

    pub fn subscribe(&self) -> broadcast::Receiver<DomainEvent> { self.tx.subscribe() }
    pub fn drain(&mut self) -> Vec<PersistedEvent> { self.log.drain(..).collect() }
}
```

---

## 14. Testing Patterns

### Unit Tests — Inline with `#[cfg(test)]`

```rust
#[cfg(test)]
mod tests {
    use super::*;

    // AAA Pattern: Arrange → Act → Assert
    #[test]
    fn test_backoff_delay_exponential() {
        let strategy = BackoffStrategy::Exponential { base_delay_ms: 100, multiplier: 2.0, max_delay_ms: 30_000 };
        assert_eq!(strategy.delay_ms(2), 400);  // 100 * 2^2 = 400
    }

    #[test]
    fn test_status_terminal() {
        assert!(ModuleStatus::Completed.is_terminal());
        assert!(!ModuleStatus::Running.is_terminal());
    }

    #[test]
    fn test_aggregate_execute_start() {
        let mut agg = ModuleAggregate::new(Uuid::new_v4());
        let events = agg.execute(ModuleCommand::Start).unwrap();
        assert_eq!(agg.status, ModuleStatus::Running);
        assert_eq!(events.len(), 1);
    }
}
```

### Serde Round-Trip Tests — For Every Serialized Type

```rust
#[test]
fn test_domain_event_serde_roundtrip() {
    let eid = Uuid::new_v4();
    let event = DomainEvent::ModuleCompleted { aggregate_id: eid, result: serde_json::json!("done"), timestamp: Utc::now() };
    let json = serde_json::to_string(&event).unwrap();
    let deserialized: DomainEvent = serde_json::from_str(&json).unwrap();
    assert_eq!(*deserialized.aggregate_id(), eid);
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
        fn test_backoff_never_exceeds_max(base in 1..1000u64, mult in 1.0..10.0f64, max in 1000..100_000u64, attempt in 0..10u8) {
            let strategy = BackoffStrategy::Exponential { base_delay_ms: base, multiplier: mult, max_delay_ms: max };
            assert!(strategy.delay_ms(attempt) <= max);
        }
    }
}
```

---

## 15. Documentation Standards

### Module-Level Docs (Every `mod.rs`)

```rust
//! [Module Name] — One-line purpose.
//!
//! @canonical .pi/architecture/modules/[module-name].md
//! Implements: Contract Freeze — [component list]
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

### Public API Docs

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

## 16. Anti-Patterns — NEVER DO

```rust
// ❌ anyhow in library code
use anyhow::Result;  // BAD — use thiserror

// ❌ Blocking in async context
async fn bad() { let data = std::fs::read_to_string("file"); }  // BAD — use tokio::fs

// ❌ Unbounded channels
let (tx, rx) = tokio::sync::mpsc::unbounded_channel();  // BAD — no backpressure

// ❌ unwrap/expect in production
let value = result.unwrap();  // BAD — use ? or proper error handling

// ❌ std::sync::Mutex held across .await — blocks the runtime thread
async fn bad() { let guard = data.lock().unwrap(); tokio::time::sleep(...).await; }  // BAD

// ❌ Direct field mutation of state
node.status = NodeStatus::Running;  // BAD — use transition methods

// ❌ Stringly-typed errors
Err("something went wrong".into())  // BAD — use typed error enums

// ❌ Logging secrets
info!("API key: {}", secret.expose());  // BAD — Secret::Debug is redacted

// ❌ Direct thread::sleep in async
std::thread::sleep(Duration::from_secs(1));  // BAD — use tokio::time::sleep

// ❌ Anemic domain models — entities with only getters/setters and no behavior
pub struct AnemicEntity { pub id: Uuid, pub name: String }  // BAD — no domain logic

// ❌ Cross-aggregate references by object ref, not ID
pub struct Order { pub customer: Customer }  // BAD — use customer_id: Uuid

// ❌ Infrastructure leak in domain
use sqlx::PgPool;  // BAD — domain NEVER imports infrastructure concerns

// ❌ Handler with trait (handlers are concrete only)
pub trait UserHandler { ... }  // BAD — the route/handler fn IS the contract
pub struct UserHandlerImpl { ... }  // BAD — pointless indirection

// ❌ Handler importing repository directly (bypasses service layer)
async fn get_users(repo: impl UserRepository) {  // BAD — use application service
    let users = repo.find_all().await;
}

// Note: This project keeps repository traits + impls together in
// infrastructure/repository/ for consistency across existing codebases.
// Both traits and concrete implementations live side by side:
// infrastructure/repository/user_repository.rs  — both trait + impl
```

---

## 17. Cargo.toml Conventions

```toml
[package]
name = "module-name"
version = "0.1.0"
edition = "2024"
description = "One-line description of this crate"

[dependencies]
serde = { version = "1", features = ["derive"] }
serde_json = "1"
thiserror = "2"
tokio = { version = "1", features = ["sync", "time", "macros", "rt"] }
tokio-util = "0.7"
uuid = { version = "1", features = ["v4", "serde"] }
chrono = { version = "0.4", features = ["serde"] }
tracing = "0.1"

[dev-dependencies]
tempfile = "3"
proptest = "1"

[features]
live-tests = []  # Flag for tests that hit real APIs
```

---

*Version: 1.0.0*
*Last updated: 2026-07-03*
*Source: Guardian DDD patterns + context7 DDD reference (/jkazama/ddd-java, /ardalis/cleanarchitecture)*
