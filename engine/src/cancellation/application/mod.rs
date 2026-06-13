//! Application layer interfaces for the Cancellation bounded context.
//!
//! @canonical .pi/architecture/modules/cancellation.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing domain objects
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, CancellationError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod cancellation_manager_factory_impl;
pub mod cancellation_service_impl;
pub mod dto;
pub mod factory;
pub mod service;

pub use cancellation_manager_factory_impl::*;
pub use cancellation_service_impl::*;
pub use dto::*;
pub use factory::*;
pub use service::*;
