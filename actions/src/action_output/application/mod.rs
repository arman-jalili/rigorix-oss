//! Application layer for the Action Output bounded context.
//!
//! @canonical actions/.pi/architecture/modules/action-output.md#application
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
//! - All public methods return `Result<_, ActionOutputError>`
//! - DTOs carry full documentation for each field
//! - No implementation — only contract signatures

pub mod annotation_writer_impl;
pub mod dto;
pub mod factory;
pub mod output_formatter_impl;
pub mod service;
pub mod step_summary_writer_impl;

pub use annotation_writer_impl::*;
pub use dto::*;
pub use factory::*;
pub use output_formatter_impl::*;
pub use service::*;
pub use step_summary_writer_impl::*;
