// Stdio integration test — TDD Red→Green→Refactor
//
// @canonical .pi/architecture/modules/mcp-server.md#transport
// Implements: stdio integration — send JSON-RPC, verify response
//
// Spawns the rigorix-mcp binary, sends newline-delimited JSON-RPC messages
// via stdin, reads responses from stdout, and verifies the protocol.

use std::io::Write;
use std::process::{Command, Stdio};
use std::time::Duration;

/// Helper: send a JSON-RPC message to the server's stdin, read one line from stdout.
fn send_rpc(input: &str, stdin: &mut impl Write, stdout: &mut impl std::io::Read) -> String {
    // Write the message
    writeln!(stdin, "{}", input).expect("Failed to write to stdin");
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Read one complete JSON line from stdout
    let mut response = String::new();
    let mut in_json = false;
    let mut depth = 0i32;

    loop {
        let mut byte = [0u8; 1];
        match stdout.read(&mut byte) {
            Ok(0) => break, // EOF
            Ok(_) => {
                let c = byte[0] as char;
                if c == '{' {
                    in_json = true;
                    depth += 1;
                } else if c == '}' {
                    depth -= 1;
                }
                if in_json {
                    response.push(c);
                    if depth == 0 && in_json {
                        break; // Complete JSON object read
                    }
                }
            }
            Err(_) => break,
        }
    }

    response
}

#[test]
fn test_stdio_initialize_handshake() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rigorix-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn rigorix-mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut stdout = child.stdout.take().expect("Failed to open stdout");

    // Give the server time to start
    std::thread::sleep(Duration::from_millis(100));

    // Send initialize
    let response = send_rpc(
        r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","clientInfo":{"name":"test-client","version":"1.0"},"capabilities":{}}}"#,
        &mut stdin,
        &mut stdout,
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&response).expect("Response should be valid JSON");

    // Verify it's a success response
    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 1);

    // Verify protocol version
    assert_eq!(
        parsed["result"]["protocolVersion"], "2025-03-26",
        "Should return negotiated protocol version"
    );

    // Verify capabilities
    assert!(
        parsed["result"]["capabilities"].is_object(),
        "Should have capabilities object"
    );

    // Verify server info
    assert_eq!(
        parsed["result"]["serverInfo"]["name"], "Rigorix MCP Gateway",
        "Should return server name"
    );
    assert_eq!(
        parsed["result"]["serverInfo"]["version"], "0.1.0",
        "Should return server version"
    );

    // Clean up
    drop(stdin);
    let _ = child.wait();
}

