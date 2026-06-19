//! Application layer interfaces for the Hook System.
//!
//! @canonical .pi/architecture/modules/hooks.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: #410
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing HookRunner instances
//!
//! # Contract (Frozen)
//! - All service methods return `Result<_, HookError>`
//! - Input/output types are DTOs defined in `dto/`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod dto;
pub mod factory;
pub mod service;

pub use dto::*;
pub use factory::*;
pub use service::*;
