//! Implementation of `TemplateEngineService`.
//!
//! @canonical .pi/architecture/modules/template-system.md#engine
//! Implements: TemplateEngineService — runtime registry, lookup, graph generation
//! Issue: issue-templateengine
//!
//! Manages the template registry (register, lookup, list) and generates
//! executable graph outputs from registered templates with parameter substitution.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::RwLock;

use crate::templates::domain::{Template, TemplateError};

use super::dto::{
    GenerateInput, GenerateOutput, GeneratedNode, GetTemplateInput, ListTemplatesOutput,
    RegisterInput, RegisterOutput, TemplateSummary,
};
use super::service::TemplateEngineService;

/// Default implementation of `TemplateEngineService`.
///
/// Maintains an in-memory registry of templates and provides graph generation
/// with parameter substitution and cycle detection.
pub struct TemplateEngineImpl {
    /// Registered templates keyed by ID.
    templates: RwLock<HashMap<String, RegisteredEntry>>,
}

struct RegisteredEntry {
    template: Template,
    is_builtin: bool,
}

impl TemplateEngineImpl {
    /// Create a new empty template engine.
    pub fn new() -> Self {
        Self {
            templates: RwLock::new(HashMap::new()),
        }
    }

    /// Register a template directly (useful for tests and builtins).
    pub fn register_direct(
        &self,
        template: Template,
        is_builtin: bool,
    ) -> Result<(), TemplateError> {
        let mut templates = self.templates.write().expect("lock poisoned");
        let id = template.id.clone();
        if templates.contains_key(&id) {
            return Err(TemplateError::DuplicateTemplate { id });
        }
        templates.insert(
            id,
            RegisteredEntry {
                template,
                is_builtin,
            },
        );
        Ok(())
    }
}

