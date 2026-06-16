//! CLI command handlers — one module per command group.
//!
//! Each module implements a CLI command handler that wraps the relevant
//! engine service. Handlers are initialized with engine dependencies
//! and provide a clean API for main.rs to call.

pub mod template_cmd;
