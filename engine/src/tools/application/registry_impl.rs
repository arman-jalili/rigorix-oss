//! ToolRegistryImpl — concrete implementation of ToolRegistryService and ToolExecutionService.
//!
//! @canonical .pi/architecture/modules/tool-system.md#registry
//! Implements: #126 — ToolRegistry service implementation
//! Issue: #126
//!
//! Thread-safe registry holding all registered tools by name.
//! Integrates with risk gating to apply execution policies.

use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;

use crate::tools::application::dto::{
    ExecuteToolInput, ExecuteToolOutput, GetToolInput, GetToolOutput, ListToolsOutput,
    RegisterToolInput, RegisterToolOutput, ToolInfo, ToolInput, ToolResult,
};
use crate::tools::application::service::{ToolExecutionService, ToolRegistryService};
use crate::tools::domain::risk_mapping::default_risk_level_for;
use crate::tools::domain::{Tool, ToolError};

/// Concrete implementation of both ToolRegistryService and ToolExecutionService.
///
/// Holds registered tools in a thread-safe HashMap behind a tokio RwLock.
/// Applies risk gating based on the tool's default risk level.
pub struct ToolRegistryImpl {
    /// Registered tools keyed by kebab-case name.
    tools: tokio::sync::RwLock<HashMap<String, RegisteredTool>>,
}

/// Internal wrapper combining a tool instance with its metadata.
struct RegisteredTool {
    tool: Arc<dyn Tool>,
    display_name: Option<String>,
    description: Option<String>,
    #[allow(dead_code)]
    usage_hint: Option<String>,
}

impl ToolRegistryImpl {
    /// Create a new empty ToolRegistry.
    pub fn new() -> Self {
        Self {
            tools: tokio::sync::RwLock::new(HashMap::new()),
        }
    }

    /// Create a ToolInfo from a registered tool entry.
    #[tracing::instrument(skip_all)]
    fn build_tool_info(name: &str, entry: &RegisteredTool) -> ToolInfo {
        let risk_level = default_risk_level_for(name)
            .unwrap_or(crate::risk_gating::domain::risk_level::RiskLevel::High);

        ToolInfo {
            name: name.to_string(),
            display_name: entry.display_name.clone(),
            description: entry.description.clone(),
            risk_level,
            read_only: risk_level.is_low(),
        }
    }
}

impl Default for ToolRegistryImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ToolRegistryService for ToolRegistryImpl {
    async fn register_tool(
        &self,
        input: RegisterToolInput,
        tool: Box<dyn Tool>,
    ) -> Result<RegisterToolOutput, ToolError> {
        let name = input.name.clone();

        if name.trim().is_empty() {
            return Err(ToolError::InvalidInput(
                "Tool name must not be empty".to_string(),
            ));
        }

        if tool.name() != name {
            return Err(ToolError::InvalidInput(format!(
                "Tool name mismatch: input '{}' but tool reports '{}'",
                name,
                tool.name()
            )));
        }

        let mut tools = self.tools.write().await;

        let replaced = tools.contains_key(&name);

        tools.insert(
            name.clone(),
            RegisteredTool {
                tool: Arc::from(tool),
                display_name: input.display_name,
                description: input.description,
                usage_hint: input.usage_hint,
            },
        );

        Ok(RegisterToolOutput {
            name,
            replaced,
            total_tools: tools.len(),
        })
    }

