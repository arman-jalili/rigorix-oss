//! Failure Parser bounded context.
//!
//! @canonical .pi/architecture/modules/failure-parser.md
//! Implements: Contract Freeze — failure-parser
//! Issue: #495 (#494 epic)
//!
//! This module parses raw compiler, test runner, and linter output into
//! structured, typed `TemplateFailure` values. Each failure carries precise
//! location context (file, line, column), a machine-readable error code, and
//! a suggested fix derived from the available source code context.
//!
//! This is the bridge between "something failed" and "here's exactly what to
//! change." The parser enables the validation loop to feed actionable feedback
//! back to the LLM for self-correction.
//!
//! # Architecture
//!
//! ```text
//! failure_parser/
//! ├── domain/                # Domain entities (TemplateFailure, FailureDetail, etc.), errors, events
//! │   ├── failure.rs         # TemplateFailure enum (6 categories)
//! │   ├── detail.rs          # FailureDetail with location, suggestion, severity
//! │   ├── input.rs           # CompilerOutput — raw stdout/stderr wrapper
//! │   ├── output.rs          # ParsedFailure, SourceContext
//! │   ├── error.rs           # FailureParserError enum
//! │   ├── registry.rs        # LanguageParser trait + ParserRegistry
//! │   └── event/             # FailureParserEvent payload schemas
//! ├── application/           # Service traits, DTOs, factory interfaces
//! │   ├── service.rs         # FailureParserService, FixSuggestionService traits
//! │   ├── factory.rs         # ParserFactory, FailureParserServiceFactory, FixSuggestionServiceFactory traits
//! │   └── dto/               # Input/Output DTOs with validation docs
//! ├── infrastructure/        # Repository interfaces
//! │   └── repository/        # ParserConfigRepository, FailureLogRepository traits
//! └── interfaces/            # API contracts
//!     └── http/              # REST endpoint contracts with request/response schemas
//! ```
//!
//! # Contract Freeze Notice
//!
//! ALL files in this module are frozen contracts.
//! - No implementation changes without explicit contract change approval
//! - Implementation PRs MUST reference these interfaces
//! - DTO schemas serve as the canonical data contract
//! - Tests verify contract assertions (serialization, naming, accessors)

pub mod application;
pub mod domain;
pub mod infrastructure;
pub mod interfaces;
