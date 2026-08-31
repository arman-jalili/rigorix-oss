#![allow(dead_code)]

//! Shared helpers for MCP e2e tests (GAP-A-25: previously duplicated in
//! three test files with divergent sleeps).

use std::io::Write;
use std::time::Duration;

/// Minify a multi-line JSON string to a single line (for line-delimited JSON).
pub fn minify_json(json: &str) -> String {
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
///
/// The initial 200ms delay is a handshake settle period; the read loop then
/// blocks until the server responds, which is the effective poll.
pub fn send_rpc(input: &str, stdin: &mut impl Write, stdout: &mut impl std::io::Read) -> String {
    let minified = minify_json(input);
    writeln!(stdin, "{}", minified).expect("Failed to write to stdin");
    std::thread::sleep(Duration::from_millis(200));

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
        // Read until the full line is available.
        if raw.contains('\n') {
            break;
        }
    }

    raw
}
