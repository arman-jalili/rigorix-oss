//! Application layer interfaces for the Event System.
//!
//! @canonical .pi/architecture/modules/event-system.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: #46
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing EventBus instances
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, EventSystemError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod dto;
pub mod event_bus_factory_impl;
pub mod event_bus_service_impl;
pub mod factory;
pub mod service;

pub use dto::*;
pub use event_bus_factory_impl::*;
pub use event_bus_service_impl::*;
pub use factory::*;
pub use service::*;
