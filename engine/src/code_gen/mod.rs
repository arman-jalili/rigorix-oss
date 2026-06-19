//! Code Generation Pipeline bounded context.
//!
//! @canonical .pi/architecture/modules/code-generation.md
//! Implements: Contract Freeze — code_gen module root
//! Issue: #424
//!
//! Converts LLM-generated code into correctly-positioned file edits.
//! Provides three tiers of tooling — read_file (context gathering),
//! edit_file (targeted string replacement), and write_file (full file
//! replacement) — plus a post-edit syntax verification gate powered by
//! Rigorix's existing tree-sitter integration.
//!
//! The core innovation is the exact-string anchor pattern: the LLM quotes
//! the precise text it wants to replace (old_string), and the engine
//! refuses the edit if that text does not exist in the file.
//!
//! # Architecture
//!
//! ```text
//! code_gen/
//! ├── domain/           # Domain entities (SyntaxGateResult, SyntaxError, CodeGenError)
//! │   ├── error.rs      # CodeGenError enum
//! │   ├── event.rs      # CodeGenEvent payload schemas
//! │   └── result.rs     # SyntaxGateResult enum, SyntaxError struct
//! ├── application/      # Service traits, DTOs, factory interfaces
//! │   ├── service.rs    # SyntaxGateService trait
//! │   └── dto/          # Input/Output DTOs (EditFileInput, EditFileResult, etc.)
//! ├── infrastructure/   # Repository interfaces
//! │   └── repository/   # Repository interfaces
//! └── interfaces/       # API contracts
//!     └── http/         # REST endpoint contracts
//! ```
//!
//! # Related Tools (in tools module)
//!
//! | Component | Location | Purpose |
//! |-----------|----------|---------|
//! | EditFileTool | tools/file_edit.rs | Exact-string replacement Tool impl |
//! | ReadFileTool | tools/file_read.rs | File reading with offset/limit |
//! | WriteFileTool | tools/file_write.rs | Full file write with atomic rename |
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
