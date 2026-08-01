//! Tracing initialization — `tracing-subscriber` setup.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#tracing
//! Implements: Contract Freeze — TracingInit component
//! Issue: issue-contract-freeze
//!
//! # Contract (Frozen)
//!
//! Tracing is initialised at startup before any command dispatch. The
//! `RIGORIX_LOG` environment variable controls the log filter (same
//! syntax as `RUST_LOG`). When not set, defaults to `info` level.
//!
//! The global subscriber is installed once. Calling `init_tracing()`
//! a second time is a no-op (tracing-subscriber handles this via
//! `try_init`).

/// Initialise the global tracing subscriber.
///
/// Reads the `RIGORIX_LOG` environment variable for the filter level.
/// Falls back to `info` if not set. Supports structured JSON output
/// via `tracing-subscriber`'s JSON layer.
///
/// # Panics
///
/// This function will panic only if the global subscriber has already
/// been set by external code (not from within this crate).
///
/// # Implementation Notes
///
/// - Use `tracing_subscriber::registry()` with `EnvFilter` and `fmt` layer
/// - Use `tracing_subscriber::fmt::format::Json` for `--format json`
/// - Respect `RIGORIX_LOG` env var (not `RUST_LOG`, to avoid conflicts)
/// - Call `tracing_subscriber::fmt().with_env_filter(...).init()`
pub fn init_tracing() {
    // Delegate to the engine's centralized observability layer so the
    // `rigorix_engine::observability` module is the single tracing
    // initializer across all entry points. RIGORIX_LOG controls the level;
    // debug builds get human-readable output, release builds JSON.
    use rigorix_engine::observability::TracingConfig;
    if let Err(e) = rigorix_engine::observability::init_tracing(&TracingConfig::default()) {
        eprintln!("Failed to initialize tracing: {e}");
    }
}
