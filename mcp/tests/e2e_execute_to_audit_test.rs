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

mod common;

use common::{minify_json, send_rpc};
