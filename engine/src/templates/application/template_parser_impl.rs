//! Implementation of `TemplateParserService`.
//!
//! @canonical .pi/architecture/modules/template-system.md#parser
//! Implements: TemplateParserService — TOML deserialization, schema validation, directory loading
//! Issue: issue-templateparser
//!
//! Handles parsing TOML template files into validated `Template` structs, directory scanning,
//! structural validation (cycle detection, dependency integrity), and built-in template loading.

use async_trait::async_trait;

use crate::templates::domain::{Template, TemplateError};
use crate::templates::infrastructure::repository::TemplateRepository;

use super::dto::{
    LoadBuiltinsInput, LoadBuiltinsOutput, LoadDirectoryOutput, ParseFileInput, ParseOutput,
    ParseStrInput, TemplateLoadFailure, ValidateTemplateInput, ValidateTemplateOutput,
    ValidationError, ValidationSeverity,
};
use super::service::TemplateParserService;

/// Default implementation of `TemplateParserService`.
///
/// Uses a `TemplateRepository` for file access and performs TOML deserialization
/// and structural validation.
pub struct TemplateParserImpl<R: TemplateRepository> {
    repository: R,
}

impl<R: TemplateRepository> TemplateParserImpl<R> {
    /// Create a new parser with the given repository backend.
    pub fn new(repository: R) -> Self {
        Self { repository }
    }
}

#[async_trait]
impl<R: TemplateRepository + Send + Sync> TemplateParserService for TemplateParserImpl<R> {
    #[tracing::instrument(skip_all)]
    async fn parse_file(&self, input: ParseFileInput) -> Result<ParseOutput, TemplateError> {
        let source = input.path.clone();
        let content = self.repository.read_template_file(&source).await?;
        let parse_input = ParseStrInput {
            toml_content: content,
            source: Some(source),
            validate: input.validate,
        };
        self.parse_str(parse_input).await
    }

