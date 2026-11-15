//! The server refuses its command line (#1589).
//!
//! Before this, `main.rs` read no arguments: `--port 3011` bound 3001 and said nothing, and `--help`
//! started a server. Both are asserted against **the binary**, because the defect was that the
//! decision was never consulted.
//!
//! Every case runs under a watchdog. A test whose subject is "the process does not start" must not be
//! able to block when it does: measured, discarding the arguments again made the `--help` case hang
//! until the runner killed it, which stalls a gate instead of reporting a failure. A killed process
//! here is a failed assertion with a message, not a hung test.

// This file's subject *is* the process boundary: the defect was that the binary ignored its
// arguments, and the only way to assert an exit code and a log line from a binary is to spawn it.
// The workspace forbids `std::process::Command` outside the sink register (ADR-023); a test harness
// for a CLI is the case that rule's `// sink:` rows do not cover, so the allow is here with its
// reason rather than a spawn in production code.
#![allow(clippy::disallowed_types)]

use std::io::Read;
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// The binary under test, built by cargo for this test target.
fn server() -> Command {
    Command::new(env!("CARGO_BIN_EXE_rigorix-server"))
}

/// Run to completion or kill after `limit`. `None` means the process was still running — the outcome
/// this file exists to make visible rather than hang on.
fn run(mut command: Command, limit: Duration) -> (Option<i32>, String, String) {
    let mut child = command
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("the binary runs");

    let mut stdout = child.stdout.take().expect("piped");
    let mut stderr = child.stderr.take().expect("piped");
    let out_thread = std::thread::spawn(move || {
        let mut buffer = String::new();
        let _ = stdout.read_to_string(&mut buffer);
        buffer
    });
    let err_thread = std::thread::spawn(move || {
        let mut buffer = String::new();
        let _ = stderr.read_to_string(&mut buffer);
        buffer
    });

    let deadline = Instant::now() + limit;
    let mut code = None;
    while Instant::now() < deadline {
        match child.try_wait().expect("try_wait") {
            Some(status) => {
                code = status.code();
                break;
            }
            None => std::thread::sleep(Duration::from_millis(25)),
        }
    }
    if code.is_none() && child.try_wait().expect("try_wait").is_none() {
        let _ = child.kill();
        let _ = child.wait();
    }

    let stdout = out_thread.join().unwrap_or_default();
    let stderr = err_thread.join().unwrap_or_default();
    (code, stdout, stderr)
}

#[test]
fn help_exits_zero_prints_the_surface_and_binds_nothing() {
    let mut command = server();
    command.arg("--help");
    let (code, stdout, stderr) = run(command, Duration::from_secs(10));

    assert_eq!(
        code,
        Some(0),
        "--help must exit 0 — a process still running after 10s was killed, which is the old \
         behaviour of starting a server. stdout: {stdout} stderr: {stderr}"
    );
    assert!(stdout.contains("usage: rigorix-server"), "{stdout}");
    assert!(
        stdout.contains("RIGORIX_SERVER_BIND"),
        "the message names the surface that actually decides the address: {stdout}"
    );
    assert!(
        !stderr.contains("listening"),
        "--help must not reach the bind path: {stderr}"
    );
}

#[test]
fn an_unrecognised_argument_refuses_with_exit_two() {
    // The issue's own command: `--port 3011`. It must not start a server on 3001 in silence.
    let mut command = server();
    command.args(["--port", "3011"]);
    let (code, stdout, stderr) = run(command, Duration::from_secs(10));

    assert_eq!(
        code,
        Some(2),
        "a usage error is exit 2 — a killed process here means the argument was accepted and the \
         server started. stdout: {stdout} stderr: {stderr}"
    );
    assert!(stderr.contains("--port"), "{stderr}");
    assert!(
        stderr.contains("RIGORIX_SERVER_BIND"),
        "the refusal names the surface that works: {stderr}"
    );
    assert!(
        stderr.contains("127.0.0.1:3001"),
        "and the address it would have used, so the mistake is visible: {stderr}"
    );
    assert!(
        !stderr.contains("listening"),
        "the refusal happens before anything binds: {stderr}"
    );
}

#[test]
fn a_refusal_quotes_the_address_the_environment_configured() {
    let mut command = server();
    command
        .arg("--bind")
        .env("RIGORIX_SERVER_BIND", "0.0.0.0:4100");
    let (code, _stdout, stderr) = run(command, Duration::from_secs(10));

    assert_eq!(code, Some(2));
    assert!(
        stderr.contains("0.0.0.0:4100"),
        "the refusal quotes the address this process would have used: {stderr}"
    );
}
