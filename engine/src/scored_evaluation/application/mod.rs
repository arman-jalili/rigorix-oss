//! Application layer for the Scored Evaluation bounded context.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md
//! Implements: Contract Freeze — application layer interfaces and DTOs
//! Issue: #673 (scored-evaluation epic)
//!
//! This layer defines service traits and data transfer objects (DTOs).
//! All orchestration logic lives behind the service interface.
//!
//! # Contract Freeze
//! - Every use case has a corresponding trait method
//! - Input/output types are DTOs
//! - No implementation — only contract signatures

pub mod dto;
pub mod service;

pub use dto::EvaluateInput;
pub use dto::EvaluateOutput;
pub use dto::EvaluationContext;
pub use service::ScoredEvaluationService;
