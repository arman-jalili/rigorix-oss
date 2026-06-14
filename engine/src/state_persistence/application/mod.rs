//! Application layer interfaces and implementations for the State Persistence
//! bounded context.
//!
//! @canonical .pi/architecture/modules/state-persistence.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing StateManager instances
//! - Concrete implementations of all service and factory traits
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, StateError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod dto;
pub mod factory;
pub mod graph_manager_factory_impl;
pub mod graph_manager_service_impl;
pub mod service;
pub mod state_manager_factory_impl;
pub mod state_manager_service_impl;

pub use dto::*;
pub use factory::*;
pub use graph_manager_factory_impl::*;
pub use graph_manager_service_impl::*;
pub use service::*;
pub use state_manager_factory_impl::*;
pub use state_manager_service_impl::*;
