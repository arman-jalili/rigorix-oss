//! Application layer for the Security Configuration bounded context.
//!
//! @canonical actions/.pi/architecture/modules/security-config.md#application
//! Implements: Contract Freeze — service traits, DTO schemas, factory interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines the application-level contracts:
//! - Service interfaces (use cases) in `service.rs`
//! - Input/output DTO schemas in `dto/`
//! - Factory interfaces in `factory.rs`
//!
//! # Contract (Frozen)
//! - All service traits are async (use `async-trait`)
//! - All public methods return `Result<_, SecurityError>`
//! - DTOs carry full documentation for each field
//! - No implementation — only contract signatures

pub mod dto;
pub mod factory;
pub mod fork_detector_impl;
pub mod service;

pub use dto::*;
pub use factory::*;
pub use service::*;
