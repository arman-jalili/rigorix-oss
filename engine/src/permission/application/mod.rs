//! Application layer interfaces for the Permission Enforcer bounded context.
//!
//! @canonical .pi/architecture/modules/permission-enforcer.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: issue-contract-freeze
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing PermissionEnforcer instances
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return domain types
//! - DTOs include documentation
//! - No implementation logic — only trait definitions

pub mod dto;
pub mod enforcer;
pub mod factory;

pub use dto::*;
pub use enforcer::*;
pub use factory::*;
