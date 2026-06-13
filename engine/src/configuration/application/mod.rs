//! Application layer interfaces for the Configuration bounded context.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: #2
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing domain objects
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, ConfigurationError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod config_service_impl;
pub mod dto;
pub mod factory;
pub mod secret_service_impl;
pub mod service;

pub use config_service_impl::*;
pub use dto::*;
pub use factory::*;
pub use secret_service_impl::*;
pub use service::*;
