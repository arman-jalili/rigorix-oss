#![allow(dead_code)]

//! Shared helpers for MCP e2e tests (GAP-A-25: previously duplicated in
//! three test files with divergent sleeps).
//!
//! A-25: the suite must not rely on fixed sleeps. Requests are written to
//! the child's stdin immediately (the kernel pipe buffers them until the
//! server's stdio loop is ready) and the response is awaited by polling
//! stdout with a bounded deadline — the server's response IS the wait
//! condition.

use std::io::{Read, Write};
use std::time::{Duration, Instant};

/// How long `send_rpc` waits for a response before failing the test.
pub const RESPONSE_TIMEOUT: Duration = Duration::from_secs(10);

/// Poll interval while awaiting a response.
const POLL_INTERVAL: Duration = Duration::from_millis(25);

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

/// Put a child's stdout pipe into non-blocking mode so `send_rpc` can poll
/// it with a deadline instead of blocking on a fixed sleep.
///
/// Call this once per spawned server, right after `child.stdout.take()`.
#[cfg(unix)]
pub fn set_nonblocking(stdout: &impl std::os::unix::io::AsRawFd) {
    let fd = stdout.as_raw_fd();
    // SAFETY: fd is a valid open pipe from `ChildStdout`; fcntl does not
    // invalidate it and only changes its non-blocking flag.
    let flags = unsafe { libc::fcntl(fd, libc::F_GETFL) };
    assert!(
        flags >= 0,
        "fcntl(F_GETFL) failed: errno {}",
        std::io::Error::last_os_error()
    );
    let rc = unsafe { libc::fcntl(fd, libc::F_SETFL, flags | libc::O_NONBLOCK) };
    assert!(
        rc == 0,
        "fcntl(F_SETFL, O_NONBLOCK) failed: {}",
        std::io::Error::last_os_error()
    );
}

/// Send a JSON-RPC message to the server's stdin, read one line from stdout.
///
/// No fixed sleeps: the request is written immediately (kernel pipe buffers
/// it until the server is ready), then stdout is polled until the response
/// line arrives or `RESPONSE_TIMEOUT` elapses (which fails the test with the
/// request echoed for debugging).
pub fn send_rpc(input: &str, stdin: &mut impl Write, stdout: &mut impl Read) -> String {
    let minified = minify_json(input);
    writeln!(stdin, "{minified}").expect("failed to write request to server stdin");

    #[cfg(unix)]
    {
        let mut buf = [0u8; 8192];
        let mut raw = String::new();
        let deadline = Instant::now() + RESPONSE_TIMEOUT;

        loop {
            match stdout.read(&mut buf) {
                Ok(0) => {
                    // EOF before a response line: the server exited.
                    panic!(
                        "server exited without responding to: {minified}\npartial response: {raw:?}"
                    );
                }
                Ok(n) => {
                    raw.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if raw.contains('\n') {
                        return raw;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    if Instant::now() >= deadline {
                        panic!(
                            "server did not respond within {:?} to: {minified}\npartial response: {raw:?}",
                            RESPONSE_TIMEOUT
                        );
                    }
                    std::thread::sleep(POLL_INTERVAL);
                }
                Err(e) => {
                    panic!("failed reading server stdout: {e}");
                }
            }
        }
    }

    #[cfg(not(unix))]
    {
        // Fallback: blocking read (no poll support for pipes on this target).
        let mut buf = [0u8; 8192];
        let mut raw = String::new();
        loop {
            match stdout.read(&mut buf) {
                Ok(0) => break,
                Ok(n) => {
                    raw.push_str(&String::from_utf8_lossy(&buf[..n]));
                    if raw.contains('\n') {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        raw
    }
}
