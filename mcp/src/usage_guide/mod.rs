//! Usage Guide — Self-documenting MCP tool for Claude context.
//!
//! @canonical .pi/architecture/modules/usage-guide.md
//!
//! Provides a single `rigorix_get_usage_guide` tool that returns structured
//! context about valid action types, intent formats, workflow patterns,
//! and plan JSON structure. Claude can call this at runtime to understand
//! how to use the rigorix tool system correctly.

pub mod interfaces;
