//! Infrastructure layer interfaces for the Consequence Gating bounded context.
//!
//! @canonical .pi/architecture/modules/consequence-gating.md#ddd-layers
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

pub use runner::{PreconditionRun, PreconditionRunner, ProcessPreconditionRunner};
