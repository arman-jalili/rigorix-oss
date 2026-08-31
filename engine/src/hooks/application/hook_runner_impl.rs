//! Implementation of `HookRunnerService`.
//!
//! @canonical .pi/architecture/modules/hooks.md#hook-runner
//! Implements: HookRunnerService — executes hook commands as child processes
//! Issue: #411, #412, #413, #414, #415
//!
//! Spawns hook commands as child processes, pipes JSON stdin payloads,
//! reads and parses stdout JSON responses, and aggregates results into
//! `HookRunResult`. Supports cooperative cancellation via `HookAbortSignal`.

use std::process::Stdio;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use async_trait::async_trait;
use tokio::io::{AsyncReadExt, AsyncWriteExt};

use crate::hooks::domain::abort::HookAbortSignal;
use crate::hooks::domain::config::HookConfig;
use crate::hooks::domain::error::HookError;
use crate::hooks::domain::event::HookEvent;
use crate::hooks::domain::protocol::{HookDecision, HookStdinPayload, HookStdoutResponse};
use crate::hooks::domain::result::HookRunResult;

use super::dto::{
    HookRunnerStatus, RunPostToolUseFailureInput, RunPostToolUseFailureOutput, RunPostToolUseInput,
    RunPostToolUseOutput, RunPreToolUseInput, RunPreToolUseOutput,
};
use super::service::{HookCommandExecutor, HookRunnerService};

/// Minimum timeout in seconds.
const MIN_TIMEOUT_SECS: u64 = 1;

/// Concrete implementation of `HookRunnerService`.
///
/// Executes hook commands by spawning child processes, piping JSON to stdin,
/// and parsing the JSON response from stdout. Results are aggregated per the
/// documented merge rules (first deny wins, last permission_override wins, etc.).
pub struct HookRunnerImpl {
    /// Hook configuration (command lists per event).
    config: HookConfig,

    /// Whether the runner is actively processing hooks.
    running: Arc<AtomicBool>,
}

impl HookRunnerImpl {
    /// Create a new HookRunner with the given configuration.
    pub fn new(config: HookConfig) -> Self {
        Self {
            config,
            running: Arc::new(AtomicBool::new(false)),
        }
    }