    #[tracing::instrument(skip_all)]
    async fn execute_tool(&self, input: ExecuteToolInput) -> Result<ExecuteToolOutput, ToolError> {
        let tool_name = input.tool_name.clone();

        // Look up the tool and extract risk level + tool Arc
        let (risk_level, tool_arc) = {
            let tools = self.tools.read().await;

            tools
                .get(&tool_name)
                .map(|entry| {
                    let risk_level = default_risk_level_for(&tool_name)
                        .unwrap_or(crate::risk_gating::domain::risk_level::RiskLevel::High);
                    (risk_level, entry.tool.clone())
                })
                .ok_or_else(|| {
                    let available: Vec<String> = tools.keys().cloned().collect();
                    ToolError::NotFound(format!(
                        "Tool '{}' not found. Available: [{}]",
                        tool_name,
                        available.join(", ")
                    ))
                })?
        };

        // Apply risk gating
        let tool_input = ToolInput {
            params: input.params,
            execution_id: Some(input.execution_id),
        };

        let dry_run = risk_level.is_high();

        let result = if dry_run {
            // High risk: return dry-run result without side effects
            ToolResult {
                output: format!(
                    "[DRY RUN] Would execute '{}' with risk level {:?}",
                    tool_name, risk_level
                ),
                exit_code: 0,
                side_effects: vec![],
                duration_ms: 0,
                dry_run: true,
            }
        } else if risk_level.is_medium() {
            // Medium risk: would require confirmation
            return Err(ToolError::RequiresConfirmation);
        } else {
            // Low risk: auto-execute
            tool_arc.execute(&tool_input).await?
        };

        Ok(ExecuteToolOutput {
            result,
            risk_level,
            dry_run,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn get_tool(&self, input: GetToolInput) -> Result<GetToolOutput, ToolError> {
        let tools = self.tools.read().await;

        if let Some(entry) = tools.get(&input.tool_name) {
            Ok(GetToolOutput {
                found: true,
                tool: Some(Self::build_tool_info(&input.tool_name, entry)),
            })
        } else {
            Ok(GetToolOutput {
                found: false,
                tool: None,
            })
        }
    }

    #[tracing::instrument(skip_all)]
    async fn list_tools(&self) -> Result<ListToolsOutput, ToolError> {
        let tools = self.tools.read().await;

        let tool_list: Vec<ToolInfo> = tools
            .iter()
            .map(|(name, entry)| Self::build_tool_info(name, entry))
            .collect();

        let total = tool_list.len();

        Ok(ListToolsOutput {
            tools: tool_list,
            total,
        })
    }

    #[tracing::instrument(skip_all)]
    async fn has_tool(&self, tool_name: &str) -> bool {
        let tools = self.tools.read().await;
        tools.contains_key(tool_name)
    }

    #[tracing::instrument(skip_all)]
    async fn tool_count(&self) -> usize {
        let tools = self.tools.read().await;
        tools.len()
    }
}

#[async_trait]
impl ToolExecutionService for ToolRegistryImpl {
    async fn execute_tool_direct(
        &self,
        tool: &dyn Tool,
        input: ToolInput,
    ) -> Result<ToolResult, ToolError> {
        tool.execute(&input).await
    }

    #[tracing::instrument(skip_all)]
    async fn dry_run(&self, _tool: &dyn Tool, _input: ToolInput) -> Result<ToolResult, ToolError> {
        Ok(ToolResult {
            output: "[DRY RUN] Preview only, no side effects".to_string(),
            exit_code: 0,
            side_effects: vec![],
            duration_ms: 0,
            dry_run: true,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::application::dto::ToolInput;
    use crate::tools::application::file_read_tool::FileReadTool;
    use crate::tools::application::file_write_tool::FileWriteTool;
    use crate::tools::application::run_command_tool::RunCommandTool;
    use std::collections::HashMap;
    use tempfile::TempDir;

    #[tracing::instrument(skip_all)]
    fn create_test_tool(name: &str) -> (Box<dyn Tool>, TempDir) {
        let dir = TempDir::new().unwrap();
        let tool: Box<dyn Tool> = match name {
            "file-read" => Box::new(FileReadTool::new(dir.path().to_str().unwrap())),
            "file-write" => Box::new(FileWriteTool::new(dir.path().to_str().unwrap())),
            "run-command" => Box::new(RunCommandTool::new(
                dir.path().to_str().unwrap(),
                vec!["echo"],
            )),
            _ => panic!("Unknown test tool: {}", name),
        };
        (tool, dir)
    }

    #[tracing::instrument(skip_all)]
    fn register_input(name: &str) -> RegisterToolInput {
        RegisterToolInput {
            name: name.to_string(),
            display_name: None,
            description: None,
            usage_hint: None,
        }
    }

    #[tokio::test]
    async fn test_register_and_list_tool() {
        let (tool, _dir) = create_test_tool("file-read");
        let registry = ToolRegistryImpl::new();

        let output = registry
            .register_tool(register_input("file-read"), tool)
            .await
            .unwrap();

        assert_eq!(output.name, "file-read");
        assert!(!output.replaced);
        assert_eq!(output.total_tools, 1);

        let list = registry.list_tools().await.unwrap();
        assert_eq!(list.total, 1);
        assert_eq!(list.tools[0].name, "file-read");
    }

    #[tokio::test]
    async fn test_register_multiple_tools() {
        let (tool1, _d1) = create_test_tool("file-read");
        let (tool2, _d2) = create_test_tool("file-write");
        let registry = ToolRegistryImpl::new();

        registry
            .register_tool(register_input("file-read"), tool1)
            .await
            .unwrap();
        registry
            .register_tool(register_input("file-write"), tool2)
            .await
            .unwrap();

        assert_eq!(registry.tool_count().await, 2);
    }

    #[tokio::test]
    async fn test_register_duplicate_replaces() {
        let (tool1, _d1) = create_test_tool("file-read");
        let (tool2, _d2) = create_test_tool("file-read");
        let registry = ToolRegistryImpl::new();

        let first = registry
            .register_tool(register_input("file-read"), tool1)
            .await
            .unwrap();
        assert!(!first.replaced);

        let second = registry
            .register_tool(register_input("file-read"), tool2)
            .await
            .unwrap();
        assert!(second.replaced);
    }

    #[tokio::test]
    async fn test_register_empty_name() {
        let (tool, _dir) = create_test_tool("file-read");
        let registry = ToolRegistryImpl::new();

        let result = registry
            .register_tool(
                RegisterToolInput {
                    name: "".to_string(),
                    display_name: None,
                    description: None,
                    usage_hint: None,
                },
                tool,
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_register_name_mismatch() {
        let (tool, _dir) = create_test_tool("file-read");
        let registry = ToolRegistryImpl::new();

        let result = registry
            .register_tool(
                RegisterToolInput {
                    name: "wrong-name".to_string(),
                    display_name: None,
                    description: None,
                    usage_hint: None,
                },
                tool,
            )
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_get_tool() {
        let (tool, _dir) = create_test_tool("file-read");
        let registry = ToolRegistryImpl::new();
        registry
            .register_tool(register_input("file-read"), tool)
            .await
            .unwrap();

        let output = registry
            .get_tool(GetToolInput {
                tool_name: "file-read".to_string(),
            })
            .await
            .unwrap();

        assert!(output.found);
        assert!(output.tool.is_some());
        assert_eq!(output.tool.unwrap().name, "file-read");
    }

    #[tokio::test]
    async fn test_get_tool_not_found() {
        let registry = ToolRegistryImpl::new();

        let output = registry
            .get_tool(GetToolInput {
                tool_name: "nonexistent".to_string(),
            })
            .await
            .unwrap();

        assert!(!output.found);
        assert!(output.tool.is_none());
    }

    #[tokio::test]
    async fn test_has_tool() {
        let (tool, _dir) = create_test_tool("file-read");
        let registry = ToolRegistryImpl::new();
        registry
            .register_tool(register_input("file-read"), tool)
            .await
            .unwrap();

        assert!(registry.has_tool("file-read").await);
        assert!(!registry.has_tool("nonexistent").await);
    }

    #[tokio::test]
    async fn test_execute_low_risk_tool() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "hello").unwrap();

        let tool = Box::new(FileReadTool::new(dir.path().to_str().unwrap()));
        let registry = ToolRegistryImpl::new();
        registry
            .register_tool(register_input("file-read"), tool)
            .await
            .unwrap();

        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::Value::String("test.txt".to_string()),
        );

        let result = registry
            .execute_tool(ExecuteToolInput {
                tool_name: "file-read".to_string(),
                params,
                execution_id: uuid::Uuid::new_v4(),
            })
            .await
            .unwrap();

        assert!(!result.dry_run);
        assert!(result.result.output.contains("hello"));
    }

    #[tokio::test]
    async fn test_execute_high_risk_dry_run() {
        let (tool, _dir) = create_test_tool("run-command");
        let registry = ToolRegistryImpl::new();
        registry
            .register_tool(register_input("run-command"), tool)
            .await
            .unwrap();

        let mut params = HashMap::new();
        params.insert(
            "command".to_string(),
            serde_json::Value::String("echo test".to_string()),
        );

        let result = registry
            .execute_tool(ExecuteToolInput {
                tool_name: "run-command".to_string(),
                params,
                execution_id: uuid::Uuid::new_v4(),
            })
            .await
            .unwrap();

        assert!(result.dry_run);
        assert!(result.result.output.contains("DRY RUN"));
    }

    #[tokio::test]
    async fn test_execute_medium_risk_requires_confirmation() {
        let (tool, _dir) = create_test_tool("file-write");
        let registry = ToolRegistryImpl::new();
        registry
            .register_tool(register_input("file-write"), tool)
            .await
            .unwrap();

        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::Value::String("test.txt".to_string()),
        );
        params.insert(
            "content".to_string(),
            serde_json::Value::String("content".to_string()),
        );

        let result = registry
            .execute_tool(ExecuteToolInput {
                tool_name: "file-write".to_string(),
                params,
                execution_id: uuid::Uuid::new_v4(),
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            ToolError::RequiresConfirmation
        ));
    }

    #[tokio::test]
    async fn test_execute_not_found() {
        let registry = ToolRegistryImpl::new();

        let result = registry
            .execute_tool(ExecuteToolInput {
                tool_name: "nonexistent".to_string(),
                params: HashMap::new(),
                execution_id: uuid::Uuid::new_v4(),
            })
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::NotFound(_)));
    }

    #[tokio::test]
    async fn test_execute_tool_direct() {
        let dir = TempDir::new().unwrap();
        let file_path = dir.path().join("test.txt");
        std::fs::write(&file_path, "direct").unwrap();

        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let registry = ToolRegistryImpl::new();

        let mut params = HashMap::new();
        params.insert(
            "path".to_string(),
            serde_json::Value::String("test.txt".to_string()),
        );

        let result = registry
            .execute_tool_direct(&tool, ToolInput::new(params))
            .await
            .unwrap();

        assert!(result.is_success());
        // FileReadTool returns JSON-serialized ReadFileOutput
        let read_output: crate::code_gen::application::dto::ReadFileOutput =
            serde_json::from_str(result.output.trim()).unwrap();
        assert_eq!(read_output.content, "direct");
    }

    #[tokio::test]
    async fn test_dry_run() {
        let dir = TempDir::new().unwrap();
        let tool = FileReadTool::new(dir.path().to_str().unwrap());
        let registry = ToolRegistryImpl::new();

        let result = registry
            .dry_run(&tool, ToolInput::new(HashMap::new()))
            .await
            .unwrap();

        assert!(result.dry_run);
        assert!(result.output.contains("DRY RUN"));
    }

    #[tokio::test]
    async fn test_empty_registry() {
        let registry = ToolRegistryImpl::new();

        assert_eq!(registry.tool_count().await, 0);
        let list = registry.list_tools().await.unwrap();
        assert_eq!(list.total, 0);
        assert!(!registry.has_tool("anything").await);
    }
}
