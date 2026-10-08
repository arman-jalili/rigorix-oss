//! Infrastructure layer interfaces for the Consequence Gating bounded context.
//!
//! @canonical .pi/architecture/modules/precondition.md#ddd-layers
//! Implements: Contract Freeze — PreconditionRepository + PreconditionRunner
//! Issue: #938 (consequence-gating epic — contract freeze)
//!
//! Config loading from `.rigorix/preconditions.toml` and deterministic argv
//! execution of operator checks (`tokio::process`). All persistence and process
//! interaction is hidden behind ports — no caller touches the TOML format or
//! spawns a process directly.
//!
//! # Contract (Frozen)
//! - `load_config` → `Ok(None)` when the file is absent (fail-open-absent);
//!   `Err(ConfigInvalid)` when it is corrupt / over-cap (fail-closed)
//! - `run` executes argv only (no shell), JSON stdin, documented env, and a
//!   wall-clock timeout; timeout / spawn / trust-boundary map to `error`
//!   (refuse), never pass

pub mod repository;
pub mod runner;

use std::path::Path;

pub use runner::{PreconditionRun, PreconditionRunner, ProcessPreconditionRunner};

/// One-way `sha256:<hex>` of arbitrary bytes.
pub(crate) fn sha256_hex(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

/// Redaction-safe reference to a path for the **signed record**: the basename
/// plus a one-way hash of the full path.
///
/// Anything derived from a precondition failure is recorded in the audit
/// envelope, which is ingested server-side and can be shown to an auditor — and
/// a composition root forwards `PreconditionError::to_string()` into the
/// `unarmed_detail` of a fail-closed gate, so a config error would otherwise
/// carry the operator's absolute path into **every** refusal. An absolute path
/// leaks a username and directory layout, so the record gets
/// `check.mjs (path sha256:…)` — exactly how `authority_path` is already handled
/// (a digest, never the path). The full path stays in the local log.
///
/// SpanPrivacy; ADR-017 §Honest boundary.
pub(crate) fn redacted_path(path: &Path) -> String {
    let basename = path
        .file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| "<unnamed>".to_string());
    format!(
        "{basename} (path {})",
        sha256_hex(path.as_os_str().as_encoded_bytes())
    )
}

/// Name a configured program for the signed record.
///
/// A bare name (`node`, `check`) is already safe and stays readable; anything
/// path-like is reduced to basename + path hash by [`redacted_path`].
pub(crate) fn described_program(program: &str) -> String {
    if program.contains('/') {
        redacted_path(Path::new(program))
    } else {
        program.to_string()
    }
}
