//! Implementation of `CancellationService`.
//!
//! @canonical .pi/architecture/modules/cancellation.md#manager
//! Implements: CancellationService trait — dual-level cancellation manager
//! Issue: issue-cancellationmanager
//!
//! Central manager for execution cancellation with Graceful and Immediate
//! shutdown levels. Uses `CancellationToken` (tokio-util) for coordinated
//! propagation to all concurrent tasks and a watch channel for signal
//! subscribers.

use async_trait::async_trait;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Instant;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;

use crate::cancellation::domain::{CancellationError, ShutdownSignal};

use super::dto::{
    CancelExecutionInput, CancelExecutionOutput, NotifyTaskCompleteInput, RegisterTaskInput,
    RegisterTaskOutput, ShutdownInput, ShutdownOutput, ShutdownStatusOutput,
};
use super::service::{CancellationService, CleanupHandler};

/// Implementation of `CancellationService`.
///
/// Manages execution cancellation with two shutdown levels:
/// - `Graceful`: Notify tasks to stop, let them finish naturally
/// - `Immediate`: Abort all in-flight work via `CancellationToken`
///
/// # Architecture
///
/// ```text
/// CancellationManagerImpl
/// ├── CancellationToken     — Propagated to all concurrent tasks
/// ├── Watch<Option<Signal>> — Subscribers get notified of signal changes
/// ├── Task counter          — Tracks running/completed/cancelled tasks
/// └── Cleanup handlers      — Registered per task type
/// ```
pub struct CancellationManagerImpl {
    /// The cancellation token propagated to all concurrent tasks.
    token: CancellationToken,
    /// Watch channel sender for shutdown signal subscribers.
    signal_tx: watch::Sender<Option<ShutdownSignal>>,
    /// Watch channel receiver (cloned for new subscribers).
    signal_rx: watch::Receiver<Option<ShutdownSignal>>,
    /// Whether cancellation has been requested.
    cancelled: AtomicBool,
    /// Number of tasks currently running.
    running_tasks: AtomicU32,
    /// Total tasks that have completed naturally.
    completed_tasks: AtomicU32,
    /// Total tasks that were cancelled/aborted.
    cancelled_tasks: AtomicU32,
    /// Timestamp when cancellation was requested.
    request_time: tokio::sync::Mutex<Option<Instant>>,
    /// Registered cleanup handlers keyed by task type.
    cleanup_handlers: tokio::sync::RwLock<Vec<(String, Box<dyn CleanupHandler>)>>,
    /// Default graceful shutdown timeout in seconds.
    #[allow(dead_code)]
    graceful_timeout_secs: u64,
}

impl CancellationManagerImpl {
    /// Create a new cancellation manager with the given graceful timeout.
    pub fn new(graceful_timeout_secs: u64) -> Self {
        let (signal_tx, signal_rx) = watch::channel(None);
        Self {
            token: CancellationToken::new(),
            signal_tx,
            signal_rx,
            cancelled: AtomicBool::new(false),
            running_tasks: AtomicU32::new(0),
            completed_tasks: AtomicU32::new(0),
            cancelled_tasks: AtomicU32::new(0),
            request_time: tokio::sync::Mutex::new(None),
            cleanup_handlers: tokio::sync::RwLock::new(Vec::new()),
            graceful_timeout_secs,
        }
    }

    /// Create a new cancellation manager with a parent token (child scope).
    pub fn child_of(parent_token: CancellationToken, graceful_timeout_secs: u64) -> Self {
        let (signal_tx, signal_rx) = watch::channel(None);
        // Create a child token that propagates from the parent
        let token = parent_token.child_token();
        Self {
            token,
            signal_tx,
            signal_rx,
            cancelled: AtomicBool::new(false),
            running_tasks: AtomicU32::new(0),
            completed_tasks: AtomicU32::new(0),
            cancelled_tasks: AtomicU32::new(0),
            request_time: tokio::sync::Mutex::new(None),
            cleanup_handlers: tokio::sync::RwLock::new(Vec::new()),
            graceful_timeout_secs,
        }
    }

    /// Register a cleanup handler for a specific task type.
    pub async fn register_cleanup_handler(
        &self,
        task_type: &str,
        handler: Box<dyn CleanupHandler>,
    ) {
        let mut handlers = self.cleanup_handlers.write().await;
        handlers.push((task_type.to_string(), handler));
    }
}

