//! Domain entities and interfaces for the Failure Parser bounded context.
//!
//! @canonical .pi/architecture/modules/failure-parser.md
//! Implements: Contract Freeze — TemplateFailure, FailureDetail, CompilerOutput,
//!              ParsedFailure, FailureParserError, FailureParserEvent, ParserRegistry
//! Issue: #495
//!
//! This module defines the core domain types — `TemplateFailure`, `FailureDetail`,
//! `CompilerOutput`, `ParsedFailure`, `FailureParserError`, and the `LanguageParser`
//! trait for parser registry. These are pure domain objects with no framework
//! dependencies. They serve as the frozen contract that all implementations
//! must satisfy.
//!
//! # Contract Freeze
//! - No implementation logic beyond enum variants, accessors, and constructors
//! - All parsing orchestration logic must happen in the application layer
//! - All persistence must happen behind repository interfaces
//! - The TemplateFailure ↔ error code mapping is the core domain invariant

pub mod detail;
pub mod error;
pub mod event;
pub mod failure;
pub mod input;
pub mod output;
pub mod registry;

pub use detail::*;
pub use error::FailureParserError;
pub use event::*;
pub use failure::*;
pub use input::*;
pub use output::*;
pub use registry::*;
