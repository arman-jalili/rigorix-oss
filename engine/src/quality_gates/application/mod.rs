//! Application layer interfaces for the Quality Gates bounded context.
//!
//! @canonical .pi/architecture/modules/quality-gates.md
//! Implements: Contract Freeze — service traits, DTOs
//! Issue: #449 (quality-gates epic)
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//!
//! # Contract (Frozen)
//! - All service methods are async (return `impl Future`)
//! - All public methods return `Result<_, QualityGateError>`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod dto;
pub mod service;
pub mod service_impl;

pub use dto::*;
pub use service::QualityGateService;
pub use service_impl::QualityGateServiceImpl;