impl Default for CancellationManagerImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new(30) // Default 30-second graceful timeout
    }
}

#[async_trait]
impl CancellationService for CancellationManagerImpl {
    async fn request_graceful_shutdown(
        &self,
        _input: CancelExecutionInput,
    ) -> Result<CancelExecutionOutput, CancellationError> {
        // Check if already cancelling
        if self.cancelled.load(Ordering::SeqCst) {
            let current = self.signal_rx.borrow().clone();
            return Err(CancellationError::AlreadyCancelling {
                current_signal: current.unwrap_or(ShutdownSignal::Graceful),
            });
        }

        // Set cancelled flag
        self.cancelled.store(true, Ordering::SeqCst);
        *self.request_time.lock().await = Some(Instant::now());

        // Trigger the cancellation token
        self.token.cancel();

        // Send the signal on the watch channel
        let affected = self.running_tasks.load(Ordering::SeqCst);
        self.signal_tx
            .send(Some(ShutdownSignal::Graceful))
            .map_err(|_| CancellationError::NoSubscribers)?;

        Ok(CancelExecutionOutput {
            accepted: true,
            signal: ShutdownSignal::Graceful,
            affected_tasks: affected,
            was_already_cancelling: false,
        })
    }

    async fn request_immediate_abort(
        &self,
        _input: CancelExecutionInput,
    ) -> Result<CancelExecutionOutput, CancellationError> {
        // Check if already cancelling
        if self.cancelled.load(Ordering::SeqCst) {
            let current = self.signal_rx.borrow().clone();
            return Err(CancellationError::AlreadyCancelling {
                current_signal: current.unwrap_or(ShutdownSignal::Immediate),
            });
        }

        // Set cancelled flag
        self.cancelled.store(true, Ordering::SeqCst);
        *self.request_time.lock().await = Some(Instant::now());

        // Trigger the cancellation token
        self.token.cancel();

        // Send the immediate signal on the watch channel
        let affected = self.running_tasks.load(Ordering::SeqCst);
        self.signal_tx
            .send(Some(ShutdownSignal::Immediate))
            .map_err(|_| CancellationError::NoSubscribers)?;

        Ok(CancelExecutionOutput {
            accepted: true,
            signal: ShutdownSignal::Immediate,
            affected_tasks: affected,
            was_already_cancelling: false,
        })
    }

    async fn await_shutdown(
        &self,
        input: ShutdownInput,
    ) -> Result<ShutdownOutput, CancellationError> {
        let start = Instant::now();
        let timeout = std::time::Duration::from_secs(input.timeout_secs);

        // Total tasks at start of await
        let total_at_start = self.running_tasks.load(Ordering::SeqCst);

        // Poll until all tasks complete or timeout
        let timed_out = loop {
            if self.running_tasks.load(Ordering::SeqCst) == 0 {
                break false;
            }
            if start.elapsed() >= timeout {
                break true;
            }
            tokio::time::sleep(std::time::Duration::from_millis(50)).await;
        };

        let shutdown_duration = start.elapsed().as_millis() as u64;
        let completed = self.completed_tasks.load(Ordering::SeqCst);
        let cancelled = self.cancelled_tasks.load(Ordering::SeqCst);
        let remaining = self.running_tasks.load(Ordering::SeqCst);

        // If timed out, return error (force-abort handled by caller via token)
        if timed_out {
            return Err(CancellationError::ShutdownTimeout {
                timeout_secs: input.timeout_secs,
                pending_tasks: remaining,
            });
        }

        // Run cleanup handlers for all registered task types
        let cleanup_success = self.run_cleanup().await;

        Ok(ShutdownOutput {
            execution_id: input.execution_id,
            signal_used: self
                .signal_rx
                .borrow()
                .clone()
                .unwrap_or(ShutdownSignal::Graceful),
            total_tasks: total_at_start + completed + cancelled,
            completed_tasks: completed,
            cancelled_tasks: cancelled,
            shutdown_duration_ms: shutdown_duration,
            forced: false,
            cleanup_success,
        })
    }