    /// Execute a single hook command and return its result.
    ///
    /// Spawns the command as a child process (GAP-A-12: `tokio::process`,
    /// never blocking the async runtime), writes the stdin JSON payload,
    /// reads stdout/stderr concurrently via a tokio drain task, and parses
    /// the response. Timeouts and abort checks are cooperative (async).
    async fn execute_single_command(
        &self,
        command: &str,
        stdin_payload: &serde_json::Value,
        event: HookEvent,
        abort_signal: Option<&HookAbortSignal>,
    ) -> Result<HookRunResult, HookError> {
        // Check abort signal before spawning
        if let Some(signal) = abort_signal
            && signal.is_aborted()
        {
            return Ok(HookRunResult::cancelled(
                event,
                vec![format!("Hook '{}' aborted before execution", command)],
            ));
        }

        let timeout_ms = (self.config.timeout_secs.max(MIN_TIMEOUT_SECS)) * 1000;

        // Spawn the child process
        let mut child = tokio::process::Command::new("sh")
            .arg("-c")
            .arg(command)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .map_err(|e| {
                if e.kind() == std::io::ErrorKind::NotFound {
                    HookError::CommandNotFound {
                        command: command.to_string(),
                    }
                } else {
                    HookError::Internal {
                        detail: format!("Failed to spawn hook '{}': {}", command, e),
                    }
                }
            })?;

        // Write stdin payload
        let stdin_json = serde_json::to_string(stdin_payload).unwrap_or_default();
        if let Some(mut stdin) = child.stdin.take() {
            if let Err(e) = stdin.write_all(stdin_json.as_bytes()).await {
                let _ = child.kill().await;
                return Err(HookError::Internal {
                    detail: format!("Failed to write stdin to hook '{}': {}", command, e),
                });
            }
            let _ = stdin.flush().await;
        }

        // Drain stdout/stderr concurrently in a tokio task while we wait.
        // GAP-A-04 + GAP-A-12: without concurrent draining, a hook writing
        // more than the ~64KB pipe buffer blocks forever on write(2); the
        // async reactor reads both pipes while the wait loop polls.
        let mut out_pipe = child.stdout.take();
        let mut err_pipe = child.stderr.take();
        let drain = tokio::spawn(async move {
            let mut stdout_buf = Vec::new();
            let mut stderr_buf = Vec::new();
            if let Some(pipe) = out_pipe.as_mut() {
                let _ = pipe.read_to_end(&mut stdout_buf).await;
            }
            if let Some(pipe) = err_pipe.as_mut() {
                let _ = pipe.read_to_end(&mut stderr_buf).await;
            }
            (stdout_buf, stderr_buf)
        });

        // Wait for the process with timeout (async, cooperative)
        let start = Instant::now();
        let status = loop {
            if let Some(signal) = abort_signal
                && signal.is_aborted()
            {
                let _ = child.kill().await;
                let _ = child.wait().await;
                let _ = drain.await;
                return Ok(HookRunResult::cancelled(
                    event,
                    vec![format!("Hook '{}' aborted during execution", command)],
                ));
            }

            match child.try_wait() {
                Ok(Some(status)) => break status,
                Ok(None) => {
                    // Process still running — check timeout
                    if start.elapsed().as_millis() as u64 > timeout_ms {
                        let _ = child.kill().await;
                        let _ = child.wait().await;
                        let _ = drain.await;
                        return Err(HookError::Timeout {
                            command: command.to_string(),
                            timeout_ms,
                        });
                    }
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(e) => {
                    let _ = child.kill().await;
                    let _ = child.wait().await;
                    let _ = drain.await;
                    return Err(HookError::Internal {
                        detail: format!("Error waiting for hook '{}': {}", command, e),
                    });
                }
            }
        };

        // Child has exited (reaped by try_wait above). The drain task sees
        // EOF once the child's write ends close, so awaiting it is prompt.
        let (stdout_bytes, stderr_bytes) = drain.await.unwrap_or_default();
        let elapsed = start.elapsed().as_millis() as u64;
        let stdout = String::from_utf8_lossy(&stdout_bytes).to_string();
        let stderr = String::from_utf8_lossy(&stderr_bytes).to_string();

        if !status.success() {
            // Try to parse JSON from stdout even on non-zero exit
            if let Ok(response) = serde_json::from_str::<HookStdoutResponse>(&stdout) {
                return Ok(Self::response_to_result(response, event, elapsed));
            }
            return Err(HookError::ProcessError {
                command: command.to_string(),
                exit_code: status.code().unwrap_or(-1),
                stderr,
            });
        }

        // Successful exit — parse stdout as JSON
        match serde_json::from_str::<HookStdoutResponse>(&stdout) {
            Ok(response) => Ok(Self::response_to_result(response, event, elapsed)),
            Err(e) => {
                if stdout.trim().is_empty() {
                    // Empty stdout on success = allow (no-op hook)
                    return Ok(HookRunResult::new(event));
                }
                Err(HookError::InvalidJson {
                    command: command.to_string(),
                    detail: e.to_string(),
                    raw_output: stdout.chars().take(500).collect(),
                })
            }
        }
    }

    /// Convert a HookStdoutResponse into a HookRunResult.
    fn response_to_result(
        response: HookStdoutResponse,
        event: HookEvent,
        _duration_ms: u64,
    ) -> HookRunResult {
        let mut result = HookRunResult::new(event);

        match response.decision {
            HookDecision::Deny => {
                result.denied = true;
                if let Some(reason) = response.reason {
                    result.messages.push(reason);
                }
            }
            HookDecision::AllowWithOverride => {
                result.permission_override = response.permission_override;
                result.permission_reason = response.reason;
            }
            HookDecision::Modify => {
                result.updated_input = response.updated_input;
            }
            HookDecision::Allow => {}
        }

        result.messages.extend(response.messages);
        result
    }

    /// Execute all commands for a given event and aggregate the results.
    async fn execute_all_for_event(
        &self,
        commands: &[String],
        payload: &HookStdinPayload,
        abort_signal: Option<&HookAbortSignal>,
        is_pre_tool_use: bool,
    ) -> HookRunResult {
        let mut aggregated = HookRunResult::new(payload.event);
        let stdin_value = serde_json::to_value(payload).unwrap_or_default();

        for command in commands {
            // Check abort before each hook
            if let Some(signal) = abort_signal
                && signal.is_aborted()
            {
                aggregated.cancelled = true;
                aggregated
                    .messages
                    .push(format!("Hook execution aborted before '{}'", command));
                break;
            }

            match self
                .execute_single_command(command, &stdin_value, payload.event, abort_signal)
                .await
            {
                Ok(hook_result) => {
                    aggregated.merge(&hook_result);
                    // If denied or cancelled, stop executing more hooks
                    if (hook_result.is_denied() || hook_result.is_cancelled()) && is_pre_tool_use {
                        break;
                    }
                }
                Err(e) => {
                    aggregated.failed = true;
                    aggregated
                        .messages
                        .push(format!("Hook '{}' failed: {}", command, e));
                    // For PreToolUse, a failed hook blocks the tool
                    if is_pre_tool_use && !e.is_recoverable() {
                        aggregated.denied = true;
                        break;
                    }
                }
            }
        }

        aggregated
    }
}

#[async_trait]
impl HookRunnerService for HookRunnerImpl {
    async fn run_pre_tool_use(
        &self,
        input: RunPreToolUseInput,
        abort_signal: Option<&HookAbortSignal>,
    ) -> Result<RunPreToolUseOutput, HookError> {
        self.running.store(true, Ordering::SeqCst);

        let commands = self.config.commands_for(HookEvent::PreToolUse);
        if commands.is_empty() {
            self.running.store(false, Ordering::SeqCst);
            return Ok(RunPreToolUseOutput {
                result: HookRunResult::new(HookEvent::PreToolUse),
            });
        }

        let payload = HookStdinPayload::new(
            HookEvent::PreToolUse,
            &input.tool_name,
            input.tool_input.clone(),
            &input.session_id,
            &input.workspace_root,
        );

        let result = self
            .execute_all_for_event(commands, &payload, abort_signal, true)
            .await;

        self.running.store(false, Ordering::SeqCst);
        Ok(RunPreToolUseOutput { result })
    }

