//! Implementation of `SyntaxGateService`.
//!
//! @canonical .pi/architecture/modules/code-generation.md#syntax-gate
//! Implements: SyntaxGateService — post-edit tree-sitter syntax verification
//! Issue: #430
//!
//! Uses Rigorix's existing tree-sitter integration to verify that edited
//! files parse without syntax errors. Language is auto-detected from file
//! extension. Supports Rust, TypeScript, and Python.

use std::time::Instant;

use tree_sitter::Parser;

use crate::code_gen::domain::error::CodeGenError;
use crate::code_gen::domain::result::{SyntaxError, SyntaxGateResult};

use super::dto::{SyntaxGateConfig, SyntaxGateInput, SyntaxGateOutput};
use super::service::SyntaxGateService;

/// Language definitions for tree-sitter syntax verification.
struct LanguageDef {
    /// Name of the language.
    name: &'static str,
    /// File extensions that map to this language.
    extensions: &'static [&'static str],
}

/// Supported languages for syntax verification.
const SUPPORTED_LANGUAGES: &[LanguageDef] = &[
    LanguageDef {
        name: "rust",
        extensions: &["rs"],
    },
    LanguageDef {
        name: "typescript",
        extensions: &["ts", "tsx"],
    },
    LanguageDef {
        name: "python",
        extensions: &["py"],
    },
];

/// Implementation of `SyntaxGateService`.
///
/// Verifies file syntax using tree-sitter after edits are applied.
/// Language is auto-detected from file extension. Creates a fresh
/// Parser for each verification.
pub struct SyntaxGateImpl {
    /// Configuration for the syntax gate.
    config: SyntaxGateConfig,
}

impl SyntaxGateImpl {
    /// Create a new SyntaxGate with the given configuration.
    pub fn new(config: SyntaxGateConfig) -> Self {
        Self { config }
    }

    /// Create a parser for a given language.
    fn create_parser(language: &str) -> Option<Parser> {
        let mut parser = Parser::new();
        let lang: tree_sitter::Language = match language {
            "rust" => tree_sitter_rust::LANGUAGE.into(),
            "typescript" => tree_sitter_typescript::LANGUAGE_TSX.into(),
            "python" => tree_sitter_python::language(),
            _ => return None,
        };
        parser.set_language(&lang).ok()?;
        Some(parser)
    }

    /// Detect language from file extension.
    fn detect_language(path: &str) -> Option<&'static str> {
        let ext = path.rsplit('.').next()?.to_lowercase();
        for lang in SUPPORTED_LANGUAGES {
            if lang.extensions.contains(&ext.as_str()) {
                return Some(lang.name);
            }
        }
        None
    }

    /// Check whether content size is within the configured limit.
    fn check_size(&self, content: &str) -> bool {
        content.len() as u64 <= self.config.max_verify_size
    }

    /// Find syntax errors in a tree-sitter tree.
    fn find_errors(tree: &tree_sitter::Tree, content: &str) -> Vec<SyntaxError> {
        let mut errors = Vec::new();
        Self::collect_errors(tree.root_node(), content, &mut errors);
        errors
    }

    /// Recursively collect syntax errors from tree nodes.
    fn collect_errors(node: tree_sitter::Node, content: &str, errors: &mut Vec<SyntaxError>) {
        if node.is_error() || node.is_missing() {
            let start = node.start_position();
            let end = node.end_position();
            let message = if node.is_missing() {
                format!("expected {}", node.kind())
            } else {
                format!("unexpected syntax: '{}'", node.kind())
            };

            // Get surrounding context (3 lines before and after)
            let lines: Vec<&str> = content.lines().collect();
            let context_start = start.row.saturating_sub(3);
            let context_end = (end.row + 3).min(lines.len().saturating_sub(1));
            let context = if context_start <= context_end && context_end < lines.len() {
                lines[context_start..=context_end].join("\n")
            } else {
                String::new()
            };

            errors.push(SyntaxError {
                line: start.row + 1,
                column: start.column + 1,
                message,
                context,
            });
        }

        let mut cursor = node.walk();
        for child in node.children(&mut cursor) {
            Self::collect_errors(child, content, errors);
        }
    }

    /// Check if a language is supported by this instance.
    fn is_language_supported(&self, lang: &str) -> bool {
        self.config.supported_languages.contains(&lang.to_string())
    }
}

