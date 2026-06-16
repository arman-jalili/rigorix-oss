//! Execution engine domain types.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: Contract Freeze — ExecutionCliError, ExecutionCliEvent
//! Issue: issue-contract-freeze

pub mod error;
pub mod event;

pub use error::ExecutionCliError;
pub use event::ExecutionCliEvent;
