//! Domain entities and interfaces for the Hook System bounded context.
//!
//! @canonical .pi/architecture/modules/hooks.md#domain
//! Implements: Contract Freeze — HookEvent, HookRunResult, HookConfig, HookError
//! Issue: #410
//!
//! This module defines the core domain types:
//! - `HookEvent` — Enum identifying the lifecycle point (PreToolUse, PostToolUse,
//!   PostToolUseFailure)
//! - `HookProtocol` — JSON stdin/stdout contract between engine and hook scripts
//! - `HookRunResult` — Aggregated result from running all hook commands for an event
//! - `HookConfig` — Declarative hook command registration per event
//! - `HookError` — Typed error enum for hook failures
//! - `HookAbortSignal` — Atomic abort flag for cooperative cancellation
//! - `HookEventPayload` — Event payload schemas for hook lifecycle events
//!
//! These are pure domain objects with no framework dependencies.
//! They serve as the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces

pub mod abort;
pub mod config;
pub mod error;
pub mod event;
pub mod event_payload;
pub mod protocol;
pub mod result;

pub use abort::HookAbortSignal;
pub use config::HookConfig;
pub use error::HookError;
pub use event::HookEvent;
pub use event_payload::HookEventPayload;
pub use protocol::*;
pub use result::HookRunResult;