impl Default for TemplateEngineImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl TemplateEngineService for TemplateEngineImpl {
    #[tracing::instrument(skip_all)]
    async fn register(&self, input: RegisterInput) -> Result<RegisterOutput, TemplateError> {
        let mut templates = self.templates.write().expect("lock poisoned");
        let id = input.template.id.clone();

        let overwritten = if templates.contains_key(&id) {
            if input.overwrite {
                true
            } else {
                return Err(TemplateError::DuplicateTemplate { id });
            }
        } else {
            false
        };

        templates.insert(
            id.clone(),
            RegisteredEntry {
                template: input.template,
                is_builtin: false,
            },
        );

        Ok(RegisterOutput {
            template_id: id,
            total_templates: templates.len(),
            overwritten,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn generate(&self, input: GenerateInput) -> Result<GenerateOutput, TemplateError> {
        let templates = self.templates.read().expect("lock poisoned");
        let entry = templates.get(&input.template_id).ok_or_else(|| {
            let available: Vec<String> = templates.keys().cloned().collect();
            TemplateError::NotFound {
                id: input.template_id.clone(),
                available,
            }
        })?;

        let template = &entry.template;

        // Validate required parameters
        for param in &template.parameters {
            if param.required && !input.params.contains_key(&param.name) {
                return Err(TemplateError::MissingParameter {
                    template: template.id.clone(),
                    param: param.name.clone(),
                    description: Some(param.description.clone()),
                });
            }
        }

        // Generate nodes with parameter substitution
        let nodes: Vec<GeneratedNode> = template
            .nodes
            .iter()
            .map(|node| {
                let action = substitute_params_in_action(node, &input.params, template);
                GeneratedNode {
                    id: node.id.clone(),
                    name: node.name.clone(),
                    action,
                    retry: node.retry.clone(),
                    validate: node.validate.clone(),
                }
            })
            .collect();

        // Build edges from depends_on
        let edges: Vec<(String, String)> = template
            .nodes
            .iter()
            .flat_map(|node| {
                node.depends_on
                    .iter()
                    .map(|dep| (node.id.clone(), dep.clone()))
                    .collect::<Vec<_>>()
            })
            .collect();

        // Attempt topological sort
        let topological_order = compute_topological_order(&template.nodes);
        let has_cycle = topological_order.is_empty() && !template.nodes.is_empty();
        let mut errors = Vec::new();

        if has_cycle {
            errors.push("Cycle detected in template dependency graph".to_string());
        }

        Ok(GenerateOutput {
            template_id: input.template_id,
            node_count: nodes.len(),
            nodes,
            edges,
            valid: !has_cycle,
            topological_order: topological_order
                .into_iter()
                .map(|s| s.to_string())
                .collect(),
            errors,
            execution_id: input.execution_id,
        })
    }

    async fn get_template(
        &self,
        input: GetTemplateInput,
    ) -> Result<Option<TemplateSummary>, TemplateError> {
        let templates = self.templates.read().expect("lock poisoned");
        Ok(templates
            .get(&input.template_id)
            .map(|entry| TemplateSummary {
                id: entry.template.id.clone(),
                name: entry.template.name.clone(),
                description: entry.template.description.clone(),
                version: entry.template.version.clone(),
                param_count: entry.template.parameters.len(),
                node_count: entry.template.nodes.len(),
                tags: entry.template.tags.clone(),
                category: entry.template.category.clone(),
                is_builtin: entry.is_builtin,
            }))
    }

    async fn get_template_full(
        &self,
        template_id: &str,
    ) -> Option<crate::templates::domain::Template> {
        let templates = self.templates.read().expect("lock poisoned");
        templates.get(template_id).map(|entry| entry.template.clone())
    }

    #[tracing::instrument(skip_all)]
    async fn list_templates(&self) -> Result<ListTemplatesOutput, TemplateError> {
        let templates = self.templates.read().expect("lock poisoned");
        let summaries: Vec<TemplateSummary> = templates.values().map(|entry| TemplateSummary {
                id: entry.template.id.clone(),
                name: entry.template.name.clone(),
                description: entry.template.description.clone(),
                version: entry.template.version.clone(),
                param_count: entry.template.parameters.len(),
                node_count: entry.template.nodes.len(),
                tags: entry.template.tags.clone(),
                category: entry.template.category.clone(),
                is_builtin: entry.is_builtin,
            })
            .collect();

        let total = summaries.len();

        Ok(ListTemplatesOutput {
            templates: summaries,
            total,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn has_template(&self, template_id: &str) -> bool {
        let templates = self.templates.read().expect("lock poisoned");
        templates.contains_key(template_id)
    }

    #[tracing::instrument(skip_all)]
    async fn template_count(&self) -> usize {
        let templates = self.templates.read().expect("lock poisoned");
        templates.len()
    }
}

// ---------------------------------------------------------------------------
// Parameter substitution
// ---------------------------------------------------------------------------

/// Perform `{{ param_name }}` substitution in a node's action fields.
fn substitute_params_in_action(
    node: &crate::templates::domain::TemplateNode,
    params: &HashMap<String, serde_json::Value>,
    _template: &Template,
) -> crate::templates::domain::TemplateAction {
    let action = &node.action;
    let substitute = |val: &str| -> String {
        let mut result = val.to_string();
        for (key, value) in params {
            let placeholder = format!("{{{{ {} }}}}", key);
            let val_str = match value {
                serde_json::Value::String(s) => s.clone(),
                serde_json::Value::Number(n) => n.to_string(),
                serde_json::Value::Bool(b) => b.to_string(),
                _ => value.to_string(),
            };
            result = result.replace(&placeholder, &val_str);
            // Also try without spaces
            let placeholder_tight = format!("{{{{{}}}}}", key);
            result = result.replace(&placeholder_tight, &val_str);
        }
        result
    };

    match action {
        crate::templates::domain::TemplateAction::FileRead { path } => {
            crate::templates::domain::TemplateAction::FileRead {
                path: substitute(path),
            }
        }
        crate::templates::domain::TemplateAction::FileWrite { path, content } => {
            crate::templates::domain::TemplateAction::FileWrite {
                path: substitute(path),
                content: substitute(content),
            }
        }
        crate::templates::domain::TemplateAction::FileAppend { path, content } => {
            crate::templates::domain::TemplateAction::FileAppend {
                path: substitute(path),
                content: substitute(content),
            }
        }
        crate::templates::domain::TemplateAction::FilePatch {
            path,
            search,
            insert,
            before,
        } => crate::templates::domain::TemplateAction::FilePatch {
            path: substitute(path),
            search: substitute(search),
            insert: substitute(insert),
            before: *before,
        },
        crate::templates::domain::TemplateAction::RunCommand {
            command,
            cwd,
            timeout_secs,
            env,
        } => crate::templates::domain::TemplateAction::RunCommand {
            command: substitute(command),
            cwd: cwd.as_ref().map(|c| substitute(c)),
            timeout_secs: *timeout_secs,
            env: env.clone(),
        },
        crate::templates::domain::TemplateAction::LspQuery {
            query_type,
            file,
            line,
            column,
        } => crate::templates::domain::TemplateAction::LspQuery {
            query_type: query_type.clone(),
            file: substitute(file),
            line: *line,
            column: *column,
        },
        crate::templates::domain::TemplateAction::GitRead {
            command,
            path,
            max_results,
        } => crate::templates::domain::TemplateAction::GitRead {
            command: substitute(command),
            path: path.as_ref().map(|p| substitute(p)),
            max_results: *max_results,
        },
        crate::templates::domain::TemplateAction::GitStage { path } => {
            crate::templates::domain::TemplateAction::GitStage {
                path: substitute(path),
            }
        }
        crate::templates::domain::TemplateAction::GitCommit {
            message,
            auto_stage,
        } => crate::templates::domain::TemplateAction::GitCommit {
            message: substitute(message),
            auto_stage: *auto_stage,
        },
    }
}

/// Compute topological order of node IDs using Kahn's algorithm.
///
/// Returns an empty vec if a cycle is detected.
fn compute_topological_order(
    nodes: &[crate::templates::domain::TemplateNode],
) -> Vec<&str> {
    let mut in_degree: HashMap<&str, usize> = HashMap::new();
    let mut adjacency: HashMap<&str, Vec<&str>> = HashMap::new();

    // Initialize
    for node in nodes {
        in_degree.entry(&node.id).or_insert(0);
        adjacency.entry(&node.id).or_default();
    }

    // Build edges (depends_on means: dep must run BEFORE node)
    for node in nodes {
        for dep in &node.depends_on {
            if let Some(edges) = adjacency.get_mut(dep.as_str()) {
                edges.push(&node.id);
                *in_degree.get_mut(&node.id as &str).unwrap_or(&mut 0) += 1;
            }
        }
    }

    // Start with zero in-degree nodes
    let mut queue: Vec<&str> = in_degree
        .iter()
        .filter(|&(_, &deg)| deg == 0)
        .map(|(id, _)| *id)
        .collect();

    let mut order = Vec::with_capacity(nodes.len());

    while let Some(node_id) = queue.pop() {
        order.push(node_id);
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

    // If we couldn't sort all nodes, there's a cycle
    if order.len() != nodes.len() {
        return vec![];
    }

    order
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use crate::templates::domain::{TemplateAction, TemplateNode};
    use uuid::Uuid;

    #[tracing::instrument(skip_all)]
    fn create_test_template(id: &str, name: &str) -> Template {
        Template {
            id: id.to_string(),
            name: name.to_string(),
            description: format!("Template {}", id),
            version: "1.0.0".to_string(),
            parameters: vec![],
            nodes: vec![],
            tags: vec![],
            category: None,
            author: None,
        }
    }

    #[tokio::test]
    async fn test_register_and_list() {
        let engine = TemplateEngineImpl::new();
        let template = create_test_template("test-1", "Test 1");

        let output = engine
            .register(RegisterInput {
                template,
                overwrite: false,
            })
            .await
            .unwrap();

        assert_eq!(output.template_id, "test-1");
        assert!(!output.overwritten);

        let list = engine.list_templates().await.unwrap();
        assert_eq!(list.total, 1);
    }

    #[tokio::test]
    async fn test_register_duplicate() {
        let engine = TemplateEngineImpl::new();
        let t1 = create_test_template("dup", "Dup");
        let t2 = create_test_template("dup", "Dup 2");

        engine
            .register(RegisterInput {
                template: t1,
                overwrite: false,
            })
            .await
            .unwrap();

        let result = engine
            .register(RegisterInput {
                template: t2,
                overwrite: false,
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TemplateError::DuplicateTemplate { .. }
        ));
    }

    #[tokio::test]
    async fn test_register_overwrite() {
        let engine = TemplateEngineImpl::new();
        let t1 = create_test_template("ow", "Original");
        let t2 = create_test_template("ow", "Overwritten");

        engine
            .register(RegisterInput {
                template: t1,
                overwrite: false,
            })
            .await
            .unwrap();

        let output = engine
            .register(RegisterInput {
                template: t2,
                overwrite: true,
            })
            .await
            .unwrap();

        assert!(output.overwritten);

        let tmpl = engine
            .get_template(GetTemplateInput {
                template_id: "ow".to_string(),
            })
            .await
            .unwrap();
        assert_eq!(tmpl.unwrap().name, "Overwritten");
    }

    #[tokio::test]
    async fn test_get_template_not_found() {
        let engine = TemplateEngineImpl::new();

        let result = engine
            .get_template(GetTemplateInput {
                template_id: "nonexistent".to_string(),
            })
            .await
            .unwrap();

        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_has_template() {
        let engine = TemplateEngineImpl::new();
        engine
            .register(RegisterInput {
                template: create_test_template("present", "Present"),
                overwrite: false,
            })
            .await
            .unwrap();

        assert!(engine.has_template("present").await);
        assert!(!engine.has_template("missing").await);
    }

    #[tokio::test]
    async fn test_template_count() {
        let engine = TemplateEngineImpl::new();
        assert_eq!(engine.template_count().await, 0);

        for i in 0..3 {
            engine
                .register(RegisterInput {
                    template: create_test_template(&format!("t-{}", i), &format!("T {}", i)),
                    overwrite: false,
                })
                .await
                .unwrap();
        }

        assert_eq!(engine.template_count().await, 3);
    }

    #[tokio::test]
    async fn test_generate_with_params() {
        let engine = TemplateEngineImpl::new();

        let template = Template {
            id: "read-file".to_string(),
            name: "Read File".to_string(),
            description: "Read a file".to_string(),
            version: "1.0.0".to_string(),
            parameters: vec![crate::templates::domain::ParameterDef {
                name: "target".to_string(),
                description: "File to read".to_string(),
                required: true,
                param_type: crate::templates::domain::ParamType::Path,
                default: None,
                constraints: vec![],
            }],
            nodes: vec![TemplateNode {
                id: "read".to_string(),
                name: "Read".to_string(),
                depends_on: vec![],
                action: TemplateAction::FileRead {
                    path: "{{ target }}".to_string(),
                },
                description: None,
                retry: crate::templates::domain::RetryConfig::default(),
                validate: vec![],
                intent: None,
            }],
            tags: vec![],
            category: None,
            author: None,
        };

        engine
            .register(RegisterInput {
                template,
                overwrite: false,
            })
            .await
            .unwrap();

        let mut params = HashMap::new();
        params.insert(
            "target".to_string(),
            serde_json::Value::String("src/main.rs".to_string()),
        );

        let output = engine
            .generate(GenerateInput {
                template_id: "read-file".to_string(),
                params,
                execution_id: Uuid::new_v4(),
                validate_graph: true,
            })
            .await
            .unwrap();

        assert!(output.valid);
        assert_eq!(output.node_count, 1);
        assert_eq!(output.nodes[0].id, "read");

        // Check parameter was substituted
        if let TemplateAction::FileRead { ref path } = output.nodes[0].action {
            assert_eq!(path, "src/main.rs");
        } else {
            panic!("Expected FileRead action");
        }
    }

    #[tokio::test]
    async fn test_generate_missing_required_param() {
        let engine = TemplateEngineImpl::new();

        let template = Template {
            id: "needs-param".to_string(),
            name: "Needs Param".to_string(),
            description: "Requires a parameter".to_string(),
            version: "1.0.0".to_string(),
            parameters: vec![crate::templates::domain::ParameterDef {
                name: "required_param".to_string(),
                description: "This is required".to_string(),
                required: true,
                param_type: crate::templates::domain::ParamType::String,
                default: None,
                constraints: vec![],
            }],
            nodes: vec![],
            tags: vec![],
            category: None,
            author: None,
        };

        engine
            .register(RegisterInput {
                template,
                overwrite: false,
            })
            .await
            .unwrap();

        let result = engine
            .generate(GenerateInput {
                template_id: "needs-param".to_string(),
                params: HashMap::new(),
                execution_id: Uuid::new_v4(),
                validate_graph: true,
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TemplateError::MissingParameter { .. }
        ));
    }

    #[tokio::test]
    async fn test_generate_template_not_found() {
        let engine = TemplateEngineImpl::new();

        let result = engine
            .generate(GenerateInput {
                template_id: "does-not-exist".to_string(),
                params: HashMap::new(),
                execution_id: Uuid::new_v4(),
                validate_graph: true,
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            TemplateError::NotFound { .. }
        ));
    }

    #[tokio::test]
    async fn test_cycle_detection_in_generate() {
        let engine = TemplateEngineImpl::new();

        let template = Template {
            id: "cycle".to_string(),
            name: "Cycle".to_string(),
            description: "Has a cycle".to_string(),
            version: "1.0.0".to_string(),
            parameters: vec![],
            nodes: vec![
                TemplateNode {
                    id: "a".to_string(),
                    name: "A".to_string(),
                    depends_on: vec!["b".to_string()],
                    action: TemplateAction::FileRead {
                        path: "a.txt".to_string(),
                    },
                    description: None,
                    retry: crate::templates::domain::RetryConfig::default(),
                    validate: vec![],
                    intent: None,
                },
                TemplateNode {
                    id: "b".to_string(),
                    name: "B".to_string(),
                    depends_on: vec!["a".to_string()],
                    action: TemplateAction::FileRead {
                        path: "b.txt".to_string(),
                    },
                    description: None,
                    retry: crate::templates::domain::RetryConfig::default(),
                    validate: vec![],
                    intent: None,
                },
            ],
            tags: vec![],
            category: None,
            author: None,
        };

        engine
            .register(RegisterInput {
                template,
                overwrite: false,
            })
            .await
            .unwrap();

        let output = engine
            .generate(GenerateInput {
                template_id: "cycle".to_string(),
                params: HashMap::new(),
                execution_id: Uuid::new_v4(),
                validate_graph: true,
            })
            .await
            .unwrap();

        assert!(!output.valid);
        assert!(!output.errors.is_empty());
        assert!(output.topological_order.is_empty());
    }
}
