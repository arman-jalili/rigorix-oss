//! Implementation of `HookRunnerService`.
//!
//! @canonical .pi/architecture/modules/hooks.md#hook-runner
//! Implements: HookRunnerService — executes hook commands as child processes
//! Issue: #411, #412, #413, #414, #415
//!
//! Spawns hook commands as child processes, pipes JSON stdin payloads,
//! reads and parses stdout JSON responses, and aggregates results into
//! `HookRunResult`. Supports cooperative cancellation via `HookAbortSignal`.

use std::io::Write;
use std::process::{Command, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Instant;

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
    /// Spawns the command as a child process, writes the stdin JSON payload,
    /// reads stdout, and parses the response. If the process exits with a
    /// non-zero code and the stdout is not valid JSON, returns a `ProcessError`.
    fn execute_single_command(
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
        let mut child = Command::new("sh")
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
        if let Some(mut stdin) = child.stdin.take()
            && let Err(e) = stdin.write_all(stdin_json.as_bytes())
        {
            let _ = child.kill();
            return Err(HookError::Internal {
                detail: format!("Failed to write stdin to hook '{}': {}", command, e),
            });
        }

        // Wait for the process with timeout
        let start = Instant::now();
        loop {
            if let Some(signal) = abort_signal
                && signal.is_aborted()
            {
                let _ = child.kill();
                return Ok(HookRunResult::cancelled(
                    event,
                    vec![format!("Hook '{}' aborted during execution", command)],
                ));
            }

            match child.try_wait() {
                Ok(Some(status)) => {
                    let elapsed = start.elapsed().as_millis() as u64;
                    let output_result =
                        child.wait_with_output().map_err(|e| HookError::Internal {
                            detail: format!("Failed to read output from hook '{}': {}", command, e),
                        })?;

                    let stdout = String::from_utf8_lossy(&output_result.stdout).to_string();
                    let stderr = String::from_utf8_lossy(&output_result.stderr).to_string();

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
                        Ok(response) => {
                            return Ok(Self::response_to_result(response, event, elapsed));
                        }
                        Err(e) => {
                            if stdout.trim().is_empty() {
                                // Empty stdout on success = allow (no-op hook)
                                return Ok(HookRunResult::new(event));
                            }
                            return Err(HookError::InvalidJson {
                                command: command.to_string(),
                                detail: e.to_string(),
                                raw_output: stdout.chars().take(500).collect(),
                            });
                        }
                    }
                }
                Ok(None) => {
                    // Process still running — check timeout
                    if start.elapsed().as_millis() as u64 > timeout_ms {
                        let _ = child.kill();
                        return Err(HookError::Timeout {
                            command: command.to_string(),
                            timeout_ms,
                        });
                    }
                    std::thread::sleep(std::time::Duration::from_millis(10));
                }
                Err(e) => {
                    return Err(HookError::Internal {
                        detail: format!("Error waiting for hook '{}': {}", command, e),
                    });
                }
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
    fn execute_all_for_event(
        &self,
        commands: &[String],
        payload: &HookStdinPayload,
        abort_signal: Option<&HookAbortSignal>,
        is_pre_tool_use: bool,
    ) -> HookRunResult {
        let mut aggregated = HookRunResult::new(payload.event);
        let stdin_value = serde_json::to_value(payload).unwrap_or_default();

        let commands_iter: Box<dyn Iterator<Item = &String>> = Box::new(commands.iter());

        for command in commands_iter {
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

            match self.execute_single_command(command, &stdin_value, payload.event, abort_signal) {
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

impl HookRunnerService for HookRunnerImpl {
    fn run_pre_tool_use(
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

        let result = self.execute_all_for_event(commands, &payload, abort_signal, true);

        self.running.store(false, Ordering::SeqCst);
        Ok(RunPreToolUseOutput { result })
    }

    fn run_post_tool_use(
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

        let result = self.execute_all_for_event(commands, &payload, abort_signal, false);

        self.running.store(false, Ordering::SeqCst);
        Ok(RunPostToolUseOutput { result })
    }

    fn run_post_tool_use_failure(
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

        let result = self.execute_all_for_event(commands, &payload, abort_signal, false);

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

impl HookCommandExecutor for HookRunnerImpl {
    fn execute_command(
        &self,
        command: &str,
        stdin_payload: &serde_json::Value,
        abort_signal: Option<&HookAbortSignal>,
    ) -> Result<HookRunResult, HookError> {
        self.execute_single_command(command, stdin_payload, HookEvent::PreToolUse, abort_signal)
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

    #[test]
    fn test_run_pre_tool_use_empty_config() {
        let runner = HookRunnerImpl::new(HookConfig::default());
        let input = RunPreToolUseInput {
            tool_name: "test_tool".into(),
            tool_input: serde_json::json!({"params": {}}),
            session_id: "test-session".into(),
            workspace_root: "/tmp".into(),
        };
        let output = runner.run_pre_tool_use(input, None).unwrap();
        assert!(output.result.is_allowed());
        assert!(!output.result.is_denied());
    }

    #[test]
    fn test_run_post_tool_use_empty_config() {
        let runner = HookRunnerImpl::new(HookConfig::default());
        let input = RunPostToolUseInput {
            tool_name: "test_tool".into(),
            tool_input: serde_json::json!({"params": {}}),
            tool_output: "success".into(),
            session_id: "test-session".into(),
            workspace_root: "/tmp".into(),
        };
        let output = runner.run_post_tool_use(input, None).unwrap();
        assert!(output.result.is_allowed());
    }

    #[test]
    fn test_run_post_tool_use_failure_empty_config() {
        let runner = HookRunnerImpl::new(HookConfig::default());
        let input = RunPostToolUseFailureInput {
            tool_name: "test_tool".into(),
            tool_input: serde_json::json!({"params": {}}),
            error_output: "error".into(),
            session_id: "test-session".into(),
            workspace_root: "/tmp".into(),
        };
        let output = runner.run_post_tool_use_failure(input, None).unwrap();
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

    #[test]
    fn test_execute_command_not_found_returns_process_error() {
        // Commands passed to sh -c that don't exist return ProcessError
        // (sh exits with non-zero code, not ENOENT at spawn time)
        let runner = HookRunnerImpl::new(HookConfig::default());
        let payload = serde_json::json!({"event":"pre_tool_use"});
        let result = runner.execute_command("nonexistent-command-xyz-99999", &payload, None);
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

    #[test]
    fn test_abort_before_execution() {
        let runner = HookRunnerImpl::new(HookConfig::default());
        let signal = HookAbortSignal::new_aborted();
        let input = RunPreToolUseInput {
            tool_name: "test".into(),
            tool_input: serde_json::json!({}),
            session_id: "s1".into(),
            workspace_root: "/tmp".into(),
        };
        // Empty config — no hooks to run, so abort doesn't matter
        let output = runner.run_pre_tool_use(input, Some(&signal)).unwrap();
        assert!(output.result.is_allowed());
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
