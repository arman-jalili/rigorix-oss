//! Command palette — fuzzy-find `/commands`.
//!
//! @canonical .pi/architecture/modules/tui.md#command-palette
//! Implements: Contract Freeze — command palette
//! Issue: issue-tui-contract-freeze
//!
//! # Contract (Frozen)
//!
//! The command palette provides fuzzy-find autocomplete for `/commands`.
//! Users type `/` followed by a search term, and the palette suggests
//! matching commands.

/// Available slash commands.
pub const SLASH_COMMANDS: &[(&str, &str)] = &[
    ("/history", "Browse past executions"),
    ("/templates", "List available templates"),
    ("/generate", "Generate a new template from intent"),
    ("/audit", "Browse audit trails"),
    ("/help", "Show help overlay"),
    ("/search", "Filter event log"),
    ("/cancel", "Cancel running execution"),
];

/// Fuzzy-match a query against available slash commands.
pub fn fuzzy_search(query: &str) -> Vec<(&'static str, &'static str)> {
    let lower = query.to_lowercase();
    SLASH_COMMANDS
        .iter()
        .filter(|(cmd, desc)| cmd.contains(&lower) || desc.to_lowercase().contains(&lower))
        .copied()
        .collect()
}
