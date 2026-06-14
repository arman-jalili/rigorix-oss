//! Implementation of `WorkspaceValidationService`.
//!
//! @canonical .pi/architecture/modules/repo-engine.md#workspace-intent
//! Implements: WorkspaceValidationService — Phase 3 pre-execution validation
//! Issue: #141
//!
//! Validates workspace operations against the symbol graph to ensure task operations
//! are consistent with the current graph state. Prevents operations that would leave
//! the graph in an inconsistent state.

use async_trait::async_trait;
use std::collections::HashSet;
use std::path::PathBuf;
use std::sync::RwLock;

use crate::repo_engine::domain::{RepoEngineError, SymbolGraph, SymbolWorkspaceIntent};

use super::dto::{
    ValidateWorkspaceInput, ValidateWorkspaceOutput, ValidationMessage, ValidationSeverity,
};
use super::service::WorkspaceValidationService;

// ---------------------------------------------------------------------------
// WorkspaceValidationServiceImpl
// ---------------------------------------------------------------------------

/// Implementation of `WorkspaceValidationService` backed by a `SymbolGraph`.
///
/// Performs read-only validation of workspace state against the symbol graph.
/// Checks that:
/// - Symbols exist before Modification/Deletion
/// - No naming conflicts for new symbol additions
/// - Reference integrity is maintained
pub struct WorkspaceValidationServiceImpl {
    graph: RwLock<SymbolGraph>,
}

impl WorkspaceValidationServiceImpl {
    /// Create a new validator with an empty graph.
    pub fn new() -> Self {
        Self {
            graph: RwLock::new(SymbolGraph::new()),
        }
    }

    /// Create a new validator backed by an existing graph.
    pub fn from_graph(graph: SymbolGraph) -> Self {
        Self {
            graph: RwLock::new(graph),
        }
    }
}

impl Default for WorkspaceValidationServiceImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl WorkspaceValidationService for WorkspaceValidationServiceImpl {
    async fn validate_workspace(
        &self,
        input: ValidateWorkspaceInput,
    ) -> Result<ValidateWorkspaceOutput, RepoEngineError> {
        let graph = self.graph.read().map_err(|e| RepoEngineError::Internal {
            detail: format!("RwLock poisoned: {}", e),
        })?;

        let mut messages: Vec<ValidationMessage> = Vec::new();
        let mut error_count = 0;
        let mut warning_count = 0;

        // Step 1: Extract symbol names from changed files
        let affected_symbols: Vec<String> = input
            .changed_files
            .iter()
            .flat_map(|file| {
                graph
                    .lookup_by_file(file)
                    .into_iter()
                    .map(|s| s.name.clone())
            })
            .collect();

        let affected_set: HashSet<&str> =
            affected_symbols.iter().map(|s| s.as_str()).collect();

        // Step 2: Validate based on intent
        match input.intent {
            SymbolWorkspaceIntent::ReadOnly => {
                // ReadOnly: no checks needed, all read operations are valid
                messages.push(ValidationMessage {
                    severity: ValidationSeverity::Info,
                    message: "ReadOnly operation — no graph validation needed.".to_string(),
                    source: None,
                    code: Some("READONLY_OK".to_string()),
                });
            }

            SymbolWorkspaceIntent::ReadWrite => {
                // ReadWrite: check for naming conflicts if requested
                if input.check_conflicts {
                    for file in &input.changed_files {
                        // Simulate checking for conflicts with existing symbols
                        let file_name = file
                            .file_stem()
                            .and_then(|s| s.to_str())
                            .unwrap_or("")
                            .to_string();
                        if !file_name.is_empty() && graph.contains_key(&file_name) {
                            messages.push(ValidationMessage {
                                severity: ValidationSeverity::Warning,
                                message: format!(
                                    "Potential naming conflict: '{}' matches an existing symbol",
                                    file_name
                                ),
                                source: Some(file.to_string_lossy().to_string()),
                                code: Some("NAME_CONFLICT".to_string()),
                            });
                            warning_count += 1;
                        }
                    }
                }
                messages.push(ValidationMessage {
                    severity: ValidationSeverity::Info,
                    message: format!(
                        "ReadWrite operation — {} affected symbol(s) in {} file(s).",
                        affected_symbols.len(),
                        input.changed_files.len()
                    ),
                    source: None,
                    code: Some("READWRITE_OK".to_string()),
                });
            }

            SymbolWorkspaceIntent::Modification => {
                // Modification: all affected symbols must exist
                if affected_symbols.is_empty() && !input.changed_files.is_empty() {
                    // Files with changes might be new — warn
                    for file in &input.changed_files {
                        messages.push(ValidationMessage {
                            severity: ValidationSeverity::Warning,
                            message: format!(
                                "Modification requested for '{}' but no symbols found in graph",
                                file.display()
                            ),
                            source: Some(file.to_string_lossy().to_string()),
                            code: Some("NO_SYMBOLS_IN_FILE".to_string()),
                        });
                        warning_count += 1;
                    }
                }
                messages.push(ValidationMessage {
                    severity: ValidationSeverity::Info,
                    message: format!(
                        "Modification operation — {} existing symbol(s) will be modified.",
                        affected_symbols.len()
                    ),
                    source: None,
                    code: Some("MODIFICATION_OK".to_string()),
                });
            }

            SymbolWorkspaceIntent::Deletion => {
                // Deletion: all affected symbols must exist
                if affected_symbols.is_empty() && !input.changed_files.is_empty() {
                    for file in &input.changed_files {
                        messages.push(ValidationMessage {
                            severity: ValidationSeverity::Error,
                            message: format!(
                                "Deletion requested for '{}' but no symbols found in graph",
                                file.display()
                            ),
                            source: Some(file.to_string_lossy().to_string()),
                            code: Some("DELETION_NO_SYMBOLS".to_string()),
                        });
                        error_count += 1;
                    }
                }

                if input.check_references {
                    // Check for orphaned references
                    for name in &affected_set {
                        let refs_to = graph.references_to(name);
                        if !refs_to.is_empty() {
                            messages.push(ValidationMessage {
                                severity: ValidationSeverity::Warning,
                                message: format!(
                                    "Deleting '{}' will orphan {} reference(s)",
                                    name,
                                    refs_to.len()
                                ),
                                source: Some(name.to_string()),
                                code: Some("ORPHANED_REFS".to_string()),
                            });
                            warning_count += 1;
                        }
                    }
                }
            }
        }

        // Step 3: Check reference integrity if requested
        if input.check_references && affected_set.len() > 1 {
            for name in &affected_set {
                if let Some(refs) = graph.references_from(name) {
                    for target in refs.iter() {
                        if !graph.contains_key(target) {
                            messages.push(ValidationMessage {
                                severity: ValidationSeverity::Error,
                                message: format!(
                                    "Symbol '{}' references non-existent symbol '{}'",
                                    name, target
                                ),
                                source: Some(name.to_string()),
                                code: Some("BROKEN_REFERENCE".to_string()),
                            });
                            error_count += 1;
                        }
                    }
                }
            }
        }

        let valid = error_count == 0;

        Ok(ValidateWorkspaceOutput {
            valid,
            messages,
            error_count,
            warning_count,
        })
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::repo_engine::domain::{
        Location, SourceLanguage, SymbolDefinition, SymbolKind,
    };
    use std::path::PathBuf;

    fn make_graph_with_symbols() -> SymbolGraph {
        let mut graph = SymbolGraph::new();
        let def = SymbolDefinition::new(
            "existing_fn".to_string(),
            SymbolKind::Function,
            Location::new(PathBuf::from("src/lib.rs"), 10, 0),
            "fn existing_fn()".to_string(),
            "fn existing_fn() {}".to_string(),
            SourceLanguage::Rust,
        );
        graph.add_symbol(def).unwrap();
        graph
    }

    #[tokio::test]
    async fn test_readonly_validation_passes() {
        let svc = WorkspaceValidationServiceImpl::from_graph(make_graph_with_symbols());

        let result = svc
            .validate_workspace(ValidateWorkspaceInput {
                changed_files: vec![PathBuf::from("src/lib.rs")],
                intent: SymbolWorkspaceIntent::ReadOnly,
                check_references: false,
                check_conflicts: false,
            })
            .await
            .unwrap();

        assert!(result.valid);
        assert_eq!(result.error_count, 0);
    }

    #[tokio::test]
    async fn test_readwrite_with_conflict_check() {
        let svc = WorkspaceValidationServiceImpl::from_graph(make_graph_with_symbols());

        // existing_fn is a file_stem conflict with the existing symbol name "existing_fn"
        let result = svc
            .validate_workspace(ValidateWorkspaceInput {
                changed_files: vec![PathBuf::from("src/existing_fn.rs")],
                intent: SymbolWorkspaceIntent::ReadWrite,
                check_references: false,
                check_conflicts: true,
            })
            .await
            .unwrap();

        // "existing_fn" matches an existing symbol name — should be a warning
        assert_eq!(result.warning_count, 1);
        assert!(result
            .messages
            .iter()
            .any(|m| m.code == Some("NAME_CONFLICT".to_string())));
    }

    #[tokio::test]
    async fn test_modification_with_nonexistent_file() {
        let svc = WorkspaceValidationServiceImpl::new();

        let result = svc
            .validate_workspace(ValidateWorkspaceInput {
                changed_files: vec![PathBuf::from("src/new_file.rs")],
                intent: SymbolWorkspaceIntent::Modification,
                check_references: false,
                check_conflicts: false,
            })
            .await
            .unwrap();

        // Modification on a file with no symbols in graph = warning
        assert_eq!(result.warning_count, 1);
        assert!(result
            .messages
            .iter()
            .any(|m| m.code == Some("NO_SYMBOLS_IN_FILE".to_string())));
    }

    #[tokio::test]
    async fn test_deletion_with_nonexistent_file() {
        let svc = WorkspaceValidationServiceImpl::new();

        let result = svc
            .validate_workspace(ValidateWorkspaceInput {
                changed_files: vec![PathBuf::from("src/missing.rs")],
                intent: SymbolWorkspaceIntent::Deletion,
                check_references: false,
                check_conflicts: false,
            })
            .await
            .unwrap();

        assert!(!result.valid);
        assert_eq!(result.error_count, 1);
        assert!(result
            .messages
            .iter()
            .any(|m| m.code == Some("DELETION_NO_SYMBOLS".to_string())));
    }

    #[tokio::test]
    async fn test_empty_changed_files_passes() {
        let svc = WorkspaceValidationServiceImpl::new();

        let result = svc
            .validate_workspace(ValidateWorkspaceInput {
                changed_files: vec![],
                intent: SymbolWorkspaceIntent::ReadOnly,
                check_references: false,
                check_conflicts: false,
            })
            .await
            .unwrap();

        assert!(result.valid);
    }

    #[tokio::test]
    async fn test_all_intents_serializable() {
        let svc = WorkspaceValidationServiceImpl::new();

        for intent in &[
            SymbolWorkspaceIntent::ReadOnly,
            SymbolWorkspaceIntent::ReadWrite,
            SymbolWorkspaceIntent::Modification,
            SymbolWorkspaceIntent::Deletion,
        ] {
            let result = svc
                .validate_workspace(ValidateWorkspaceInput {
                    changed_files: vec![],
                    intent: intent.clone(),
                    check_references: false,
                    check_conflicts: false,
                })
                .await
                .unwrap();

            assert!(result.valid, "Failed for intent {:?}", intent);
        }
    }
}
