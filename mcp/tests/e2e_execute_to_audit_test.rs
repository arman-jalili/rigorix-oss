//! End-to-end integration test: plan execution → audit query cycle.
//!
//! Spawns the rigorix-mcp binary, sends a rigorix_execute tool call,
//! extracts the execution_id from the response, then sends
//! rigorix_read_audit to verify the audit cycle.
//!
//! @canonical .pi/architecture/modules/execution-tools.md
//! Implements: Acceptance Criterion #12 — end-to-end plan execution → audit

use std::io::Write;
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
            _ if !in_string && (c == ' ' || c == '\n' || c == '\t' || c == '\r') => {
                // skip whitespace outside strings
                prev_escape = false;
            }
            _ => {
                result.push(c);
                prev_escape = false;
            }
        }
    }
    result
}

/// Send a JSON-RPC message to the server's stdin, read one line from stdout.
fn send_rpc(input: &str, stdin: &mut impl Write, stdout: &mut impl std::io::Read) -> String {
    // Minify to a single line (server reads line-delimited JSON)
    let minified = minify_json(input);
    writeln!(stdin, "{}", minified).expect("Failed to write to stdin");
    std::thread::sleep(Duration::from_millis(200));

    // Read the first complete JSON object, ignoring any trailing data
    let mut buf = [0u8; 8192];
    let mut raw = String::new();

    loop {
        match stdout.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => {
                raw.push_str(&String::from_utf8_lossy(&buf[..n]));
            }
            Err(_) => break,
        }

        // Scan for the first complete JSON object
        if let Some(start) = raw.find('{') {
            let mut depth = 0i32;
            let mut end = None;
            for (i, c) in raw[start..].char_indices() {
                match c {
                    '{' => depth += 1,
                    '}' => {
                        depth -= 1;
                        if depth == 0 {
                            end = Some(start + i + 1);
                            break;
                        }
                    }
                    _ => {}
                }
            }
            if let Some(end) = end {
                return raw[start..end].to_string();
            }
        }

        // Wait for more data
        std::thread::sleep(Duration::from_millis(100));
    }

    // Fallback: return what we have
    raw.trim().to_string()
}

#[test]
fn test_e2e_execute_to_audit_cycle() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rigorix-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn rigorix-mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut stdout = child.stdout.take().expect("Failed to open stdout");

    // Give the server time to start
    std::thread::sleep(Duration::from_millis(200));

    // -----------------------------------------------------------------------
    // Step 1: Initialize the server
    // -----------------------------------------------------------------------
    let resp = send_rpc(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","clientInfo":{"name":"e2e-test","version":"1.0"},"capabilities":{}}}"#,
        &mut stdin,
        &mut stdout,
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&resp).expect("Initialize response should be valid JSON");
    assert_eq!(parsed["id"], 1, "Initialize should succeed");

    // -----------------------------------------------------------------------
    // Step 2: Register a template first (real engine requires it)
    // -----------------------------------------------------------------------
    let resp = send_rpc(
        r#"{
            "jsonrpc":"2.0",
            "id":2,
            "method":"tools/call",
            "params":{
                "name":"rigorix_create_template",
                "arguments":{
                    "name":"e2e-test-plan",
                    "plan":{
                        "name":"e2e-test-plan",
                        "description":"End-to-end test plan",
                        "version":"1.0.0",
                        "tags":[],
                        "steps":[
                            {
                                "name":"step-1",
                                "tool":"test_tool",
                                "parameters":{},
                                "requires_approval":false,
                                "description":"Test step"
                            }
                        ],
                        "metadata":{},
                        "created_at":"2026-07-12T00:00:00Z",
                        "updated_at":"2026-07-12T00:00:00Z"
                    },
                    "overwrite":true
                }
            }
        }"#,
        &mut stdin,
        &mut stdout,
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&resp).expect("Template create response should be valid JSON");
    assert!(
        !parsed["result"]["isError"].as_bool().unwrap_or(true),
        "Template creation should succeed, got: {:?}",
        parsed
    );

    // -----------------------------------------------------------------------
    // Step 3: Execute a plan via the template name
    // -----------------------------------------------------------------------
    let resp = send_rpc(
        r#"{
            "jsonrpc":"2.0",
            "id":3,
            "method":"tools/call",
            "params":{
                "name":"rigorix_execute",
                "arguments":{
                    "plan":{
                        "name":"e2e-test-plan",
                        "description":"End-to-end test plan",
                        "version":"1.0.0",
                        "tags":[],
                        "steps":[
                            {
                                "name":"step-1",
                                "tool":"test_tool",
                                "parameters":{},
                                "requires_approval":false,
                                "description":"Test step"
                            }
                        ],
                        "metadata":{},
                        "created_at":"2026-07-12T00:00:00Z",
                        "updated_at":"2026-07-12T00:00:00Z"
                    }
                }
            }
        }"#,
        &mut stdin,
        &mut stdout,
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&resp).expect("Execute response should be valid JSON");
    assert_eq!(parsed["id"], 3, "Execute should have matching id");
    assert!(
        !parsed["result"]["isError"].as_bool().unwrap_or(true),
        "Execute should succeed, got: {:?}",
        parsed
    );

    // Extract execution_id from the response content
    let content_text = parsed["result"]["content"][0]["text"]
        .as_str()
        .expect("Execute response should have text content");
    let content_json: serde_json::Value =
        serde_json::from_str(content_text).expect("Content should be valid JSON");
    let execution_id = content_json["execution_id"]
        .as_str()
        .expect("Execute result should contain execution_id")
        .to_string();

    assert!(!execution_id.is_empty(), "execution_id should not be empty");

    // -----------------------------------------------------------------------
    // Step 3: Read audit trail with the execution_id
    // -----------------------------------------------------------------------
    let read_audit = format!(
        r#"{{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{{"name":"rigorix_read_audit","arguments":{{"execution_id":"{}","format":"json"}}}}}}"#,
        execution_id
    );
    let resp = send_rpc(&read_audit, &mut stdin, &mut stdout);
    let parsed: serde_json::Value =
        serde_json::from_str(&resp).expect("Read audit response should be valid JSON");
    assert_eq!(parsed["id"], 3, "Read audit should have matching id");
    assert!(
        !parsed["result"]["isError"].as_bool().unwrap_or(true),
        "Read audit should succeed, got: {:?}",
        parsed
    );

    // Parse the JSON audit response and verify execution_id matches
    let audit_text = parsed["result"]["content"][0]["text"]
        .as_str()
        .expect("Audit response should have text content");
    let audit_json: Result<serde_json::Value, _> = serde_json::from_str(audit_text);
    match audit_json {
        Ok(ref json) => {
            let reported_id = json["execution_id"]
                .as_str()
                .unwrap_or("no-id-field")
                .to_string();
            assert_eq!(
                reported_id, execution_id,
                "Audit execution_id should match the execute result"
            );
        }
        Err(e) => {
            panic!(
                "Audit response text should be valid JSON, got error: {}. Text: {}",
                e, audit_text
            );
        }
    }

    // -----------------------------------------------------------------------
    // Step 4: Verify the end-to-end cycle is complete
    // -----------------------------------------------------------------------

    // Clean up
    drop(stdin);
    let _ = child.wait();
}

