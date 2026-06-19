//! ParserRegistry — extensible parser registry for failure parsing.
//!
//! @canonical .pi/architecture/modules/failure-parser.md#registry
//! Implements: Contract Freeze — LanguageParser trait, ParserRegistry struct
//! Issue: #495
//!
//! # Contract (Frozen)
//! - LanguageParser trait is the parser interface for all language/tool parsers
//! - ParserRegistry manages a dynamic registry of parsers by tool name
//! - Built-in parsers: TypeScriptParser, JestParser, RustcParser, PytestParser
//! - Custom parsers can be registered at runtime

use async_trait::async_trait;

use crate::failure_parser::domain::{FailureParserError, ParsedFailure, SourceContext};

/// Trait for language/tool-specific parsers.
///
/// Each parser implementation handles one specific tool's output format
/// and converts it into structured `TemplateFailure` values.
///
/// Built-in implementations:
/// - `TypeScriptParser` → tool name "tsc"
/// - `JestParser` → tool name "jest"
/// - `RustcParser` → tool name "rustc"
/// - `PytestParser` → tool name "pytest"
#[async_trait]
pub trait LanguageParser: Send + Sync {
    /// The name of the tool this parser handles (e.g., "tsc", "jest", "rustc", "pytest").
    fn tool_name(&self) -> &str;

    /// Parse the raw output and return structured failures.
    ///
    /// If the output is clean (no failures), returns an empty `ParsedFailure`.
    /// If the output format is unrecognized, returns a `FailureParserError`.
    async fn parse(
        &self,
        output: &str,
        source_context: &SourceContext,
    ) -> Result<ParsedFailure, FailureParserError>;

    /// Returns `true` if this parser can handle the given tool name.
    fn supports_tool(&self, tool: &str) -> bool {
        self.tool_name() == tool
    }
}

/// Registry of language/tool parsers.
///
/// Maintains a mapping of tool names to parser implementations.
/// New parsers can be registered at runtime using `register()`.
///
/// Built-in parsers are registered at startup:
/// - TypeScriptParser → "tsc"
/// - JestParser → "jest"
/// - RustcParser → "rustc"
/// - PytestParser → "pytest"
pub struct ParserRegistry {
    /// Registered parsers keyed by tool name.
    parsers: std::collections::HashMap<String, Box<dyn LanguageParser>>,
}

impl ParserRegistry {
    /// Create a new empty ParserRegistry.
    pub fn new() -> Self {
        Self {
            parsers: std::collections::HashMap::new(),
        }
    }

    /// Register a parser for a specific tool.
    ///
    /// If a parser is already registered for this tool, it is replaced.
    /// Emits `FailureParserEvent::ParserRegistered` if event bus is available.
    pub fn register(&mut self, parser: Box<dyn LanguageParser>) {
        let tool = parser.tool_name().to_string();
        self.parsers.insert(tool, parser);
    }

    /// Get the parser for a specific tool.
    ///
    /// Returns `None` if no parser is registered for this tool.
    pub fn get(&self, tool: &str) -> Option<&dyn LanguageParser> {
        self.parsers.get(tool).map(|p| p.as_ref())
    }

    /// Returns `true` if a parser is registered for the given tool.
    pub fn has_parser(&self, tool: &str) -> bool {
        self.parsers.contains_key(tool)
    }

    /// Returns the list of all registered tool names.
    pub fn available_tools(&self) -> Vec<String> {
        let mut tools: Vec<String> = self.parsers.keys().cloned().collect();
        tools.sort();
        tools
    }

    /// Returns the number of registered parsers.
    pub fn len(&self) -> usize {
        self.parsers.len()
    }

    /// Returns `true` if no parsers are registered.
    pub fn is_empty(&self) -> bool {
        self.parsers.is_empty()
    }
}

impl Default for ParserRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A mock parser for testing.
    struct MockParser {
        tool: String,
    }

    #[async_trait]
    impl LanguageParser for MockParser {
        fn tool_name(&self) -> &str {
            &self.tool
        }

        async fn parse(
            &self,
            _output: &str,
            _source_context: &SourceContext,
        ) -> Result<ParsedFailure, FailureParserError> {
            Ok(ParsedFailure::from_failures(vec![], &self.tool))
        }
    }

    #[tokio::test]
    async fn test_parser_registry_register_and_get() {
        let mut registry = ParserRegistry::new();
        let parser = MockParser {
            tool: "tsc".to_string(),
        };
        registry.register(Box::new(parser));
        assert!(registry.has_parser("tsc"));
        assert!(!registry.has_parser("jest"));
    }

    #[tokio::test]
    async fn test_parser_registry_available_tools() {
        let mut registry = ParserRegistry::new();
        registry.register(Box::new(MockParser {
            tool: "rustc".to_string(),
        }));
        registry.register(Box::new(MockParser {
            tool: "tsc".to_string(),
        }));
        let tools = registry.available_tools();
        assert_eq!(tools.len(), 2);
        assert!(tools.contains(&"tsc".to_string()));
        assert!(tools.contains(&"rustc".to_string()));
    }

    #[tokio::test]
    async fn test_parser_registry_empty() {
        let registry = ParserRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[tokio::test]
    async fn test_parser_supports_tool() {
        let parser = MockParser {
            tool: "jest".to_string(),
        };
        assert!(parser.supports_tool("jest"));
        assert!(!parser.supports_tool("tsc"));
    }
}
