//! Template generation application layer — service traits and DTOs.
//!
//! @canonical .pi/architecture/modules/template-generation.md
//! Implements: Contract Freeze — GenerateCommandService trait, DTOs
//! Issue: issue-contract-freeze

pub mod dto;
pub mod service;

pub use service::GenerateCommandService;