    async fn run_post_tool_use(
        &self,
        input: RunPostToolUseInput,
        abort_signal: Option<&HookAbortSignal>,
    ) -> Result<RunPostToolUseOutput, HookError> {
        self.running.store(true, Ordering::SeqCst);

        let commands = self.config.commands_for(HookEvent::PostToolUse);
        if commands.is_empty() {
            self.running.store(false, Ordering::SeqCst);
            return Ok(RunPostToolUseOutput {
                result: HookRunResult::new(HookEvent::PostToolUse),
            });
        }

        let payload = HookStdinPayload::new(
            HookEvent::PostToolUse,
            &input.tool_name,
            input.tool_input.clone(),
            &input.session_id,
            &input.workspace_root,
        );

        let result = self
            .execute_all_for_event(commands, &payload, abort_signal, false)
            .await;

        self.running.store(false, Ordering::SeqCst);
        Ok(RunPostToolUseOutput { result })
    }

    async fn run_post_tool_use_failure(
        &self,
        input: RunPostToolUseFailureInput,
        abort_signal: Option<&HookAbortSignal>,
    ) -> Result<RunPostToolUseFailureOutput, HookError> {
        self.running.store(true, Ordering::SeqCst);

        let commands = self.config.commands_for(HookEvent::PostToolUseFailure);
        if commands.is_empty() {
            self.running.store(false, Ordering::SeqCst);
            return Ok(RunPostToolUseFailureOutput {
                result: HookRunResult::new(HookEvent::PostToolUseFailure),
            });
        }

        let payload = HookStdinPayload::new(
            HookEvent::PostToolUseFailure,
            &input.tool_name,
            input.tool_input.clone(),
            &input.session_id,
            &input.workspace_root,
        );

        let result = self
            .execute_all_for_event(commands, &payload, abort_signal, false)
            .await;

        self.running.store(false, Ordering::SeqCst);
        Ok(RunPostToolUseFailureOutput { result })
    }

    fn status(&self) -> HookRunnerStatus {
        HookRunnerStatus {
            pre_tool_use_count: self.config.pre_tool_use.len(),
            post_tool_use_count: self.config.post_tool_use.len(),
            post_tool_use_failure_count: self.config.post_tool_use_failure.len(),
            total_hook_count: self.config.total_command_count(),
            is_running: self.running.load(Ordering::SeqCst),
            timeout_secs: self.config.timeout_secs,
        }
    }

    fn reconfigure(&self, config: HookConfig) -> Result<(), HookError> {
        // This would need interior mutability in a real implementation
        // For now, this is a placeholder
        let _ = config;
        Ok(())
    }

