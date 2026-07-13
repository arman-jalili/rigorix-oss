//! End-to-end integration tests for the Enterprise Proxy.
//!
//! Tests the conditional wiring in main.rs:
//! 1. Without config: no enterprise tools appear → clear error when calling one
//! 2. With config: enterprise tools appear → call returns diagnostic error
//!    (enterprise API isn't running in tests, so we verify error handling)
//!
//! @canonical .pi/architecture/modules/enterprise-proxy.md
//! Implements: Acceptance Criteria #8, #9 — E2E conditional wiring

use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Stdio};
use std::time::Duration;

/// Minify a multi-line JSON string to a single line (for line-delimited JSON).
fn minify_json(json: &str) -> String {
    let mut result = String::with_capacity(json.len());
    let mut in_string = false;
    let mut prev_escape = false;
    for c in json.chars() {
        match c {
            '\\' if in_string => {
                result.push(c);
                prev_escape = !prev_escape;
            }
            '"' if !prev_escape => {
                result.push(c);
                in_string = !in_string;
                prev_escape = false;
            }
            _ if !in_string && (c == ' ' || c == '\n' || c == '\t' || c == '\r') => {}
            _ => {
                result.push(c);
                prev_escape = false;
            }
        }
    }
    result
}

/// Spawn the MCP binary, send one request, read one response, kill the process.
fn send_one_request(
    env_vars: &[(&str, &str)],
    request: &str,
    timeout_secs: u64,
) -> Result<serde_json::Value, String> {
    let binary = env!("CARGO_BIN_EXE_rigorix-mcp");
    let mut cmd = Command::new(binary);
    cmd.stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null());

    // Clear any inherited enterprise env to ensure clean test state
    cmd.env_remove("ENTERPRISE_API_URL");
    cmd.env_remove("ENTERPRISE_API_KEY");

    for (key, val) in env_vars {
        cmd.env(key, val);
    }

    let mut child = cmd.spawn().map_err(|e| format!("spawn: {}", e))?;

    let mut stdin = child.stdin.take().ok_or("no stdin")?;
    let stdout = child.stdout.take().ok_or("no stdout")?;
    let mut reader = BufReader::new(stdout);

    let minified = minify_json(request);
    writeln!(stdin, "{}", minified).map_err(|e| format!("write: {}", e))?;
    stdin.flush().map_err(|e| format!("flush: {}", e))?;

    // Read one line from stdout
    let start = std::time::Instant::now();
    let mut line = String::new();
    loop {
        if start.elapsed() > Duration::from_secs(timeout_secs) {
            let _ = child.kill();
            return Err("timeout".into());
        }
        match reader.read_line(&mut line) {
            Ok(0) => {
                let _ = child.kill();
                return Err("EOF before response line".into());
            }
            Ok(_) => break,
            Err(e) => {
                let _ = child.kill();
                return Err(format!("read: {}", e));
            }
        }
    }

    let _ = child.kill();
    let _ = child.wait();

    serde_json::from_str(&line).map_err(|e| format!("json parse: {} from: {}", e, line))
}

/// Helper: get tool names from a tools/list response.
fn get_tool_names(response: &serde_json::Value) -> Vec<String> {
    response["result"]["tools"]
        .as_array()
        .map(|tools| {
            tools
                .iter()
                .filter_map(|t| t["name"].as_str().map(|s| s.to_string()))
                .collect()
        })
        .unwrap_or_default()
}

// =========================================================================
// Tests
// =========================================================================

#[test]
fn test_without_enterprise_config_no_enterprise_tools() {
    // Initialize
    let init = serde_json::json!({
        "jsonrpc": "2.0", "method": "initialize",
        "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0"}},
        "id": 1
    });
    let init_resp = send_one_request(&[], &init.to_string(), 5).unwrap();
    assert_eq!(init_resp["id"], 1, "init should succeed");

    // tools/list — no enterprise config
    let list_req = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "params": {}, "id": 2
    });
    let list_resp = send_one_request(&[], &list_req.to_string(), 5).unwrap();
    let tool_names = get_tool_names(&list_resp);

    // Exactly 13 OSS tools (including rigorix_get_usage_guide, rigorix_plan, rigorix_run), no enterprise tools
    assert_eq!(
        tool_names.len(),
        13,
        "expected 13 OSS tools, got {}: {:?}",
        tool_names.len(),
        tool_names
    );
    assert!(
        !tool_names
            .iter()
            .any(|n| n.starts_with("rigorix_enterprise_")),
        "enterprise tools should not appear without config"
    );
}

