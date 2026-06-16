//! Cancellation module — signal handlers for graceful shutdown.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Captures Ctrl+C (SIGINT) with double-press detection.
//! Single press = graceful shutdown. Double press within 2s = immediate.

pub mod application;
pub mod domain;
pub mod infrastructure;
