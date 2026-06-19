//! Application layer interfaces for the Policy Engine bounded context.
//!
//! @canonical .pi/architecture/modules/policy-engine.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing PolicyEngine instances
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, PolicyEngineError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod dto;
pub mod engine;
pub mod factory;

pub use dto::*;
pub use engine::*;
pub use factory::*;
