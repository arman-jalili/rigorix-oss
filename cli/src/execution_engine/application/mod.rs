//! Execution engine application layer — service traits, DTOs.
//!
//! @canonical .pi/architecture/modules/execution-engine.md
//! Implements: Contract Freeze — ExecutionCommandService trait, DTOs
//! Issue: issue-contract-freeze

pub mod dto;
pub mod service;

pub use service::ExecutionCommandService;
