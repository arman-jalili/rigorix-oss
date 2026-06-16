//! LspQueryTool — query a language server for code intelligence.
//!
//! @canonical .pi/architecture/modules/tool-system.md#lsp
//! Implements: Tool trait — LspQuery concrete tool
//! Issue: #125
//!
//! Queries a language server for code intelligence data including
//! go-to-definition, find-references, and hover information.
//! Read-only operation (Low risk).

use async_trait::async_trait;

use crate::tools::application::dto::{ToolInput, ToolResult};
use crate::tools::domain::{Tool, ToolError};

/// Supported LSP query types.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LspQueryType {
    /// Go to definition of a symbol.
    GotoDefinition,
    /// Find all references to a symbol.
    FindReferences,
    /// Get hover information for a symbol.
    Hover,
    /// Get code completion suggestions.
    Completion,
    /// Get document symbols.
    DocumentSymbols,
}

impl LspQueryType {
    /// Parse a query type from a string.
    pub fn parse(s: &str) -> Option<Self> {
        match s {
            "goto-definition" | "goto_definition" => Some(Self::GotoDefinition),
            "find-references" | "find_references" | "references" => Some(Self::FindReferences),
            "hover" => Some(Self::Hover),
            "completion" => Some(Self::Completion),
            "document-symbols" | "document_symbols" | "symbols" => Some(Self::DocumentSymbols),
            _ => None,
        }
    }

    /// Get the LSP method string for this query type.
    pub fn lsp_method(&self) -> &'static str {
        match self {
            Self::GotoDefinition => "textDocument/definition",
            Self::FindReferences => "textDocument/references",
            Self::Hover => "textDocument/hover",
            Self::Completion => "textDocument/completion",
            Self::DocumentSymbols => "textDocument/documentSymbol",
        }
    }
}

/// Tool for querying a language server for code intelligence.
///
/// # Input Parameters
/// - `query_type` (required, string): Type of query (goto-definition, find-references,
///   hover, completion, document-symbols).
/// - `file` (required, string): File path to query.
/// - `line` (required, int): Line number (0-indexed).
/// - `column` (required, int): Column number (0-indexed).
///
/// # Risk Level
/// Low — read-only, no side effects.
///
/// # Implementation Notes
/// Currently returns a structured description of what the query would do.
/// Full LSP integration requires an LSP client connection.
pub struct LspQueryTool;

impl LspQueryTool {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LspQueryTool {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Tool for LspQueryTool {
    fn name(&self) -> &str {
        "lsp-query"
    }

