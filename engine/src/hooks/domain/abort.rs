//! HookAbortSignal — cooperative cancellation for hook execution.
//!
//! @canonical .pi/architecture/modules/hooks.md#hook-abort
//! Implements: Contract Freeze — HookAbortSignal struct
//! Issue: #410
//!
//! Provides an atomic abort flag that hooks can check for cooperative
//! cancellation. When the signal is set, hook commands should terminate
//! as soon as possible. The `HookRunner` checks this signal between
//! hook executions and before spawning new processes.
//!
//! # Contract (Frozen)
//! - Thread-safe (uses `AtomicBool`)
//! - Cloneable — clones share the same underlying flag
//! - Default is unset (execution proceeds normally)
//! - Once set, it cannot be unset

use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

/// A shared, thread-safe abort signal for cooperative hook cancellation.
///
/// When the signal is triggered, all running and pending hook commands
/// should stop as soon as safely possible.
///
/// # Example
///
/// ```rust
/// use rigorix_engine::hooks::domain::HookAbortSignal;
///
/// let signal = HookAbortSignal::new();
/// assert!(!signal.is_aborted());
///
/// signal.abort();
/// assert!(signal.is_aborted());
/// ```
#[derive(Debug, Clone)]
pub struct HookAbortSignal {
    inner: Arc<AtomicBool>,
}

impl HookAbortSignal {
    /// Create a new abort signal in the unset (not aborted) state.
    pub fn new() -> Self {
        Self {
            inner: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Create a new abort signal that is already in the aborted state.
    ///
    /// Useful for tests or when cancellation is requested before
    /// hook execution begins.
    pub fn new_aborted() -> Self {
        Self {
            inner: Arc::new(AtomicBool::new(true)),
        }
    }

    /// Trigger the abort signal.
    ///
    /// Once set, all clones of this signal will also return `true`
    /// from `is_aborted()`. This operation is idempotent.
    pub fn abort(&self) {
        self.inner.store(true, Ordering::SeqCst);
    }

    /// Check whether the abort signal has been triggered.
    pub fn is_aborted(&self) -> bool {
        self.inner.load(Ordering::SeqCst)
    }

    /// Reset the abort signal to unset.
    ///
    /// # Safety
    ///
    /// This creates a new `AtomicBool` and replaces the inner `Arc`.
    /// All existing clones of the previous signal will still see the
    /// old value. Only use this when you are sure no other references
    /// to the old signal exist.
    pub fn reset(&mut self) {
        self.inner = Arc::new(AtomicBool::new(false));
    }
}

impl Default for HookAbortSignal {
    fn default() -> Self {
        Self::new()
    }
}

impl From<bool> for HookAbortSignal {
    /// Create a signal from a boolean. `true` creates an aborted signal,
    /// `false` creates an unset signal.
    fn from(aborted: bool) -> Self {
        if aborted {
            Self::new_aborted()
        } else {
            Self::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_is_not_aborted() {
        let signal = HookAbortSignal::new();
        assert!(!signal.is_aborted());
    }

    #[test]
    fn test_new_aborted() {
        let signal = HookAbortSignal::new_aborted();
        assert!(signal.is_aborted());
    }

    #[test]
    fn test_abort_sets_flag() {
        let signal = HookAbortSignal::new();
        signal.abort();
        assert!(signal.is_aborted());
    }

    #[test]
    fn test_abort_is_idempotent() {
        let signal = HookAbortSignal::new();
        signal.abort();
        signal.abort(); // Should not panic
        assert!(signal.is_aborted());
    }

    #[test]
    fn test_clone_shares_flag() {
        let signal = HookAbortSignal::new();
        let cloned = signal.clone();
        signal.abort();
        assert!(cloned.is_aborted());
        assert!(signal.is_aborted());
    }

    #[test]
    fn test_reset() {
        let mut signal = HookAbortSignal::new();
        signal.abort();
        assert!(signal.is_aborted());
        signal.reset();
        assert!(!signal.is_aborted());
    }

    #[test]
    fn test_from_bool_true_is_aborted() {
        let signal: HookAbortSignal = true.into();
        assert!(signal.is_aborted());
    }

    #[test]
    fn test_from_bool_false_is_not_aborted() {
        let signal: HookAbortSignal = false.into();
        assert!(!signal.is_aborted());
    }

    #[test]
    fn test_default_trait() {
        let signal = HookAbortSignal::default();
        assert!(!signal.is_aborted());
    }

    #[test]
    fn test_debug_output() {
        let signal = HookAbortSignal::new();
        let debug = format!("{:?}", signal);
        assert!(debug.contains("HookAbortSignal"));
    }

    #[test]
    fn test_concurrent_access() {
        use std::thread;
        let signal = HookAbortSignal::new();
        let cloned = signal.clone();

        let handle = thread::spawn(move || {
            cloned.abort();
        });

        handle.join().unwrap();
        assert!(signal.is_aborted());
    }
}
