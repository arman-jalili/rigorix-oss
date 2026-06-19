# Runbook: hooks Module

<!--
Canonical Reference: .pi/architecture/modules/hooks.md
Last Updated: 2026-06-19
-->

## Overview

The `hooks` module provides external script-based interception points around every
tool execution. Hooks run as shell commands receiving JSON payloads on stdin and
returning structured JSON decisions. They enable deployment-specific policies,
CI/CD integration, audit enrichment, and custom pre/post-flight validation.

## Startup Sequence

### Dependencies

| Dependency | Required | Description |
|------------|----------|-------------|
| std::process::Command | Yes | Child process spawning for hook commands |
| serde / serde_json | Yes | JSON protocol serialization/deserialization |
| std::sync::atomic | Yes | HookAbortSignal for cooperative cancellation |
| HookConfig | Yes | Configuration loaded from `.rigorix/hooks.toml` |

### Initialization

1. Load `HookConfig` from configuration source (`.rigorix/hooks.toml`)
2. Create a `HookRunner` (struct) with the config
3. Pass the runner to the Execution Engine pipeline

```rust
use rigorix::hooks::domain::HookConfig;
use rigorix::hooks::application::HookRunner;

// From configuration
let config: HookConfig = load_hook_config()?;
let runner = HookRunner::new(config);
```

### Quick Start

```rust
use rigorix::hooks::domain::*;
use rigorix::hooks::application::*;

// Create a runner with PreToolUse hooks
let config = HookConfig {
    pre_tool_use: vec![
        "rigorix-hook-validate-path".into(),
        "rigorix-hook-ci-guard".into(),
    ],
    ..Default::default()
};
let runner = HookRunner::new(config);

// Run hooks before a tool execution
let result = runner.run_pre_tool_use(
    "write_file",
    &tool_input,
    None,           // abort_signal
    None,           // progress_reporter
);

if result.is_denied() {
    // Tool blocked — report reason to LLM
} else if let Some(modified) = result.modified_input() {
    // Use modified input for tool execution
}
```

## Graceful Shutdown

### Hook Execution Interruption

1. Create a `HookAbortSignal` and share it with running hooks
2. Call `signal.abort()` to trigger cooperative cancellation
3. Running hook processes should check the signal and terminate
4. After timeout, any remaining processes are killed

```rust
let abort = HookAbortSignal::new();

// Spawn hook runner with abort signal
let signal = abort.clone();
let handle = std::thread::spawn(move || {
    runner.run_pre_tool_use("write_file", &input, Some(&signal), None);
});

// If cancellation needed:
abort.abort();
handle.join().unwrap();
```

### Timeout Handling

- Default hook timeout: 30 seconds (configurable via `HookConfig.timeout_secs`)
- On timeout: hook process is killed
- PreToolUse timeout → tool is **blocked** (safety-first)
- PostToolUse/PostToolUseFailure timeout → tool result returned without hook feedback

## Common Failure Modes and Recovery

| Failure Mode | Symptom | Cause | Recovery |
|-------------|---------|-------|----------|
| Hook command not found | `HookError::CommandNotFound` | Command not in PATH | Hook is skipped; execution continues with warning |
| Hook execution timeout | `HookError::Timeout` | Hook script hangs or is slow | Increase `timeout_secs` in config; debug hook script |
| Invalid JSON response | `HookError::InvalidJson` | Hook stdout malformed | Validate hook script output; check for stderr leakage |
| Hook process error | `HookError::ProcessError` | Non-zero exit + no valid JSON | Check hook stderr; fix script |
| Aborted execution | `HookError::Aborted` | AbortSignal triggered | User cancellation; check if intended |
| All hooks denied | `HookRunResult.denied == true` | Hook policy prevents operation | Review hook decision logic; override if needed |
| Permission override | `permission_override` set | Hook elevated/restricted risk | Verify hook is trusted; audit the override |

## Configuration Reference

### `.rigorix/hooks.toml`

```toml
[hooks]
# Commands to run before every tool execution
pre_tool_use = [
    "rigorix-hook-validate-path",
    "rigorix-hook-ci-guard --env $RIGORIX_ENV",
]

# Commands to run after every successful tool execution
post_tool_use = [
    "rigorix-hook-fmt-check --path $TOOL_PATH",
]

# Commands to run after every failed tool execution
post_tool_use_failure = [
    "rigorix-hook-notify --channel alerts",
]

# Hook execution timeout in seconds (default: 30)
timeout_secs = 30

# Whether PreToolUse hooks run sequentially (default: false = concurrent)
sequential_pre_tool_use = true
```

### Environment Variables

| Variable | Set By | Purpose |
|----------|--------|---------|
| `RIGORIX_TOOL_NAME` | Engine | Name of the tool being intercepted |
| `RIGORIX_EVENT` | Engine | Lifecycle event (pre_tool_use, post_tool_use, post_tool_use_failure) |
| `RIGORIX_SESSION_ID` | Engine | Session/execution identifier for correlation |
| `RIGORIX_WORKSPACE` | Engine | Absolute path to workspace root |

## Monitoring

### Metrics

Key metrics to monitor (see `Observability` section in architecture docs):

- `hook_execution_duration_ms` — Per-hook execution latency
- `hook_timeout_count` — Hook timeout counter
- `hook_deny_count` — Number of tool executions blocked by hooks
- `hook_failure_count` — Hook execution failure count
- `hook_total_count` — Total hook executions

### Logging

- Hook lifecycle events emitted via `HookEventPayload` for event bus integration
- Each hook execution produces: start, completed/failed/aborted events
- Hook stdout responses are logged at debug level (redact sensitive input)
- Non-recoverable errors are logged at error level

## Health Check

A healthy hooks module:
1. Configuration loads successfully (syntax-valid TOML, commands exist)
2. Hook commands can be spawned (PATH resolution works)
3. JSON protocol round-trips correctly (valid stdin → valid stdout)
4. AbortSignal operates correctly (set → detected → process killed)

## Troubleshooting

### Hook Not Executing

1. Check `HookConfig.commands_for(event)` returns your commands
2. Verify command exists in PATH (or is absolute path)
3. Check logs for `HookError::CommandNotFound`

### Hook Returns Unexpected Decision

1. Test the hook script directly: `echo '{"event":"pre_tool_use",...}' | your-hook`
2. Validate stdout is valid `HookStdoutResponse` JSON
3. Check stderr — hook scripts should output only JSON on stdout

### Hook Hangs

1. Check `timeout_secs` configuration (default 30s)
2. Verify hook doesn't wait for stdin beyond first read
3. Check for infinite loops in hook script
4. Ensure hook handles `HookAbortSignal` properly

## Performance

| Metric | Target | Notes |
|--------|--------|-------|
| PreToolUse hook latency | < 500ms per hook | Sequential execution by default for input modification |
| PostToolUse hook latency | < 2s per hook | Concurrent execution (no input modification) |
| Hook timeout | 30s (configurable) | Killed on timeout |
| Process spawn overhead | < 10ms | Per hook command |

## Related Documents

- [Architecture: hooks](../.pi/architecture/modules/hooks.md)
- [DR Plan: hooks](dr-plan-hooks.md)
- [Tool System Architecture](../.pi/architecture/modules/tool-system.md)
