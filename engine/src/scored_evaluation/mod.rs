//! Scored Evaluation bounded context.
//!
//! @canonical .pi/architecture/modules/scored-evaluation.md
//! Implements: Contract Freeze — scored-evaluation module
//! Issue: #673 (scored-evaluation epic)
//!
//! The Scored Evaluation system adds a scored quality evaluation primitive to
//! the Rigorix DAG. A `scored_evaluation` node sends generated artifacts to a
//! pluggable scoring backend and receives multidimensional scores back. Policy
//! rules can gate merge on score thresholds.
//!
//! This complements the Quality Gates system (GreenContract), which evaluates
//! test scope, by adding output quality scoring as an orthogonal dimension.
//!
//! # Protocol Ownership
//!
//! Rigorix defines the scoring protocol (`rigorix_evaluate_artifact`,
//! `rigorix_ping`, etc.). External scoring systems (e.g. RuntimeAI) adopt
//! this protocol by implementing the server side. The initial protocol design
//! is informed by RuntimeAI's conceptual model (checkrides, scenarios, rubrics)
//! since they are the first planned backend adopter.
//!
//! # Architecture
//!
//! ```text
//! scored_evaluation/
//! ├── domain/               # Domain entities and interfaces
//! │   ├── node.rs           # ScoredEvaluationNode value object
//! │   ├── rubric.rs         # Rubric value object + RubricSource enum
//! │   ├── result.rs         # ScoringResult + ScoreDimension value objects
//! │   ├── backend.rs        # ScoringBackend trait (domain interface)
//! │   ├── event.rs          # ScoredEvaluationEvent domain events
//! │   └── error.rs          # ScoredEvaluationError (thiserror)
//! ├── application/          # Service traits, DTOs
//! │   ├── service.rs        # ScoredEvaluationService trait
//! │   └── dto.rs            # EvaluateInput, EvaluateOutput DTOs
//! ├── infrastructure/       # Backend adapters and repository
//! │   ├── backends/         # Scoring backend implementations
//! │   │   ├── mcp_backend.rs    # MCP protocol adapter
//! │   │   ├── http_backend.rs   # HTTP REST adapter
//! │   │   └── local_backend.rs  # Local script adapter
//! │   └── repository.rs     # EvaluationRepository trait
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
