//! Quality Gates bounded context.
//!
//! @canonical .pi/architecture/modules/quality-gates.md
//! Implements: Contract Freeze — quality-gates module
//! Issue: #449 (quality-gates epic)
//!
//! This module formalizes test quality into a four-tier escalation:
//! `TargetedTests` → `Package` → `Workspace` → `MergeReady`. Each tier
//! represents a broader scope of validation. The `GreenContract` pattern
//! allows users and the orchestrator to declare a required quality level,
//! and the engine verifies whether the observed test scope satisfies the
//! contract.
//!
//! This replaces the binary "tests passed" signal with a structured quality
//! framework that gates merge/closeout decisions.
//!
//! # Architecture
//!
//! ```text
//! quality_gates/
//! ├── domain/               # Domain entities
//! │   ├── level.rs          # QualityLevel enum (4 tiers)
//! │   ├── contract.rs       # GreenContract struct + evaluate()
//! │   ├── outcome.rs        # QualityGateOutcome enum
//! │   ├── config.rs         # QualityGateConfig struct
//! │   ├── event.rs          # QualityGateEvent payload schemas
//! │   └── error.rs          # QualityGateError enum
//! ├── application/          # Service traits, DTOs
//! │   ├── service.rs        # QualityGateService trait
//! │   └── dto.rs            # Input/Output DTOs
//! ├── infrastructure/       # Repository interfaces
//! │   └── repository.rs     # QualityGateConfigRepository trait
//! └── interfaces/           # API contracts
//!     └── http.rs           # REST endpoint contracts
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
