# Rust Code Patterns

> **Purpose:** Reusable Rust patterns for Guardian projects.
> **Source:** Extracted from Rigorix framework.

---

## Error Handling

```rust
// Use thiserror for all errors (never anyhow in library code)
use thiserror::Error;

#[derive(Error, Debug)]
pub enum MyError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("Parse error at line {line}: {message}")]
    Parse { line: usize, message: String },

    #[error("Invalid configuration: {0}")]
    Config(String),
}

// Automatic conversion with #[from]
fn read_file(path: &str) -> Result<String, MyError> {
    std::fs::read_to_string(path)?; // Auto-converts io::Error → MyError
    Ok(content)
}
```

---

## Tracing / Logging

```rust
use tracing::{instrument, info, warn, error};

// Add #[instrument] to public functions
#[instrument(skip(non_debug_param), fields(user_id = %user_id))]
pub async fn process_request(user_id: Uuid, non_debug_param: Vec<u8>) -> Result<()> {
    info!("Processing request");
    // ...
    warn!("Something unusual happened");
    // ...
}
```

---

## Cancellation / Cleanup

```rust
use tokio_util::sync::CancellationToken;

pub async fn long_running_task(cancel_token: CancellationToken) -> Result<()> {
    loop {
        // Check cancellation in loops
        if cancel_token.is_cancelled() {
            info!("Task cancelled, cleaning up");
            break;
        }

        // Do work
        do_work()?;

        // Sleep with cancellation awareness
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_secs(1)) => {},
            _ = cancel_token.cancelled() => break,
        }
    }
    Ok(())
}
```

---

## Atomic Writes

```rust
use std::fs;
use std::path::Path;

// Write-rename pattern for atomic file persistence
pub fn atomic_write(path: &Path, content: &str) -> std::io::Result<()> {
    let temp_path = path.with_extension("tmp");

    // Write to temp file
    fs::write(&temp_path, content)?;

    // Atomic rename (on POSIX systems)
    fs::rename(&temp_path, path)?;

    Ok(())
}
```

---

## Async Patterns

```rust
// Use tokio for async runtime
use tokio::sync::{Mutex, RwLock};

// RwLock for read-heavy workloads
pub struct Cache {
    data: RwLock<HashMap<String, Data>>,
}

// Mutex for write-heavy or complex state
pub struct StateMachine {
    state: Mutex<State>,
}

// Prefer async-friendly types
// ✅ Use tokio::fs instead of std::fs in async context
// ❌ Never block in async context (no std::fs::read_to_string)
```

---

## Testing

```rust
// Unit tests inline
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse() {
        let result = parse("input");
        assert!(result.is_ok());
    }
}

// Integration tests in tests/ directory
// tests/integration_test.rs
use rigorix::*;

#[tokio::test]
async fn test_full_flow() {
    let result = run_flow();
    assert!(result.is_ok());
}
```

---

## Anti-Patterns (NEVER DO)

```rust
// ❌ Using anyhow in library code
use anyhow::Result;  // BAD

// ✅ Use thiserror for library errors
use thiserror::Error;  // GOOD

// ❌ Blocking in async context
async fn bad() {
    let data = std::fs::read_to_string("file");  // BAD - blocks
}

// ✅ Use async-friendly APIs
async fn good() {
    let data = tokio::fs::read_to_string("file").await;  // GOOD
}

// ❌ Unbounded channels
let (tx, rx) = mpsc::unbounded_channel();  // BAD

// ✅ Bounded channels with backpressure
let (tx, rx) = mpsc::channel(100);  // GOOD

// ❌ unwrap() in production code
let value = result.unwrap();  // BAD

// ✅ Proper error handling
let value = result?;  // GOOD
// Or with context
let value = result.map_err(|e| MyError::Context(e))?;
```

---

## Build Commands

```bash
# Build
cargo build

# Build release
cargo build --release

# Test
cargo test --all

# Test specific
cargo test --test integration_test

# Lint
cargo clippy -- -D warnings

# Format
cargo fmt --check

# Security audit
cargo audit

# Coverage
cargo tarpaulin --out Html
```

---

## Dependencies

```toml
# Cargo.toml standard deps
[dependencies]
tokio = { version = "1", features = ["full"] }
thiserror = "1"
tracing = "0.1"
tracing-subscriber = "0.3"
serde = { version = "1", features = ["derive"] }

[dev-dependencies]
tokio-test = "0.4"
```