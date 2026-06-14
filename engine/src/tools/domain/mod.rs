//! Domain entities and interfaces for the Tool System bounded context.
//!
//! @canonical .pi/architecture/modules/tool-system.md#domain
//! Implements: Contract Freeze — Tool trait, ToolError, ToolEvent, risk mapping
//! Issue: #124
//!
//! This module defines the core domain types:
//! - `Tool` trait — the core abstraction for all tool implementations
//! - `ToolError` — structured error type for tool execution failures
//! - `ToolEvent` — event payload schemas emitted by tool operations
//! - Risk level mapping — per-tool risk classification
//!
//! These are pure domain objects with no framework dependencies. They serve as
//! the frozen contract that all implementation must satisfy.
//!
//! # Contract (Frozen)
//! - No implementation logic beyond constructors and field accessors
//! - All validation must happen in the application layer (service traits)
//! - All persistence must happen behind repository interfaces

pub mod error;
pub mod event;
pub mod risk_mapping;
pub mod tool_trait;

pub use error::ToolError;
pub use tool_trait::Tool;