    #[tracing::instrument(skip_all)]
    fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::SeqCst) || self.token.is_cancelled()
    }

    #[tracing::instrument(skip_all)]
    fn current_signal(&self) -> Option<ShutdownSignal> {
        self.signal_rx.borrow().clone()
    }

    #[tracing::instrument(skip_all)]
    async fn status(&self) -> ShutdownStatusOutput {
        let elapsed = self.request_time.lock().await;
        ShutdownStatusOutput {
            is_cancelled: self.is_cancelled(),
            current_signal: self.current_signal(),
            running_tasks: self.running_tasks.load(Ordering::SeqCst),
            completed_tasks: self.completed_tasks.load(Ordering::SeqCst),
            cancelled_tasks: self.cancelled_tasks.load(Ordering::SeqCst),
            shutdown_complete: self.running_tasks.load(Ordering::SeqCst) == 0
                && self.is_cancelled(),
            elapsed_since_request_ms: elapsed.as_ref().map(|i| i.elapsed().as_millis() as u64),
        }
    }

    #[tracing::instrument(skip_all)]
    fn subscribe(&self) -> watch::Receiver<ShutdownSignal> {
        // Create a bridging channel that flattens Option<ShutdownSignal> -> ShutdownSignal
        let (tx, rx) = watch::channel(ShutdownSignal::Graceful);
        let mut inner = self.signal_tx.subscribe();
        tokio::spawn(async move {
            // Forward the initial value
            let initial = inner.borrow().clone();
            if let Some(signal) = initial {
                let _ = tx.send(signal);
            }
            // Forward subsequent updates
            while inner.changed().await.is_ok() {
                let signal = inner.borrow().clone();
                if let Some(s) = signal {
                    let _ = tx.send(s);
                }
            }
        });
        rx
    }

    #[tracing::instrument(skip_all)]
    fn cancellation_token(&self) -> CancellationToken {
        self.token.clone()
    }
}

// --- Internal helpers ---

impl CancellationManagerImpl {
    /// Register a task as running. Returns `false` if already cancelled.
    pub async fn register_task(&self, _input: &RegisterTaskInput) -> RegisterTaskOutput {
        let already_cancelled = self.is_cancelled();
        if !already_cancelled {
            self.running_tasks.fetch_add(1, Ordering::SeqCst);
        }
        RegisterTaskOutput {
            accepted: !already_cancelled,
            is_already_cancelled: already_cancelled,
        }
    }

    /// Notify that a task has completed (naturally or cancelled).
    pub async fn notify_task_complete(&self, input: &NotifyTaskCompleteInput) {
        self.running_tasks.fetch_sub(1, Ordering::SeqCst);
        if input.was_cancelled {
            self.cancelled_tasks.fetch_add(1, Ordering::SeqCst);
        } else {
            self.completed_tasks.fetch_add(1, Ordering::SeqCst);
        }
    }

    /// Run all registered cleanup handlers.
    #[tracing::instrument(skip_all)]
    async fn run_cleanup(&self) -> bool {
        let handlers = self.cleanup_handlers.read().await;
        let mut all_ok = true;
        for (task_type, handler) in handlers.iter() {
            if handler.cleanup(task_type).await.is_err() {
                all_ok = false;
            }
        }
        all_ok
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tracing::instrument(skip_all)]
    fn create_manager() -> CancellationManagerImpl {
        CancellationManagerImpl::new(5)
    }

    #[tokio::test]
    async fn test_initial_state_not_cancelled() {
        let mgr = create_manager();
        assert!(!mgr.is_cancelled());
        assert!(mgr.current_signal().is_none());
    }

    #[tokio::test]
    async fn test_graceful_shutdown_returns_accepted() {
        let mgr = create_manager();
        let input = CancelExecutionInput {
            execution_id: "exec-1".to_string(),
            reason: Some("user request".to_string()),
            source: "test".to_string(),
        };
        let output = mgr.request_graceful_shutdown(input).await.unwrap();
        assert!(output.accepted);
        assert_eq!(output.signal, ShutdownSignal::Graceful);
        assert!(mgr.is_cancelled());
        assert_eq!(mgr.current_signal(), Some(ShutdownSignal::Graceful));
    }