#[test]
fn test_without_enterprise_config_enterprise_call_returns_clear_error() {
    // Initialize + call enterprise tool
    let init = serde_json::json!({
        "jsonrpc": "2.0", "method": "initialize",
        "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0"}},
        "id": 1
    });
    let _ = send_one_request(&[], &init.to_string(), 5).unwrap();

    let call_req = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "rigorix_enterprise_team_audit", "arguments": {"team_id": "123"}},
        "id": 2
    });
    let call_resp = send_one_request(&[], &call_req.to_string(), 5).unwrap();

    let is_error = call_resp["result"]["isError"].as_bool().unwrap_or(false);
    assert!(is_error, "enterprise call without config should error");
    let text = call_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("");
    assert!(
        text.contains("not configured") || text.contains("not enabled"),
        "error should mention config, got: {}",
        text
    );
}

#[test]
fn test_with_enterprise_config_tools_appear() {
    // With enterprise config, tools/list should include enterprise tools
    let init_req = serde_json::json!({
        "jsonrpc": "2.0", "method": "initialize",
        "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0"}},
        "id": 1
    });
    let env = &[
        ("ENTERPRISE_API_URL", "https://enterprise-test.example.com"),
        ("ENTERPRISE_API_KEY", "sk-test-key-for-e2e"),
        ("ENTERPRISE_TIMEOUT_SECS", "2"),
        ("ENTERPRISE_TLS_VERIFY", "false"),
    ];
    let _ = send_one_request(env, &init_req.to_string(), 5).unwrap();

    let list_req = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/list", "params": {}, "id": 2
    });
    let list_resp = send_one_request(env, &list_req.to_string(), 5).unwrap();
    let tool_names = get_tool_names(&list_resp);

    // Should have 13 OSS + at least the static enterprise tool
    assert!(
        tool_names.len() >= 14,
        "expected 13+ tools with enterprise config, got {}: {:?}",
        tool_names.len(),
        tool_names
    );
    assert!(
        tool_names.iter().any(|n| n == "rigorix_enterprise_call"),
        "expected rigorix_enterprise_call, got: {:?}",
        tool_names
    );
}

#[test]
fn test_with_enterprise_config_call_returns_diagnostic_error() {
    // With valid config but no real API, enterprise call returns a diagnostic error
    let init_req = serde_json::json!({
        "jsonrpc": "2.0", "method": "initialize",
        "params": {"protocolVersion": "2025-03-26", "capabilities": {}, "clientInfo": {"name": "test", "version": "1.0"}},
        "id": 1
    });
    let env = &[
        ("ENTERPRISE_API_URL", "https://enterprise-test.example.com"),
        ("ENTERPRISE_API_KEY", "sk-test-key-for-e2e"),
        ("ENTERPRISE_TIMEOUT_SECS", "2"),
        ("ENTERPRISE_TLS_VERIFY", "false"),
    ];
    let _ = send_one_request(env, &init_req.to_string(), 5).unwrap();

    // Call enterprise tool — should get a diagnostic error, not a crash
    let call_req = serde_json::json!({
        "jsonrpc": "2.0", "method": "tools/call",
        "params": {"name": "rigorix_enterprise_team_audit", "arguments": {"team_id": "123"}},
        "id": 2
    });
    let call_resp = send_one_request(env, &call_req.to_string(), 10).unwrap();

    let is_error = call_resp["result"]["isError"].as_bool().unwrap_or(true);
    let text = call_resp["result"]["content"][0]["text"]
        .as_str()
        .unwrap_or("");

    assert!(
        is_error,
        "enterprise call to fake server should error, got: {}",
        text
    );
    assert!(!text.is_empty(), "error should have content");
    // Should be a structured diagnostic, not a raw Rust error dump
    assert!(
        text.contains("error")
            || text.contains("timeout")
            || text.contains("unreachable")
            || text.contains("resolution_hint")
            || text.contains("Configuration"),
        "should be a clear diagnostic: {}",
        text
    );
}
