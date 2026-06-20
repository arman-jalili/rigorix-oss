//! Policy Evaluator — Mode A reactive governance bounded context.
//!
//! @canonical actions/.pi/architecture/modules/policy-evaluator.md
//! Implements: Contract Freeze — all component interfaces for policy-evaluator epic
//! Issue: issue-contract-freeze
//!
//! Mode A of the Rigorix GitHub Action. The Policy Evaluator checks Pull Request
//! diffs against a configurable policy file (`.rigorix/policy.toml`) and classifies
//! violations into three categories: deny (blocks the PR), require_review (flags for
//! human review), and flag (warns without blocking). Policies are loaded from the
//! **base branch** (not the PR) to prevent tampering.
//!
//! This is the governance layer — it checks code **after** it's written,
//! complementing Mode B which **generates** code.
//!
//! # Components
//!
//! | Component | Domain | Application | Infrastructure | Interfaces |
//! |-----------|--------|-------------|----------------|------------|
//! | PolicyDocument | `domain::types::PolicyDocument` | `application::dto` | — | `interfaces::http` |
//! | PolicyRule Types | `domain::types::DenyRule`, `ReviewRule`, `FlagRule` | — | — | — |
//! | PolicyLoader | — | `application::service::PolicyLoadingService` | `infrastructure::repository::PolicyRepository` | — |
//! | PolicyEvaluator | — | `application::service::PolicyEvaluationService` | — | — |
//! | OrgPolicyMerger | — | `application::service::OrgPolicyMergingService` | `infrastructure::repository::OrgPolicyRepository` | — |
//!
//! # Layer Structure
//!
//! ```text
//! policy_evaluator/
//! ├── mod.rs                          # Module root
//! ├── domain/                         # Domain entities and interfaces
//! │   ├── mod.rs
//! │   ├── types.rs                    # PolicyDocument, PolicyRules, DenyRule, ReviewRule, FlagRule, Severity, PolicyViolation, PolicyResult, etc.
//! │   ├── error.rs                    # PolicyError
//! │   └── event/
//! │       └── mod.rs                  # PolicyEvent payloads
//! ├── application/                    # Application service interfaces and DTOs
//! │   ├── mod.rs
//! │   ├── service.rs                  # Service traits (PolicyLoadingService, PolicyEvaluationService, OrgPolicyMergingService, PolicyTamperDetectionService)
//! │   ├── dto/
//! │   │   └── mod.rs                  # Input/output DTO schemas
//! │   └── factory.rs                  # Factory interfaces
//! ├── infrastructure/                 # Infrastructure layer
//! │   ├── mod.rs
//! │   └── repository/
//! │       └── mod.rs                  # Repository interfaces (PolicyRepository, OrgPolicyRepository)
//! └── interfaces/                     # External interfaces
//!     ├── mod.rs
//!     └── http/
//!         └── mod.rs                  # HTTP API contracts
//! ```
//!
//! # Contract Freeze
//!
//! All public interfaces, DTO schemas, and contracts in this module are
//! frozen. Implementation must satisfy these contracts, not the other way around.
//! See `actions/.pi/architecture/modules/policy-evaluator.md` for the canonical spec.
//!
//! # Dependencies
//!
//! - **diff-analyzer**: `PrDiff` struct for changed file iteration
//! - **security-config**: Organization policy path for merging
//! - **GitHub API**: Reading base branch content (via `GitHubClient`)
//!
//! # Related Issues
//!
//! - Issue #564: Contract Freeze (this issue)
//! - Issue #564: Epic "policy-evaluator"

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
