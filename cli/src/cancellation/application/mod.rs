//! Cancellation application layer — service traits, DTOs for CLI cancellation.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — SignalHandler trait, DTO schemas
//! Issue: issue-contract-freeze
//!
//! Defines the application-level contracts for CLI cancellation operations.
//! Service traits (use cases) live here, implementations live in infrastructure.
//!
//! # Contract (Frozen)
//! - Service traits define all use cases for CLI cancellation
//! - DTOs define input/output schemas for each operation
//! - No implementation — only contract signatures and type definitions

pub mod dto;
pub mod service;

pub use dto::*;
pub use service::{ShutdownLevel, SignalHandler};