    #[tokio::test]
    async fn test_immediate_shutdown_returns_accepted() {
        let mgr = create_manager();
        let input = CancelExecutionInput {
            execution_id: "exec-1".to_string(),
            reason: Some("abort".to_string()),
            source: "test".to_string(),
        };
        let output = mgr.request_immediate_abort(input).await.unwrap();
        assert!(output.accepted);
        assert_eq!(output.signal, ShutdownSignal::Immediate);
        assert!(mgr.is_cancelled());
        assert_eq!(mgr.current_signal(), Some(ShutdownSignal::Immediate));
    }

    #[tokio::test]
    async fn test_duplicate_cancel_returns_error() {
        let mgr = create_manager();
        let input = CancelExecutionInput {
            execution_id: "exec-1".to_string(),
            reason: None,
            source: "test".to_string(),
        };
        mgr.request_graceful_shutdown(input).await.unwrap();

        let result = mgr
            .request_graceful_shutdown(CancelExecutionInput {
                execution_id: "exec-1".to_string(),
                reason: None,
                source: "test".to_string(),
            })
            .await;
        assert!(matches!(
            result,
            Err(CancellationError::AlreadyCancelling { .. })
        ));
    }

    #[tokio::test]
    async fn test_cancellation_token_triggers() {
        let mgr = create_manager();
        let token = mgr.cancellation_token();
        assert!(!token.is_cancelled());

        mgr.request_graceful_shutdown(CancelExecutionInput {
            execution_id: "exec-1".to_string(),
            reason: None,
            source: "test".to_string(),
        })
        .await
        .unwrap();

        assert!(token.is_cancelled());
    }

    #[tokio::test]
    async fn test_subscribe_receives_signal() {
        let mgr = create_manager();
        let mut rx = mgr.subscribe();

        mgr.request_immediate_abort(CancelExecutionInput {
            execution_id: "exec-1".to_string(),
            reason: None,
            source: "test".to_string(),
        })
        .await
        .unwrap();

        // Watch channel should have the signal
        let _ = rx.changed().await;
        assert_eq!(*rx.borrow(), ShutdownSignal::Immediate);
    }

    #[tokio::test]
    async fn test_status_before_cancellation() {
        let mgr = create_manager();
        let status = mgr.status().await;
        assert!(!status.is_cancelled);
        assert!(status.current_signal.is_none());
        assert_eq!(status.running_tasks, 0);
    }

    #[tokio::test]
    async fn test_status_after_cancellation() {
        let mgr = create_manager();
        mgr.request_graceful_shutdown(CancelExecutionInput {
            execution_id: "exec-1".to_string(),
            reason: None,
            source: "test".to_string(),
        })
        .await
        .unwrap();

        let status = mgr.status().await;
        assert!(status.is_cancelled);
        assert_eq!(status.current_signal, Some(ShutdownSignal::Graceful));
    }

    #[tokio::test]
    async fn test_task_registration_denied_when_cancelled() {
        let mgr = create_manager();
        mgr.request_graceful_shutdown(CancelExecutionInput {
            execution_id: "exec-1".to_string(),
            reason: None,
            source: "test".to_string(),
        })
        .await
        .unwrap();

        let result = mgr
            .register_task(&RegisterTaskInput {
                execution_id: "exec-1".to_string(),
                task_id: "task-1".to_string(),
                description: None,
            })
            .await;

        assert!(!result.accepted);
        assert!(result.is_already_cancelled);
    }

    #[tokio::test]
    async fn test_task_lifecycle_tracking() {
        let mgr = create_manager();

        // Register 2 tasks
        let reg1 = mgr
            .register_task(&RegisterTaskInput {
                execution_id: "exec-1".to_string(),
                task_id: "task-1".to_string(),
                description: None,
            })
            .await;
        assert!(reg1.accepted);

        let reg2 = mgr
            .register_task(&RegisterTaskInput {
                execution_id: "exec-1".to_string(),
                task_id: "task-2".to_string(),
                description: None,
            })
            .await;
        assert!(reg2.accepted);

        assert_eq!(mgr.status().await.running_tasks, 2);

        // Complete one task naturally
        mgr.notify_task_complete(&NotifyTaskCompleteInput {
            execution_id: "exec-1".to_string(),
            task_id: "task-1".to_string(),
            was_cancelled: false,
            running_duration_ms: 100,
            cleanup_success: true,
        })
        .await;

        assert_eq!(mgr.status().await.running_tasks, 1);
        assert_eq!(mgr.status().await.completed_tasks, 1);

        // Cancel the other task
        mgr.notify_task_complete(&NotifyTaskCompleteInput {
            execution_id: "exec-1".to_string(),
            task_id: "task-2".to_string(),
            was_cancelled: true,
            running_duration_ms: 50,
            cleanup_success: true,
        })
        .await;

        assert_eq!(mgr.status().await.running_tasks, 0);
        assert_eq!(mgr.status().await.cancelled_tasks, 1);
    }