#[test]
fn test_e2e_tools_list_contains_all_tools() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rigorix-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn rigorix-mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut stdout = child.stdout.take().expect("Failed to open stdout");

    std::thread::sleep(Duration::from_millis(200));

    // Initialize
    let _ = send_rpc(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &mut stdin,
        &mut stdout,
    );

    // List tools
    let resp = send_rpc(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        &mut stdin,
        &mut stdout,
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&resp).expect("Response should be valid JSON");
    let tools = parsed["result"]["tools"]
        .as_array()
        .expect("tools should be an array");

    // Verify all 10 OSS tools are present
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();

    let expected: &[&str] = &[
        "rigorix_execute",
        "rigorix_validate_plan",
        "rigorix_check_enforcement",
        "rigorix_read_audit",
        "rigorix_list_audits",
        "rigorix_audit_summary",
        "rigorix_list_templates",
        "rigorix_get_template",
        "rigorix_create_template",
        "rigorix_validate_template",
        "rigorix_get_usage_guide",
    ];

    for tool in expected {
        assert!(
            names.contains(tool),
            "Missing tool '{}' in tools/list. Got: {:?}",
            tool,
            names
        );
    }

    assert_eq!(
        names.len(),
        11,
        "Should have exactly 11 tools, got {}",
        names.len()
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn test_e2e_create_template_then_get_template() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rigorix-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn rigorix-mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut stdout = child.stdout.take().expect("Failed to open stdout");

    std::thread::sleep(Duration::from_millis(200));

    // Initialize
    let _ = send_rpc(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}"#,
        &mut stdin,
        &mut stdout,
    );

    // Create a template (PlanTemplate deserialization requires created_at/updated_at)
    let resp = send_rpc(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"rigorix_create_template","arguments":{"name":"e2e-test-template","plan":{"name":"e2e-test-template","description":"E2E test template","version":"1.0.0","steps":[{"name":"step-1","tool":"test_tool","parameters":{},"requires_approval":false,"description":"Test step"}],"created_at":"2026-01-01T00:00:00Z","updated_at":"2026-01-01T00:00:00Z"}}}}"#,
        &mut stdin,
        &mut stdout,
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&resp).expect("Create response should be valid JSON");
    assert_eq!(parsed["id"], 2);
    assert!(
        !parsed["result"]["isError"].as_bool().unwrap_or(true),
        "Create template should succeed, got: {:?}",
        parsed
    );

    // Get the template
    let resp = send_rpc(
        r#"{"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"rigorix_get_template","arguments":{"name":"e2e-test-template"}}}"#,
        &mut stdin,
        &mut stdout,
    );
    let parsed: serde_json::Value =
        serde_json::from_str(&resp).expect("Get response should be valid JSON");
    assert_eq!(parsed["id"], 3);
    assert!(
        !parsed["result"]["isError"].as_bool().unwrap_or(true),
        "Get template should succeed, got: {:?}",
        parsed
    );

    // Verify we got our template back
    let content_text = parsed["result"]["content"][0]["text"]
        .as_str()
        .expect("Get response should have text content");
    assert!(
        content_text.contains("e2e-test-template"),
        "Get should return the created template"
    );

    // Clean up the template file
    let template_path = std::path::Path::new(".rigorix/templates/e2e-test-template.toml");
    if template_path.exists() {
        let _ = std::fs::remove_file(template_path);
    }

    drop(stdin);
    let _ = child.wait();
}
