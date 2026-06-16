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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_fuzzy_search_exact_cmd() {
        let results = fuzzy_search("/history");
        assert!(!results.is_empty());
        assert!(results.iter().any(|(cmd, _)| *cmd == "/history"));
    }

    #[test]
    fn test_fuzzy_search_partial_cmd() {
        let results = fuzzy_search("hist");
        assert!(!results.is_empty());
        assert!(results.iter().any(|(cmd, _)| *cmd == "/history"));
    }

    #[test]
    fn test_fuzzy_search_partial_desc() {
        let results = fuzzy_search("audit");
        assert!(!results.is_empty());
        assert!(results.iter().any(|(cmd, _)| *cmd == "/audit"));
    }

    #[test]
    fn test_fuzzy_search_no_match() {
        let results = fuzzy_search("zzzznotfound");
        assert!(results.is_empty());
    }

    #[test]
    fn test_fuzzy_search_empty_query() {
        let results = fuzzy_search("");
        assert_eq!(results.len(), SLASH_COMMANDS.len());
    }

    #[test]
    fn test_fuzzy_search_case_insensitive() {
        let results = fuzzy_search("HISTORY");
        assert!(!results.is_empty());
        assert!(results.iter().any(|(cmd, _)| *cmd == "/history"));
    }

    #[test]
    fn test_slash_commands_not_empty() {
        assert!(!SLASH_COMMANDS.is_empty());
    }

    #[test]
    fn test_every_command_has_description() {
        for (cmd, desc) in SLASH_COMMANDS {
            assert!(!cmd.is_empty(), "Command should not be empty");
            assert!(
                !desc.is_empty(),
                "Description for {cmd} should not be empty"
            );
        }
    }
}