    #[tracing::instrument(skip_all)]
    async fn parse_str(&self, input: ParseStrInput) -> Result<ParseOutput, TemplateError> {
        // Attempt TOML deserialization
        let template: Template = match toml::from_str(&input.toml_content) {
            Ok(t) => t,
            Err(e) => {
                return Ok(ParseOutput {
                    template: Template::default(),
                    valid: false,
                    errors: vec![format!("TOML parse error: {}", e)],
                    warnings: vec![],
                });
            }
        };

        let mut errors: Vec<String> = Vec::new();
        let mut warnings: Vec<String> = Vec::new();

        if input.validate {
            // Run structural validation
            let validation = validate_template_structure(&template);
            errors.extend(validation.errors.into_iter().map(|e| e.message));
            warnings.extend(validation.warnings);
        }

        let valid = errors.is_empty();

        Ok(ParseOutput {
            template,
            valid,
            errors,
            warnings,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn load_directory(&self, path: &str) -> Result<LoadDirectoryOutput, TemplateError> {
        let files = self.repository.list_template_files(path, "toml").await?;

        let mut templates = Vec::new();
        let mut failures = Vec::new();

        for file_path in &files {
            match self
                .parse_file(ParseFileInput {
                    path: file_path.clone(),
                    validate: true,
                })
                .await
            {
                Ok(output) if output.valid => {
                    templates.push(output.template);
                }
                Ok(output) => {
                    failures.push(TemplateLoadFailure {
                        path: file_path.clone(),
                        error: output.errors.join("; "),
                        line: None,
                    });
                }
                Err(e) => {
                    failures.push(TemplateLoadFailure {
                        path: file_path.clone(),
                        error: e.to_string(),
                        line: None,
                    });
                }
            }
        }

        Ok(LoadDirectoryOutput {
            total_files: files.len(),
            successful: templates.len(),
            templates,
            failures,
        })
    }

    async fn validate_template(
        &self,
        input: ValidateTemplateInput,
    ) -> Result<ValidateTemplateOutput, TemplateError> {
        let result = validate_template_structure(&input.template);
        let mut errors = result.errors;
        let mut warnings = result.warnings;

        if input.check_cycles {
            let cycle_result = detect_cycles(&input.template);
            for node_ids in &cycle_result {
                errors.push(ValidationError {
                    field: "nodes".to_string(),
                    message: format!("Cycle detected involving nodes: {:?}", node_ids),
                    value: Some(format!("{:?}", node_ids)),
                    severity: ValidationSeverity::Error,
                });
            }
        }

        if input.check_param_references {
            let param_result = validate_param_references(&input.template);
            errors.extend(param_result.errors);
            warnings.extend(param_result.warnings);
        }

        let valid = errors
            .iter()
            .all(|e| e.severity == ValidationSeverity::Warning);

        Ok(ValidateTemplateOutput {
            valid,
            errors,
            warnings,
            cycles_checked: input.check_cycles,
            params_checked: input.check_param_references,
        })
    }

    async fn load_builtins(
        &self,
        input: LoadBuiltinsInput,
    ) -> Result<LoadBuiltinsOutput, TemplateError> {
        let source_ids = self.repository.list_builtin_ids().await;

        let mut loaded = Vec::new();

        for id in &source_ids {
            // Apply category filter if specified
            if let Some(ref categories) = input.categories {
                if !categories.iter().any(|c| id.contains(c)) {
                    continue;
                }
            }

            if let Some(source) = self.repository.get_builtin_source(id).await {
                match self
                    .parse_str(ParseStrInput {
                        toml_content: source.to_string(),
                        source: Some(format!("builtin:{}", id)),
                        validate: true,
                    })
                    .await
                {
                    Ok(output) if output.valid => {
                        loaded.push(id.to_string());
                    }
                    _ => {
                        // Skip invalid builtins (shouldn't happen, but be defensive)
                    }
                }
            }
        }

        let count = loaded.len();
        Ok(LoadBuiltinsOutput { loaded, count })
    }
}

// ---------------------------------------------------------------------------
// Structural Validation
// ---------------------------------------------------------------------------

/// Result of a structural validation check.
struct ValidationResult {
    errors: Vec<ValidationError>,
    warnings: Vec<String>,
}

/// Validate the structure of a template definition.
///
/// Checks:
/// - Template has required metadata (id, name)
/// - Node IDs are unique
/// - Dependency references are valid
/// - Parameter definitions have names
#[tracing::instrument(skip_all)]
fn validate_template_structure(template: &Template) -> ValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // Check required fields
    if template.id.is_empty() {
        errors.push(ValidationError {
            field: "id".to_string(),
            message: "Template ID is required".to_string(),
            value: None,
            severity: ValidationSeverity::Error,
        });
    }

    if template.name.is_empty() {
        errors.push(ValidationError {
            field: "name".to_string(),
            message: "Template name is required".to_string(),
            value: None,
            severity: ValidationSeverity::Error,
        });
    }

    // Check for duplicate node IDs
    let mut seen_ids: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for node in &template.nodes {
        if !seen_ids.insert(&node.id) {
            errors.push(ValidationError {
                field: format!("nodes[{}]", node.id),
                message: format!("Duplicate node ID: {}", node.id),
                value: Some(node.id.clone()),
                severity: ValidationSeverity::Error,
            });
        }
    }

    // Check for duplicate parameter names
    let mut seen_params: std::collections::HashSet<&str> = std::collections::HashSet::new();
    for param in &template.parameters {
        if !seen_params.insert(&param.name) {
            errors.push(ValidationError {
                field: format!("parameters[{}]", param.name),
                message: format!("Duplicate parameter name: {}", param.name),
                value: Some(param.name.clone()),
                severity: ValidationSeverity::Error,
            });
        }
    }

    // Check dependency references exist
    let node_id_set: std::collections::HashSet<&str> =
        template.nodes.iter().map(|n| n.id.as_str()).collect();

    for node in &template.nodes {
        for dep in &node.depends_on {
            if !node_id_set.contains(dep.as_str()) {
                errors.push(ValidationError {
                    field: format!("nodes[{}].depends_on", node.id),
                    message: format!(
                        "Node '{}' depends on '{}' which does not exist",
                        node.id, dep
                    ),
                    value: Some(dep.clone()),
                    severity: ValidationSeverity::Error,
                });
            }
        }
    }

    // Warn if no nodes defined
    if template.nodes.is_empty() {
        warnings.push("Template has no nodes defined".to_string());
    }

    ValidationResult { errors, warnings }
}

/// Detect cycles in the template's node dependency graph using Kahn's algorithm.
///
/// Returns a list of cycles, where each cycle is a list of node IDs involved.
#[tracing::instrument(skip_all)]
fn detect_cycles(template: &Template) -> Vec<Vec<String>> {
    let node_count = template.nodes.len();
    if node_count == 0 {
        return vec![];
    }

    // Build adjacency and in-degree
    let mut in_degree: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    let mut adjacency: std::collections::HashMap<&str, Vec<&str>> =
        std::collections::HashMap::new();

    for node in &template.nodes {
        in_degree.entry(&node.id).or_insert(0);
        adjacency.entry(&node.id).or_default();
    }

    for node in &template.nodes {
        for dep in &node.depends_on {
            if let Some(edges) = adjacency.get_mut(dep.as_str()) {
                edges.push(&node.id);
                *in_degree.entry(&node.id).or_insert(0) += 1;
            }
        }
    }

    // Kahn's algorithm
    let mut queue: Vec<&str> = in_degree
        .iter()
        .filter(|(_, &deg)| deg == 0)
        .map(|(id, _)| *id)
        .collect();

    let mut sorted_count = 0;

    while let Some(node_id) = queue.pop() {
        sorted_count += 1;
        if let Some(neighbors) = adjacency.get(node_id) {
            for neighbor in neighbors {
                if let Some(deg) = in_degree.get_mut(neighbor) {
                    *deg -= 1;
                    if *deg == 0 {
                        queue.push(neighbor);
                    }
                }
            }
        }
    }

    if sorted_count == node_count {
        vec![] // No cycles
    } else {
        // Nodes with remaining in-degree are part of cycles
        let cycle_nodes: Vec<String> = in_degree
            .iter()
            .filter(|(_, &deg)| deg > 0)
            .map(|(id, _)| id.to_string())
            .collect();
        if cycle_nodes.is_empty() {
            vec![]
        } else {
            vec![cycle_nodes]
        }
    }
}

/// Validate that parameter references in node actions match parameter definitions.
#[tracing::instrument(skip_all)]
fn validate_param_references(template: &Template) -> ValidationResult {
    let errors = Vec::new();
    let mut warnings = Vec::new();

    // Collect defined parameter names
    let defined_params: std::collections::HashSet<&str> = template
        .parameters
        .iter()
        .map(|p| p.name.as_str())
        .collect();

    // Check each node's action for {{ param }} references
    for node in &template.nodes {
        let refs = node.action.referenced_params();
        for param_name in &refs {
            if !defined_params.contains(param_name.as_str()) {
                warnings.push(format!(
                    "Node '{}' references undefined parameter '{}'",
                    node.id, param_name
                ));
            }
        }
    }

    ValidationResult { errors, warnings }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::infrastructure::repository::InMemoryTemplateRepository;

    #[tracing::instrument(skip_all)]
    fn create_test_repo() -> InMemoryTemplateRepository {
        InMemoryTemplateRepository::new()
    }

    #[tracing::instrument(skip_all)]
    fn valid_template_toml() -> &'static str {
        r#"
id = "test-template"
name = "Test Template"
description = "A test template"
version = "1.0.0"

[[parameters]]
name = "target_file"
description = "File to modify"
required = true
param_type = "path"

[[nodes]]
id = "read-file"
name = "Read file"
depends_on = []
[nodes.action]
type = "file_read"
path = "{{ target_file }}"

[[nodes]]
id = "write-file"
name = "Write file"
depends_on = ["read-file"]
[nodes.action]
type = "file_write"
path = "{{ target_file }}"
content = "updated content"
"#
    }

    #[tokio::test]
    async fn test_parse_valid_toml() {
        let repo = create_test_repo();
        let parser = TemplateParserImpl::new(repo);

        let output = parser
            .parse_str(ParseStrInput {
                toml_content: valid_template_toml().to_string(),
                source: None,
                validate: true,
            })
            .await
            .unwrap();

        assert!(
            output.valid,
            "Expected valid template, got errors: {:?}",
            output.errors
        );
        assert_eq!(output.template.id, "test-template");
        assert_eq!(output.template.nodes.len(), 2);
        assert_eq!(output.template.parameters.len(), 1);
    }

    #[tokio::test]
    async fn test_parse_invalid_toml() {
        let repo = create_test_repo();
        let parser = TemplateParserImpl::new(repo);

        let output = parser
            .parse_str(ParseStrInput {
                toml_content: "invalid toml content {{{".to_string(),
                source: None,
                validate: true,
            })
            .await
            .unwrap();

        assert!(!output.valid);
        assert!(!output.errors.is_empty());
    }

    #[tokio::test]
    async fn test_parse_minimal_template() {
        let repo = create_test_repo();
        let parser = TemplateParserImpl::new(repo);

        let toml = r#"
id = "minimal"
name = "Minimal"
description = "Minimal template"
version = "0.1.0"
"#;

        let output = parser
            .parse_str(ParseStrInput {
                toml_content: toml.to_string(),
                source: None,
                validate: true,
            })
            .await
            .unwrap();

        assert!(output.valid);
        assert_eq!(output.template.id, "minimal");
    }

    #[tokio::test]
    async fn test_parse_missing_id() {
        let repo = create_test_repo();
        let parser = TemplateParserImpl::new(repo);

        let toml = r#"
name = "No ID"
description = "Missing ID template"
version = "1.0.0"
"#;

        let output = parser
            .parse_str(ParseStrInput {
                toml_content: toml.to_string(),
                source: None,
                validate: true,
            })
            .await
            .unwrap();

        assert!(!output.valid, "Expected invalid, got valid");
        assert!(
            output.errors.iter().any(|e| e.contains("missing field")),
            "Expected missing field error, got: {:?}",
            output.errors
        );
    }

    #[tokio::test]
    async fn test_parse_duplicate_node_ids() {
        let repo = create_test_repo();
        let parser = TemplateParserImpl::new(repo);

        let toml = r#"
id = "dup-nodes"
name = "Duplicate Nodes"
description = "Has duplicate node IDs"
version = "1.0.0"

[[nodes]]
id = "same-id"
name = "First"
[nodes.action]
type = "file_read"
path = "file1.txt"

[[nodes]]
id = "same-id"
name = "Second"
[nodes.action]
type = "file_read"
path = "file2.txt"
"#;

        let output = parser
            .parse_str(ParseStrInput {
                toml_content: toml.to_string(),
                source: None,
                validate: true,
            })
            .await
            .unwrap();

        assert!(!output.valid);
        assert!(output.errors.iter().any(|e| e.contains("Duplicate")));
    }

    #[tokio::test]
    async fn test_parse_missing_dependency() {
        let repo = create_test_repo();
        let parser = TemplateParserImpl::new(repo);

        let toml = r#"
id = "missing-dep"
name = "Missing Dependency"
description = "Has a dependency that doesn't exist"
version = "1.0.0"

[[nodes]]
id = "node-b"
name = "Node B"
depends_on = ["node-a"]
[nodes.action]
type = "file_read"
path = "test.txt"
"#;

        let output = parser
            .parse_str(ParseStrInput {
                toml_content: toml.to_string(),
                source: None,
                validate: true,
            })
            .await
            .unwrap();

        assert!(!output.valid);
        assert!(output.errors.iter().any(|e| e.contains("node-a")));
    }

    #[tokio::test]
    async fn test_cycle_detection() {
        let repo = create_test_repo();
        let parser = TemplateParserImpl::new(repo);

        let toml = r#"
id = "cycle-template"
name = "Cycle Test"
description = "Has a cycle"
version = "1.0.0"

[[nodes]]
id = "node-a"
name = "Node A"
depends_on = ["node-b"]
[nodes.action]
type = "file_read"
path = "a.txt"

[[nodes]]
id = "node-b"
name = "Node B"
depends_on = ["node-a"]
[nodes.action]
type = "file_read"
path = "b.txt"
"#;

        let validation = parser
            .validate_template(ValidateTemplateInput {
                template: toml::from_str(toml).unwrap(),
                check_cycles: true,
                check_param_references: false,
            })
            .await
            .unwrap();

        assert!(!validation.valid);
        assert!(validation
            .errors
            .iter()
            .any(|e| e.message.contains("Cycle")));
    }

    #[tokio::test]
    async fn test_load_directory_empty() {
        let repo = create_test_repo();
        let parser = TemplateParserImpl::new(repo);

        let output = parser.load_directory("/nonexistent/path").await.unwrap();
        assert_eq!(output.total_files, 0);
        assert_eq!(output.successful, 0);
    }

    #[tokio::test]
    async fn test_validate_param_references() {
        let repo = create_test_repo();
        let parser = TemplateParserImpl::new(repo);

        let toml = r#"
id = "param-ref"
name = "Param Reference"
description = "Has undefined param reference"
version = "1.0.0"

[[nodes]]
id = "read-file"
name = "Read file"
[nodes.action]
type = "file_read"
path = "{{ undefined_param }}"
"#;

        let validation = parser
            .validate_template(ValidateTemplateInput {
                template: toml::from_str(toml).unwrap(),
                check_cycles: false,
                check_param_references: true,
            })
            .await
            .unwrap();

        assert!(validation.valid); // Warning, not error
        assert!(validation
            .warnings
            .iter()
            .any(|w| w.contains("undefined_param")));
    }

    #[tokio::test]
    async fn test_parse_file_with_repository() {
        let mut repo = create_test_repo();
        let toml = valid_template_toml();
        repo.add_source("test_template.toml".to_string(), toml.to_string());

        let parser = TemplateParserImpl::new(repo);

        let output = parser
            .parse_file(ParseFileInput {
                path: "test_template.toml".to_string(),
                validate: true,
            })
            .await
            .unwrap();

        assert!(output.valid);
        assert_eq!(output.template.id, "test-template");
    }

    #[tokio::test]
    async fn test_load_builtins() {
        let mut repo = create_test_repo();
        repo.add_builtin(
            "read-file",
            r#"
id = "read-file"
name = "Read File"
description = "Read a file from disk"
version = "1.0.0"

[[parameters]]
name = "path"
description = "File path"
required = true
param_type = "path"

[[nodes]]
id = "read"
name = "Read"
[nodes.action]
type = "file_read"
path = "{{ path }}"
"#,
        );

        let parser = TemplateParserImpl::new(repo);

        let output = parser
            .load_builtins(LoadBuiltinsInput {
                categories: None,
                overwrite: false,
            })
            .await
            .unwrap();

        assert_eq!(output.count, 1);
        assert_eq!(output.loaded[0], "read-file");
    }
}
