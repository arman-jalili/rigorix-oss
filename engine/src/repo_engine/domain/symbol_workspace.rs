//! SymbolWorkspaceIntent — describes how a task interacts with the symbol graph.
//!
//! @canonical .pi/architecture/modules/repo-engine.md#workspace-intent
//! Implements: Contract Freeze — SymbolWorkspaceIntent enum
//! Issue: #138
//!
//! Each `TaskNode` carries a `SymbolWorkspaceIntent` that describes how it
//! interacts with the symbol graph. Used for pre-execution validation (Phase 3)
//! to ensure that operations are consistent with the current graph state.
//!
//! # Contract (Frozen)
//! - Every task node MUST declare its workspace intent
//! - Phase 3 validation checks intent against graph state
//! - ReadOnly operations never modify the graph
//! - ReadWrite operations may add new symbols
//! - Modification operations change existing symbols
//! - Deletion operations remove symbols

use serde::{Deserialize, Serialize};

/// Describes how a task node interacts with the symbol graph.
///
/// Used by Phase 3 (pre-execution validation) to:
/// - Verify that symbols exist before Modification/Deletion
/// - Prevent write-after-read conflicts
/// - Schedule exclusive access for write operations
///
/// # Contract (Frozen)
/// - `ReadOnly` — The task only reads symbols (file_read, lsp_query)
/// - `ReadWrite` — The task reads and may add new symbols (file_write new file)
/// - `Modification` — The task modifies existing symbols (file_patch, file_write overwrite)
/// - `Deletion` — The task removes symbols (file_write empty, git_commit delete)
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SymbolWorkspaceIntent {
    /// The task only reads symbols from the graph.
    ///
    /// Examples: `file_read`, `lsp_query` (goto-definition, find-references).
    /// These operations never modify the graph and can run concurrently.
    ReadOnly,

    /// The task reads the graph and may add new symbol definitions.
    ///
    /// Examples: `file_write` for a new file that creates new symbols.
    /// The task may add symbols but must not modify or delete existing ones.
    ReadWrite,

    /// The task modifies existing symbol definitions in the graph.
    ///
    /// Examples: `file_patch`, `file_write` overwriting an existing file.
    /// The target symbols must exist in the graph before execution.
    Modification,

    /// The task removes symbols from the graph.
    ///
    /// Examples: `file_write` with empty content, `git_commit` deleting a file.
    /// The target symbols must exist in the graph before execution.
    Deletion,
}

impl SymbolWorkspaceIntent {
    /// Check if this intent allows reading from the symbol graph.
    pub fn allows_read(&self) -> bool {
        matches!(
            self,
            SymbolWorkspaceIntent::ReadOnly
                | SymbolWorkspaceIntent::ReadWrite
                | SymbolWorkspaceIntent::Modification
        )
    }

    /// Check if this intent allows writing to the symbol graph.
    pub fn allows_write(&self) -> bool {
        matches!(
            self,
            SymbolWorkspaceIntent::ReadWrite
                | SymbolWorkspaceIntent::Modification
                | SymbolWorkspaceIntent::Deletion
        )
    }

    /// Check if this intent requires existing symbols in the graph.
    ///
    /// Returns `true` for `Modification` and `Deletion` — these operations
    /// require the target symbols to already exist.
    pub fn requires_existing_symbols(&self) -> bool {
        matches!(
            self,
            SymbolWorkspaceIntent::Modification | SymbolWorkspaceIntent::Deletion
        )
    }

    /// Check if this intent may add new symbols to the graph.
    pub fn may_add_symbols(&self) -> bool {
        matches!(self, SymbolWorkspaceIntent::ReadWrite)
    }

    /// Get a human-readable description of this intent.
    pub fn description(&self) -> &'static str {
        match self {
            SymbolWorkspaceIntent::ReadOnly => {
                "Reads symbols from the graph without modification"
            }
            SymbolWorkspaceIntent::ReadWrite => {
                "Reads and may add new symbols to the graph"
            }
            SymbolWorkspaceIntent::Modification => {
                "Modifies existing symbol definitions in the graph"
            }
            SymbolWorkspaceIntent::Deletion => {
                "Removes symbols from the graph"
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SymbolWorkspaceIntent Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_readonly_allows_read_not_write() {
        let intent = SymbolWorkspaceIntent::ReadOnly;
        assert!(intent.allows_read());
        assert!(!intent.allows_write());
        assert!(!intent.requires_existing_symbols());
        assert!(!intent.may_add_symbols());
    }

    #[test]
    fn test_readwrite_allows_read_and_write() {
        let intent = SymbolWorkspaceIntent::ReadWrite;
        assert!(intent.allows_read());
        assert!(intent.allows_write());
        assert!(!intent.requires_existing_symbols());
        assert!(intent.may_add_symbols());
    }

    #[test]
    fn test_modification_allows_read_write_and_requires_existing() {
        let intent = SymbolWorkspaceIntent::Modification;
        assert!(intent.allows_read());
        assert!(intent.allows_write());
        assert!(intent.requires_existing_symbols());
        assert!(!intent.may_add_symbols());
    }

    #[test]
    fn test_deletion_allows_write_requires_existing() {
        let intent = SymbolWorkspaceIntent::Deletion;
        assert!(!intent.allows_read());
        assert!(intent.allows_write());
        assert!(intent.requires_existing_symbols());
        assert!(!intent.may_add_symbols());
    }

    #[test]
    fn test_descriptions_are_not_empty() {
        let cases = vec![
            SymbolWorkspaceIntent::ReadOnly,
            SymbolWorkspaceIntent::ReadWrite,
            SymbolWorkspaceIntent::Modification,
            SymbolWorkspaceIntent::Deletion,
        ];
        for intent in cases {
            assert!(!intent.description().is_empty(), "Description for {:?} should not be empty", intent);
        }
    }

    #[test]
    fn test_equality_and_hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SymbolWorkspaceIntent::ReadOnly);
        set.insert(SymbolWorkspaceIntent::ReadWrite);
        set.insert(SymbolWorkspaceIntent::Modification);
        set.insert(SymbolWorkspaceIntent::Deletion);
        set.insert(SymbolWorkspaceIntent::ReadOnly); // duplicate

        assert_eq!(set.len(), 4);
    }

    #[test]
    fn test_serialization_roundtrip() {
        let cases = vec![
            SymbolWorkspaceIntent::ReadOnly,
            SymbolWorkspaceIntent::ReadWrite,
            SymbolWorkspaceIntent::Modification,
            SymbolWorkspaceIntent::Deletion,
        ];
        for intent in &cases {
            let json = serde_json::to_string(intent).unwrap();
            let deserialized: SymbolWorkspaceIntent = serde_json::from_str(&json).unwrap();
            assert_eq!(*intent, deserialized);
        }
    }
}
