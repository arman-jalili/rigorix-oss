//! Templates application layer — service traits, DTOs for CLI template operations.
//!
//! @canonical .pi/architecture/modules/templates.md
//! Implements: Contract Freeze — TemplateCommandService trait, DTO schemas
//! Issue: issue-contract-freeze
//!
//! Defines the application-level contracts for CLI template commands.
//! Service traits (use cases) live here, implementations live in infrastructure.
//!
//! # Contract (Frozen)
//! - Service traits define all use cases for CLI template operations
//! - DTOs define input/output schemas for each operation
//! - No implementation — only contract signatures and type definitions

pub mod dto;
pub mod service;

pub use dto::*;
pub use service::TemplateCommandService;
