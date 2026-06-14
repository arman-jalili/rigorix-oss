//! Application layer interfaces for the Template System bounded context.
//!
//! @canonical .pi/architecture/modules/template-system.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: #101
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing domain objects
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, TemplateError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod dto;
pub mod factory;
pub mod service;

pub use dto::*;
pub use factory::*;
pub use service::*;
