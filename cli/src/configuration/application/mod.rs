//! Configuration application layer — service traits, DTOs for CLI config operations.
//!
//! @canonical .pi/architecture/modules/configuration.md
//! Implements: Contract Freeze — CliConfigLoader trait, DTO schemas
//! Issue: issue-contract-freeze
//!
//! Defines the application-level contracts for CLI configuration operations.
//! Service traits (use cases) live here, implementations live in infrastructure.
//!
//! # Contract (Frozen)
//! - Service traits define all use cases for CLI config operations
//! - DTOs define input/output schemas for each operation
//! - No implementation — only contract signatures and type definitions

pub mod dto;
pub mod service;

pub use service::CliConfigLoader;
