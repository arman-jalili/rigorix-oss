//! Signal handling interface — re-exported from the application layer.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! The `SignalHandler` trait and `ShutdownLevel` enum are defined in
//! `application/service.rs` (their canonical Clean Architecture location).
//! This module re-exports them for backward compatibility with existing imports.
//!
//! # Contract (Frozen)
//! - `install()` registers the OS signal handler and returns a receiver
//! - Double press detection uses a configurable window (default 2s)
//! - Forwards signals to the engine's `CancellationService`
//! - No signal handler state leaks between process invocations
//!
//! # Migration
//! New code should import directly from
//! `crate::cancellation::application::{SignalHandler, ShutdownLevel}`.
//! This re-export will be removed in a future update.

pub use crate::cancellation::application::service::{ShutdownLevel, SignalHandler};
