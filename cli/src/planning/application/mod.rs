//! Planning application layer — service traits, DTOs.
//!
//! @canonical .pi/architecture/modules/planning-pipeline.md
//! Implements: Contract Freeze — PlanCommandService trait, DTOs
//! Issue: issue-contract-freeze

pub mod dto;
pub mod service;

pub use service::PlanCommandService;
