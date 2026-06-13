//! Application layer interfaces for the Failure Classification bounded context.
//!
//! @canonical .pi/architecture/modules/failure-classification.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: #33
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing strategy objects
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, FailureClassificationError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod dto;
pub mod factory;
pub mod service;

pub use dto::*;
pub use factory::*;
pub use service::*;