    fn create_abort_signal(&self) -> HookAbortSignal {
        HookAbortSignal::new()
    }
}

#[async_trait]
impl HookCommandExecutor for HookRunnerImpl {
    async fn execute_command(
        &self,
        command: &str,
        stdin_payload: &serde_json::Value,
        abort_signal: Option<&HookAbortSignal>,
    ) -> Result<HookRunResult, HookError> {
        self.execute_single_command(command, stdin_payload, HookEvent::PreToolUse, abort_signal)
            .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hooks::domain::config::HookConfig;
    use crate::hooks::domain::protocol::HookPermissionOverride;

    #[test]
    fn test_new_runner_not_running() {
        let runner = HookRunnerImpl::new(HookConfig::default());
        assert!(!runner.status().is_running);
        assert_eq!(runner.status().total_hook_count, 0);
    }

    #[tokio::test]
    async fn test_run_pre_tool_use_empty_config() {
        let runner = HookRunnerImpl::new(HookConfig::default());
        let input = RunPreToolUseInput {
            tool_name: "test_tool".into(),
            tool_input: serde_json::json!({"params": {}}),
            session_id: "test-session".into(),
            workspace_root: "/tmp".into(),
        };
        let output = runner.run_pre_tool_use(input, None).await.unwrap();
        assert!(output.result.is_allowed());
        assert!(!output.result.is_denied());
    }

    #[tokio::test]
    async fn test_run_post_tool_use_empty_config() {
        let runner = HookRunnerImpl::new(HookConfig::default());
        let input = RunPostToolUseInput {
            tool_name: "test_tool".into(),
            tool_input: serde_json::json!({"params": {}}),
            tool_output: "success".into(),
            session_id: "test-session".into(),
            workspace_root: "/tmp".into(),
        };
        let output = runner.run_post_tool_use(input, None).await.unwrap();
        assert!(output.result.is_allowed());
    }

    #[tokio::test]
    async fn test_run_post_tool_use_failure_empty_config() {
        let runner = HookRunnerImpl::new(HookConfig::default());
        let input = RunPostToolUseFailureInput {
            tool_name: "test_tool".into(),
            tool_input: serde_json::json!({"params": {}}),
            error_output: "error".into(),
            session_id: "test-session".into(),
            workspace_root: "/tmp".into(),
        };
        let output = runner.run_post_tool_use_failure(input, None).await.unwrap();
        assert!(output.result.is_allowed());
    }

    #[test]
    fn test_status_counts() {
        let config = HookConfig {
            pre_tool_use: vec!["hook1".into(), "hook2".into()],
            post_tool_use: vec!["hook3".into()],
            post_tool_use_failure: vec![],
            timeout_secs: 30,
            sequential_pre_tool_use: false,
        };
        let runner = HookRunnerImpl::new(config);
        let status = runner.status();
        assert_eq!(status.pre_tool_use_count, 2);
        assert_eq!(status.post_tool_use_count, 1);
        assert_eq!(status.post_tool_use_failure_count, 0);
        assert_eq!(status.total_hook_count, 3);
        assert_eq!(status.timeout_secs, 30);
    }

    #[test]
    fn test_create_abort_signal() {
        let runner = HookRunnerImpl::new(HookConfig::default());
        let signal = runner.create_abort_signal();
        assert!(!signal.is_aborted());
        signal.abort();
        assert!(signal.is_aborted());
    }

    #[test]
    fn test_reconfigure() {
        let runner = HookRunnerImpl::new(HookConfig::default());
        let new_config = HookConfig {
            pre_tool_use: vec!["new-hook".into()],
            ..Default::default()
        };
        // reconfigure is a no-op for now
        assert!(runner.reconfigure(new_config).is_ok());
    }

    #[tokio::test]
    async fn test_execute_command_not_found_returns_process_error() {
        // Commands passed to sh -c that don't exist return ProcessError
        // (sh exits with non-zero code, not ENOENT at spawn time)
        let runner = HookRunnerImpl::new(HookConfig::default());
        let payload = serde_json::json!({"event":"pre_tool_use"});
        let result = runner
            .execute_command("nonexistent-command-xyz-99999", &payload, None)
            .await;
        match result {
            Err(HookError::ProcessError {
                command, exit_code, ..
            }) => {
                assert!(command.contains("nonexistent-command-xyz-99999"));
                assert_ne!(exit_code, 0);
            }
            other => {
                // On some systems this might be an Internal error (e.g. macOS sandbox)
                // Allow it as long as it's an error
                assert!(other.is_err(), "Expected an error, got {:?}", other);
            }
        }
    }

    #[tokio::test]
    async fn test_abort_before_execution() {
        let runner = HookRunnerImpl::new(HookConfig::default());
        let signal = HookAbortSignal::new_aborted();
        let input = RunPreToolUseInput {
            tool_name: "test".into(),
            tool_input: serde_json::json!({}),
            session_id: "s1".into(),
            workspace_root: "/tmp".into(),
        };
        // Empty config — no hooks to run, so abort doesn't matter
        let output = runner.run_pre_tool_use(input, Some(&signal)).await.unwrap();
        assert!(output.result.is_allowed());
    }

    /// GAP-A-04 regression: a hook emitting more than the ~64KB pipe buffer
    /// must complete promptly (output drained on reader threads) instead of
    /// blocking forever and only escaping via the wall-clock timeout.
    ///
    /// `execute_command` surfaces the raw error: before the fix this was
    /// `Timeout` (after the 1s window); with the fix the child exits as soon
    /// as the pipe drains and the non-JSON output yields `InvalidJson`.
    #[tokio::test]
    async fn test_large_hook_output_does_not_deadlock() {
        let runner = HookRunnerImpl::new(HookConfig::default());
        let payload = serde_json::json!({});
        let start = std::time::Instant::now();
        let result = runner
            .execute_command("yes x | head -c 200000", &payload, None)
            .await;
        let elapsed_ms = start.elapsed().as_millis();
        assert!(
            matches!(result, Err(HookError::InvalidJson { .. })),
            "expected InvalidJson (output drained), got {:?}",
            result
        );
        assert!(
            elapsed_ms < 900,
            "must not wait out the 1s timeout, took {}ms",
            elapsed_ms
        );
    }

    #[tokio::test]
    async fn test_abort_kills_long_running_hook() {
        // Abort must kill + reap the child and drain the pipes so no
        // zombie/leaked pipe is left behind.
        let config = HookConfig {
            pre_tool_use: vec!["sleep 30".into()],
            timeout_secs: 30,
            ..Default::default()
        };
        let runner = HookRunnerImpl::new(config);
        let signal = runner.create_abort_signal();
        let input = RunPreToolUseInput {
            tool_name: "test_tool".into(),
            tool_input: serde_json::json!({}),
            session_id: "s".into(),
            workspace_root: "/tmp".into(),
        };
        let thread_signal = signal.clone();
        let handle =
            tokio::spawn(async move { runner.run_pre_tool_use(input, Some(&thread_signal)).await });
        // Give the hook a moment to spawn, then abort.
        tokio::time::sleep(Duration::from_millis(200)).await;
        signal.abort();
        let output = handle
            .await
            .expect("runner task must not hang")
            .expect("runner must return Ok");
        assert!(
            output.result.is_cancelled(),
            "aborted hook must report cancelled"
        );
    }

    // -----------------------------------------------------------------------
    // Response parsing tests
    // -----------------------------------------------------------------------

    #[test]
    fn test_response_to_result_allow() {
        let response = HookStdoutResponse::allow(vec!["OK".into()]);
        let result = HookRunnerImpl::response_to_result(response, HookEvent::PreToolUse, 100);
        assert!(result.is_allowed());
        assert_eq!(result.messages, vec!["OK"]);
    }

    #[test]
    fn test_response_to_result_deny() {
        let response = HookStdoutResponse::deny("Blocked by policy");
        let result = HookRunnerImpl::response_to_result(response, HookEvent::PreToolUse, 100);
        assert!(result.is_denied());
        assert!(
            result
                .messages
                .iter()
                .any(|m| m.contains("Blocked by policy"))
        );
    }

    #[test]
    fn test_response_to_result_allow_with_override() {
        let response = HookStdoutResponse::allow_with_override(
            HookPermissionOverride::RequireConfirmation,
            "Elevated risk",
            vec!["Caution".into()],
        );
        let result = HookRunnerImpl::response_to_result(response, HookEvent::PreToolUse, 100);
        assert!(result.is_allowed());
        assert_eq!(
            result.permission_override,
            Some(HookPermissionOverride::RequireConfirmation)
        );
    }

    #[test]
    fn test_response_to_result_modify() {
        let updated = serde_json::json!({"params": {"cmd": "safe"}});
        let response = HookStdoutResponse::modify(
            updated.clone(),
            "Modified for safety",
            vec!["Input sanitized".into()],
        );
        let result = HookRunnerImpl::response_to_result(response, HookEvent::PreToolUse, 100);
        assert_eq!(result.updated_input, Some(updated));
    }
}
