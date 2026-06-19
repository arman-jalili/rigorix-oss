//! Application layer interfaces for the Code Generation Pipeline.
//!
//! @canonical .pi/architecture/modules/code-generation.md
//! Implements: Contract Freeze — service traits, DTOs, factory interfaces
//! Issue: #424
//!
//! This module defines:
//! - Service traits (use cases / application services)
//! - Input/Output DTOs with validation
//! - Factory interfaces for constructing code generation services
//!
//! # Contract (Frozen)
//! - All service methods return `Result<_, CodeGenError>`
//! - Input/output types are DTOs defined in `dto/`
//! - DTOs include validation annotations/documentation
//! - No implementation logic — only trait definitions

pub mod dto;
pub mod factory;
pub mod service;
pub mod syntax_gate_impl;

pub use dto::*;
pub use factory::*;
pub use service::*;
pub use syntax_gate_impl::*;
