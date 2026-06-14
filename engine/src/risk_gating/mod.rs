//! Risk Gating bounded context.
//!
//! @canonical .pi/architecture/modules/risk-gating.md
//! Implements: Contract Freeze — risk-gating module root
//! Issue: issue-contract-freeze
//!
//! Classifies tools/tasks by risk level (Low, Medium, High) and enforces
//! gating policies. Every tool invocation passes through the risk gate
//! before execution. The gate determines whether the tool auto-executes,
//! requires user confirmation, or runs in dry-run mode.
//!
//! # Architecture
//!
//! ```text
//! risk_gating/
//! ├── domain/               # Core domain entities (RiskLevel, RiskConfig), errors, events
//! │   ├── risk_level.rs     # RiskLevel enum (Low, Medium, High)
//! │   ├── risk_classifier.rs# RiskClassifier trait — maps tool name → RiskLevel
//! │   ├── risk_config.rs    # RiskConfig struct — configurable policy overrides
//! │   ├── error.rs          # RiskGatingError enum
//! │   └── event/            # RiskGateEvent payload schemas
//! ├── application/          # Service traits, DTOs, factory interfaces
//! │   ├── service.rs        # RiskGateService trait
//! │   ├── factory.rs        # RiskGateFactory interface
//! │   └── dto/              # Input/Output DTOs with validation
//! ├── infrastructure/       # Repository interfaces
//! │   └── repository/       # RiskConfigRepository trait
//! └── interfaces/           # API contracts
//!     └── http/             # REST endpoint contracts
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract
//!
//! # Related Components
//!
//! - `ToolRiskLevel` (in `crate::enforcement::domain::config`) is the existing
//!   enforcement risk level. The risk-gating module provides a higher-level
//!   classification per tool or operation.
//! - `ExecutionEvent::ToolExecuted` carries the `risk_level` field that this
//!   module populates during gating.

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