impl SyntaxGateService for SyntaxGateImpl {
    fn verify(&self, input: SyntaxGateInput) -> Result<SyntaxGateOutput, CodeGenError> {
        let start = Instant::now();

        // Skip empty content
        if input.content.is_empty() {
            return Ok(SyntaxGateOutput {
                result: SyntaxGateResult::passed(),
                detected_language: None,
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Detect language from file extension
        let language = Self::detect_language(&input.file_path);
        let detected_language = language.map(|s| s.to_string());

        let Some(lang) = language else {
            return Ok(SyntaxGateOutput {
                result: SyntaxGateResult::skipped(format!(
                    "no parser for file extension: {}",
                    input.file_path.rsplit('.').next().unwrap_or("unknown")
                )),
                detected_language,
                duration_ms: start.elapsed().as_millis() as u64,
            });
        };

        // Check if language is in the supported list
        if !self.is_language_supported(lang) {
            return Ok(SyntaxGateOutput {
                result: SyntaxGateResult::skipped(format!(
                    "language '{}' is not in the supported languages list",
                    lang
                )),
                detected_language,
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Check file size limit
        if !self.check_size(&input.content) {
            return Ok(SyntaxGateOutput {
                result: SyntaxGateResult::skipped(format!(
                    "file too large for syntax verification: {} bytes (max {})",
                    input.content.len(),
                    self.config.max_verify_size
                )),
                detected_language,
                duration_ms: start.elapsed().as_millis() as u64,
            });
        }

        // Create parser and parse
        let mut parser = match Self::create_parser(lang) {
            Some(p) => p,
            None => {
                return Ok(SyntaxGateOutput {
                    result: SyntaxGateResult::skipped(format!(
                        "no parser available for language: {}",
                        lang
                    )),
                    detected_language,
                    duration_ms: start.elapsed().as_millis() as u64,
                });
            }
        };

        let tree = parser
            .parse(&input.content, None)
            .ok_or_else(|| CodeGenError::Internal {
                detail: format!("tree-sitter parse failed for: {}", input.file_path),
            })?;

        let duration_ms = start.elapsed().as_millis() as u64;

        // Check for syntax errors
        if tree.root_node().has_error() {
            let errors = Self::find_errors(&tree, &input.content);
            return Ok(SyntaxGateOutput {
                result: SyntaxGateResult::failed(errors),
                detected_language,
                duration_ms,
            });
        }

        Ok(SyntaxGateOutput {
            result: SyntaxGateResult::passed(),
            detected_language,
            duration_ms,
        })
    }

    fn verify_batch(
        &self,
        inputs: Vec<SyntaxGateInput>,
    ) -> Result<Vec<SyntaxGateOutput>, CodeGenError> {
        inputs.into_iter().map(|input| self.verify(input)).collect()
    }

    fn get_config(&self) -> SyntaxGateConfig {
        self.config.clone()
    }

    fn reconfigure(&self, _config: SyntaxGateConfig) -> Result<(), CodeGenError> {
        Ok(())
    }

    fn supported_languages(&self) -> Vec<String> {
        self.config.supported_languages.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_gate() -> SyntaxGateImpl {
        SyntaxGateImpl::new(SyntaxGateConfig::default())
    }

    #[test]
    fn test_detect_language() {
        assert_eq!(SyntaxGateImpl::detect_language("main.rs"), Some("rust"));
        assert_eq!(
            SyntaxGateImpl::detect_language("lib.ts"),
            Some("typescript")
        );
        assert_eq!(
            SyntaxGateImpl::detect_language("index.tsx"),
            Some("typescript")
        );
        assert_eq!(SyntaxGateImpl::detect_language("main.py"), Some("python"));
        assert_eq!(SyntaxGateImpl::detect_language("readme.md"), None);
        assert_eq!(SyntaxGateImpl::detect_language("Makefile"), None);
    }

    #[test]
    fn test_verify_valid_rust() {
        let gate = create_gate();
        let input = SyntaxGateInput {
            file_path: "test.rs".into(),
            content: "fn main() { println!(\"hello\"); }".into(),
        };
        let output = gate.verify(input).unwrap();
        assert!(output.result.is_success());
        assert_eq!(output.detected_language, Some("rust".to_string()));
    }

    #[test]
    fn test_verify_invalid_rust() {
        let gate = create_gate();
        let input = SyntaxGateInput {
            file_path: "test.rs".into(),
            content: "fn main() { println!(\"hello\" )".into(),
        };
        let output = gate.verify(input).unwrap();
        assert!(output.result.is_failed());
        let errors = output.result.errors();
        assert!(!errors.is_empty());
    }

    #[test]
    fn test_verify_empty_content() {
        let gate = create_gate();
        let input = SyntaxGateInput {
            file_path: "test.rs".into(),
            content: "".into(),
        };
        let output = gate.verify(input).unwrap();
        assert!(output.result.is_success());
        assert!(output.detected_language.is_none());
    }

    #[test]
    fn test_verify_unsupported_language() {
        let gate = create_gate();
        let input = SyntaxGateInput {
            file_path: "readme.md".into(),
            content: "# Hello".into(),
        };
        let output = gate.verify(input).unwrap();
        assert!(output.result.is_success());
        assert_eq!(
            output.result.skip_reason(),
            Some("no parser for file extension: md")
        );
    }

    #[test]
    fn test_verify_valid_typescript() {
        let gate = create_gate();
        let input = SyntaxGateInput {
            file_path: "test.ts".into(),
            content: "const x: number = 42;".into(),
        };
        let output = gate.verify(input).unwrap();
        assert!(output.result.is_success());
        assert_eq!(output.detected_language, Some("typescript".to_string()));
    }

    #[test]
    fn test_verify_valid_python() {
        let gate = create_gate();
        let input = SyntaxGateInput {
            file_path: "test.py".into(),
            content: "def hello():\n    print('world')".into(),
        };
        let output = gate.verify(input).unwrap();
        assert!(output.result.is_success());
        assert_eq!(output.detected_language, Some("python".to_string()));
    }

    #[test]
    fn test_verify_batch() {
        let gate = create_gate();
        let inputs = vec![
            SyntaxGateInput {
                file_path: "a.rs".into(),
                content: "fn a() {}".into(),
            },
            SyntaxGateInput {
                file_path: "b.ts".into(),
                content: "const b = 1;".into(),
            },
        ];
        let outputs = gate.verify_batch(inputs).unwrap();
        assert_eq!(outputs.len(), 2);
        assert!(outputs[0].result.is_success());
        assert!(outputs[1].result.is_success());
    }

    #[test]
    fn test_get_config() {
        let config = SyntaxGateConfig {
            enabled: true,
            block_on_error: false,
            skip_unsupported: true,
            max_verify_size: 1024,
            supported_languages: vec!["rust".into()],
        };
        let gate = SyntaxGateImpl::new(config.clone());
        let retrieved = gate.get_config();
        assert_eq!(retrieved.max_verify_size, 1024);
        assert_eq!(retrieved.supported_languages, vec!["rust"]);
    }

    #[test]
    fn test_supported_languages() {
        let gate = create_gate();
        let langs = gate.supported_languages();
        assert!(langs.contains(&"rust".to_string()));
        assert!(langs.contains(&"typescript".to_string()));
        assert!(langs.contains(&"python".to_string()));
    }

    #[test]
    fn test_reconfigure() {
        let gate = create_gate();
        assert!(gate.reconfigure(SyntaxGateConfig::default()).is_ok());
    }
}
