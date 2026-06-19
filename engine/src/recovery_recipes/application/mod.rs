//! Application layer interfaces for the Recovery Recipes bounded context.
//!
//! @canonical .pi/architecture/modules/recovery-recipes.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: #438 (recovery-recipes epic)
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - RecoveryContext for per-session attempt tracking
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, RecoveryError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions and structs

pub mod context;
pub mod dto;
pub mod service;
pub mod service_impl;

pub use context::RecoveryContext;
pub use dto::*;
pub use service::RecoveryService;
pub use service_impl::RecoveryServiceImpl;