#[test]
fn test_stdio_list_tools() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rigorix-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn rigorix-mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut stdout = child.stdout.take().expect("Failed to open stdout");

    std::thread::sleep(Duration::from_millis(100));

    // Send tools/list
    let response = send_rpc(
        r#"{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}"#,
        &mut stdin,
        &mut stdout,
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&response).expect("Response should be valid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 2);

    // Verify tools returns all 14 registered OSS tool descriptors
    let tools = parsed["result"]["tools"]
        .as_array()
        .expect("tools should be an array");
    assert_eq!(
        tools.len(),
        14,
        "tools/list should return 14 registered tool descriptors, got {}",
        tools.len()
    );

    // Verify tool names are correct
    let names: Vec<&str> = tools.iter().filter_map(|t| t["name"].as_str()).collect();
    assert!(
        names.contains(&"rigorix_execute"),
        "Missing rigorix_execute"
    );
    assert!(
        names.contains(&"rigorix_validate_plan"),
        "Missing rigorix_validate_plan"
    );
    assert!(
        names.contains(&"rigorix_check_enforcement"),
        "Missing rigorix_check_enforcement"
    );
    assert!(
        names.contains(&"rigorix_read_audit"),
        "Missing rigorix_read_audit"
    );
    assert!(
        names.contains(&"rigorix_list_audits"),
        "Missing rigorix_list_audits"
    );
    assert!(
        names.contains(&"rigorix_audit_summary"),
        "Missing rigorix_audit_summary"
    );
    assert!(
        names.contains(&"rigorix_list_templates"),
        "Missing rigorix_list_templates"
    );
    assert!(
        names.contains(&"rigorix_get_template"),
        "Missing rigorix_get_template"
    );
    assert!(
        names.contains(&"rigorix_create_template"),
        "Missing rigorix_create_template"
    );
    assert!(
        names.contains(&"rigorix_validate_template"),
        "Missing rigorix_validate_template"
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn test_stdio_list_resources() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rigorix-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn rigorix-mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut stdout = child.stdout.take().expect("Failed to open stdout");

    std::thread::sleep(Duration::from_millis(100));

    // Send resources/list
    let response = send_rpc(
        r#"{"jsonrpc":"2.0","id":3,"method":"resources/list","params":{}}"#,
        &mut stdin,
        &mut stdout,
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&response).expect("Response should be valid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 3);

    // Verify rigorix:// resource templates
    let resources = parsed["result"]["resources"]
        .as_array()
        .expect("resources should be an array");

    let uris: Vec<&str> = resources.iter().filter_map(|r| r["uri"].as_str()).collect();

    assert!(
        uris.contains(&"rigorix://audit/{id}"),
        "Should expose rigorix://audit/{{id}} resource"
    );
    assert!(
        uris.contains(&"rigorix://templates/{name}"),
        "Should expose rigorix://templates/{{name}} resource"
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn test_stdio_list_prompts() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rigorix-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn rigorix-mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut stdout = child.stdout.take().expect("Failed to open stdout");

    std::thread::sleep(Duration::from_millis(100));

    // Send prompts/list
    let response = send_rpc(
        r#"{"jsonrpc":"2.0","id":4,"method":"prompts/list","params":{}}"#,
        &mut stdin,
        &mut stdout,
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&response).expect("Response should be valid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 4);

    // Verify prompt templates
    let prompts = parsed["result"]["prompts"]
        .as_array()
        .expect("prompts should be an array");

    let names: Vec<&str> = prompts.iter().filter_map(|p| p["name"].as_str()).collect();

    assert!(
        names.contains(&"rigorix_introduction"),
        "Should expose rigorix_introduction prompt"
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn test_stdio_method_not_found() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rigorix-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn rigorix-mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut stdout = child.stdout.take().expect("Failed to open stdout");

    std::thread::sleep(Duration::from_millis(100));

    // Send unknown method
    let response = send_rpc(
        r#"{"jsonrpc":"2.0","id":5,"method":"unknown/method","params":{}}"#,
        &mut stdin,
        &mut stdout,
    );

    let parsed: serde_json::Value =
        serde_json::from_str(&response).expect("Response should be valid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0");
    assert_eq!(parsed["id"], 5);

    // Verify error response
    assert!(
        parsed["error"].is_object(),
        "Should return an error for unknown method"
    );
    assert_eq!(
        parsed["error"]["code"], -32601,
        "Should return MethodNotFound error code (-32601)"
    );

    drop(stdin);
    let _ = child.wait();
}

#[test]
fn test_stdio_parse_error() {
    let mut child = Command::new(env!("CARGO_BIN_EXE_rigorix-mcp"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .expect("Failed to spawn rigorix-mcp");

    let mut stdin = child.stdin.take().expect("Failed to open stdin");
    let mut stdout = child.stdout.take().expect("Failed to open stdout");

    std::thread::sleep(Duration::from_millis(100));

    // Send malformed JSON
    let response = send_rpc(r#"this is not json"#, &mut stdin, &mut stdout);

    let parsed: serde_json::Value =
        serde_json::from_str(&response).expect("Response should be valid JSON");

    assert_eq!(parsed["jsonrpc"], "2.0");

    // Verify parse error
    assert!(
        parsed["error"].is_object(),
        "Should return an error for malformed JSON"
    );
    assert_eq!(
        parsed["error"]["code"], -32700,
        "Should return ParseError code (-32700)"
    );

    drop(stdin);
    let _ = child.wait();
}