    async fn execute(&self, input: &ToolInput) -> Result<ToolResult, ToolError> {
        let query_type_str = input.require_string("query_type")?;
        let file = input.require_string("file")?;
        let line = input.require_u64("line")?;
        let column = input.require_u64("column")?;

        let query_type = LspQueryType::parse(&query_type_str).ok_or_else(|| {
            ToolError::InvalidInput(format!(
                "Unknown query type: '{}'. Supported types: goto-definition, find-references, \
                 hover, completion, document-symbols",
                query_type_str
            ))
        })?;

        // In a full implementation, this would connect to an LSP server.
        // For now, return a structured description.
        let output = format!(
            "LSP query: type={}, file={}, position=({}, {})",
            query_type.lsp_method(),
            file,
            line,
            column
        );

        Ok(ToolResult {
            output,
            exit_code: 0,
            side_effects: vec![],
            duration_ms: 0,
            dry_run: false,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn make_input(query_type: &str, file: &str, line: u64, column: u64) -> ToolInput {
        let mut params = HashMap::new();
        params.insert(
            "query_type".to_string(),
            serde_json::Value::String(query_type.to_string()),
        );
        params.insert(
            "file".to_string(),
            serde_json::Value::String(file.to_string()),
        );
        params.insert(
            "line".to_string(),
            serde_json::Value::Number(serde_json::Number::from(line)),
        );
        params.insert(
            "column".to_string(),
            serde_json::Value::Number(serde_json::Number::from(column)),
        );
        ToolInput::new(params)
    }

    #[tokio::test]
    async fn test_goto_definition() {
        let tool = LspQueryTool::new();
        let result = tool
            .execute(&make_input("goto-definition", "src/main.rs", 10, 5))
            .await
            .unwrap();

        assert!(result.is_success());
        assert!(result.output.contains("textDocument/definition"));
        assert!(result.output.contains("src/main.rs"));
    }

    #[tokio::test]
    async fn test_find_references() {
        let tool = LspQueryTool::new();
        let result = tool
            .execute(&make_input("references", "src/lib.rs", 42, 0))
            .await
            .unwrap();

        assert!(result.is_success());
        assert!(result.output.contains("textDocument/references"));
    }

    #[tokio::test]
    async fn test_hover() {
        let tool = LspQueryTool::new();

        let mut params = HashMap::new();
        params.insert(
            "query_type".to_string(),
            serde_json::Value::String("hover".to_string()),
        );
        params.insert(
            "file".to_string(),
            serde_json::Value::String("src/main.rs".to_string()),
        );
        params.insert(
            "line".to_string(),
            serde_json::Value::Number(serde_json::Number::from(0)),
        );
        params.insert(
            "column".to_string(),
            serde_json::Value::Number(serde_json::Number::from(0)),
        );
        let input = ToolInput::new(params);

        let result = tool.execute(&input).await.unwrap();
        assert!(result.output.contains("textDocument/hover"));
    }

    #[tokio::test]
    async fn test_unknown_query_type() {
        let tool = LspQueryTool::new();
        let result = tool
            .execute(&make_input("invalid_type", "file.rs", 0, 0))
            .await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_missing_parameters() {
        let tool = LspQueryTool::new();
        let result = tool.execute(&ToolInput::new(HashMap::new())).await;

        assert!(result.is_err());
        assert!(matches!(result.unwrap_err(), ToolError::InvalidInput(_)));
    }

    #[tokio::test]
    async fn test_tool_name() {
        let tool = LspQueryTool::new();
        assert_eq!(tool.name(), "lsp-query");
    }

    #[test]
    fn test_lsp_query_type_parsing() {
        assert_eq!(
            LspQueryType::parse("goto-definition"),
            Some(LspQueryType::GotoDefinition)
        );
        assert_eq!(
            LspQueryType::parse("goto_definition"),
            Some(LspQueryType::GotoDefinition)
        );
        assert_eq!(
            LspQueryType::parse("references"),
            Some(LspQueryType::FindReferences)
        );
        assert_eq!(
            LspQueryType::parse("find-references"),
            Some(LspQueryType::FindReferences)
        );
        assert_eq!(LspQueryType::parse("hover"), Some(LspQueryType::Hover));
        assert_eq!(
            LspQueryType::parse("completion"),
            Some(LspQueryType::Completion)
        );
        assert_eq!(
            LspQueryType::parse("symbols"),
            Some(LspQueryType::DocumentSymbols)
        );
        assert_eq!(LspQueryType::parse("unknown"), None);
    }

    #[test]
    fn test_lsp_method_names() {
        assert_eq!(
            LspQueryType::GotoDefinition.lsp_method(),
            "textDocument/definition"
        );
        assert_eq!(
            LspQueryType::FindReferences.lsp_method(),
            "textDocument/references"
        );
        assert_eq!(LspQueryType::Hover.lsp_method(), "textDocument/hover");
        assert_eq!(
            LspQueryType::Completion.lsp_method(),
            "textDocument/completion"
        );
        assert_eq!(
            LspQueryType::DocumentSymbols.lsp_method(),
            "textDocument/documentSymbol"
        );
    }

    #[tokio::test]
    async fn test_no_side_effects() {
        let tool = LspQueryTool::new();
        let result = tool
            .execute(&make_input("hover", "file.rs", 1, 1))
            .await
            .unwrap();

        assert!(!result.has_side_effects());
    }
}