    #[tokio::test]
    async fn test_await_shutdown_with_no_tasks() {
        let mgr = create_manager();
        mgr.request_graceful_shutdown(CancelExecutionInput {
            execution_id: "exec-1".to_string(),
            reason: None,
            source: "test".to_string(),
        })
        .await
        .unwrap();

        let output = mgr
            .await_shutdown(ShutdownInput {
                execution_id: "exec-1".to_string(),
                timeout_secs: 1,
                force_abort_on_timeout: false,
            })
            .await
            .unwrap();

        assert_eq!(output.signal_used, ShutdownSignal::Graceful);
        assert!(!output.forced);
        assert!(output.cleanup_success);
    }

    #[tokio::test]
    async fn test_await_shutdown_with_running_tasks_times_out() {
        let mgr = create_manager();

        // Register a fake task that never completes
        mgr.register_task(&RegisterTaskInput {
            execution_id: "exec-1".to_string(),
            task_id: "hanging-task".to_string(),
            description: None,
        })
        .await;

        mgr.request_graceful_shutdown(CancelExecutionInput {
            execution_id: "exec-1".to_string(),
            reason: None,
            source: "test".to_string(),
        })
        .await
        .unwrap();

        let result = mgr
            .await_shutdown(ShutdownInput {
                execution_id: "exec-1".to_string(),
                timeout_secs: 1,
                force_abort_on_timeout: false,
            })
            .await;

        assert!(matches!(
            result,
            Err(CancellationError::ShutdownTimeout { .. })
        ));

        // Clean up the hanging task
        mgr.notify_task_complete(&NotifyTaskCompleteInput {
            execution_id: "exec-1".to_string(),
            task_id: "hanging-task".to_string(),
            was_cancelled: true,
            running_duration_ms: 1000,
            cleanup_success: false,
        })
        .await;
    }

    #[tokio::test]
    async fn test_child_token_propagates_from_parent() {
        let parent_token = CancellationToken::new();
        let mgr = CancellationManagerImpl::child_of(parent_token.clone(), 30);

        // Initial state: child not cancelled
        assert!(!mgr.is_cancelled());

        // Cancel the parent
        parent_token.cancel();

        // Child should be cancelled too
        assert!(mgr.is_cancelled());
    }

    #[tokio::test]
    async fn test_cleanup_handler_invoked_on_shutdown() {
        let mgr = create_manager();
        let cleaned = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let cleaned_clone = cleaned.clone();

        struct TestCleanup {
            flag: std::sync::Arc<std::sync::atomic::AtomicBool>,
        }

        #[async_trait]
        impl CleanupHandler for TestCleanup {
            #[tracing::instrument(skip_all)]
            async fn cleanup(&self, _task_id: &str) -> Result<(), CancellationError> {
                self.flag.store(true, std::sync::atomic::Ordering::SeqCst);
                Ok(())
            }

            #[tracing::instrument(skip_all)]
            async fn release(&self, _task_id: &str) -> Result<(), CancellationError> {
                Ok(())
            }
        }

        mgr.register_cleanup_handler(
            "test-task",
            Box::new(TestCleanup {
                flag: cleaned_clone,
            }),
        )
        .await;

        mgr.request_graceful_shutdown(CancelExecutionInput {
            execution_id: "exec-1".to_string(),
            reason: None,
            source: "test".to_string(),
        })
        .await
        .unwrap();

        mgr.await_shutdown(ShutdownInput {
            execution_id: "exec-1".to_string(),
            timeout_secs: 1,
            force_abort_on_timeout: false,
        })
        .await
        .unwrap();

        assert!(cleaned.load(std::sync::atomic::Ordering::SeqCst));
    }
}
