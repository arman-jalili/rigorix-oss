//! Interfaces layer for the Plan Validation bounded context.
//!
//! @canonical .pi/architecture/modules/plan-validation.md
//! Implements: Contract Freeze — HTTP endpoint contracts
//! Issue: issue-contract-freeze
//!
//! The interfaces layer defines the API surface that external consumers
//! interact with. Currently provides HTTP REST contracts.
//!
//! # Architecture
//!
//! ```text
//! interfaces/
//! ├── mod.rs                   # Module root
//! └── http/                    # HTTP API contracts
//!     └── mod.rs               # Endpoint paths, methods, request/response schemas
//! ```

pub mod http;
