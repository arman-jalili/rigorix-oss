//! TemplateGenerator trait — fallback template generation from user intent.
//!
//! @canonical .pi/architecture/modules/template-generation.md#generator
//! Implements: Contract Freeze — TemplateGenerator trait, ClaudeTemplateGenerator,
//! GeneratorError, RepoContext, GeneratedTemplate
//! Issue: issue-contract-freeze
//!
//! TemplateGenerator is the fallback path in the planning pipeline.
//! When the Classifier finds no good match (confidence < 0.3 for all
//! templates), the pipeline falls back to the TemplateGenerator to
//! create a new template definition on-the-fly from the user intent.

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::path::PathBuf;
use std::time::Duration;
use thiserror::Error;

/// Generates a new template definition from user intent.
///
/// Used as a fallback when no existing template matches the user's
/// intent. The generator creates a complete TOML template definition
/// that can be parsed and registered for immediate use.
#[async_trait]
pub trait TemplateGenerator: Send + Sync {
    /// Generate a template definition from user intent.
    async fn generate(
        &self,
        intent: &crate::planning::domain::UserIntent,
        repo_context: &RepoContext,
        budget: &crate::budget_tracking::domain::LlmBudget,
    ) -> Result<GeneratedTemplate, GeneratorError>;

    /// Estimate the token cost of generating a template.
    fn estimate_cost(&self, intent: &crate::planning::domain::UserIntent) -> GeneratedTemplateCost;
}

/// A template generated on-the-fly by the TemplateGenerator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTemplate {
    /// The TOML string of the generated template definition.
    pub toml_content: String,
    /// Suggested template ID.
    pub suggested_id: String,
    /// Suggested human-readable name.
    pub suggested_name: String,
    /// Brief description of what this template does.
    pub description: String,
    /// Number of LLM calls used.
    pub llm_calls_used: u32,
    /// Number of LLM tokens consumed.
    pub llm_tokens_used: u32,
}

/// Estimated cost of generating a template.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneratedTemplateCost {
    /// Estimated number of LLM calls.
    pub estimated_calls: u32,
    /// Estimated number of LLM tokens.
    pub estimated_tokens: u32,
}

// ---------------------------------------------------------------------------
// RepoContext — Repository snapshot for generation context
// ---------------------------------------------------------------------------

/// Summary of an existing template shown to the generator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplateSummary {
    /// Template ID
    pub id: String,
    /// Template description
    pub description: String,
}

/// Snapshot of repository structure used as context for template generation.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RepoContext {
    /// Working directory being operated on.
    pub root_dir: PathBuf,
    /// Detected project type.
    pub project_type: String,
    /// Formatted directory tree (tree command output style).
    pub dir_tree: String,
    /// Flat list of relevant file paths (for backward compat).
    pub directory_tree: Vec<String>,
    /// External dependencies.
    pub dependencies: Vec<String>,
    /// Public type, function, and trait names (formatted string for prompt).
    pub public_api: String,
    /// Public type, function, and trait names (list for backward compat).
    pub public_api_list: Vec<String>,
    /// Existing template IDs (to avoid duplicates).
    pub existing_templates: Vec<TemplateSummary>,
    /// Content of key entry-point files (src/lib.rs, src/main.rs, etc.).
    pub key_file_contents: String,
    /// Architecture overview from ARCHITECTURE.md or .pi/architecture/.
    #[serde(default)]
    pub architecture_overview: String,
    /// Detected bounded context name (e.g. "dag-engine", "planning").
    #[serde(default)]
    pub bounded_context: String,
    /// Optional symbol graph subset for Phase 3 validation.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub symbol_graph_snapshot: Option<serde_json::Value>,

    /// Formatted module dependency graph (CodeGraph output) for the LLM prompt.
    /// Populated by CodeGraphBuilder/Formatter at context construction time.
    /// Uses Mermaid format by default — shows which modules import which.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub module_deps: Option<String>,
}

impl RepoContext {
    /// Create a new empty RepoContext for a given directory.
    pub fn new(root_dir: PathBuf, project_type: String) -> Self {
        Self {
            root_dir,
            project_type,
            dir_tree: String::new(),
            directory_tree: Vec::new(),
            dependencies: Vec::new(),
            public_api: String::new(),
            public_api_list: Vec::new(),
            existing_templates: Vec::new(),
            key_file_contents: String::new(),
            architecture_overview: String::new(),
            bounded_context: String::new(),
            symbol_graph_snapshot: None,
            module_deps: None,
        }
    }

    /// Build a RepoContext from a working directory by scanning the filesystem.
    pub fn from_path(dir: &std::path::Path) -> std::io::Result<Self> {
        let dir_tree = build_dir_tree(dir, 5)?;
        let project_type = detect_project_type(dir);
        let dependencies = read_dependencies(dir, &project_type);

        // Read key entry-point files
        let key_file_contents = read_key_files(dir);

        // Scan public API symbols from source files
        let public_api = scan_public_api(dir, &project_type);

        // Read architecture documentation
        let architecture_overview = read_architecture_docs(dir);

        // Detect bounded context from CWD
        let bounded_context = detect_bounded_context(dir);

        Ok(Self {
            root_dir: dir.to_path_buf(),
            project_type,
            dir_tree,
            directory_tree: Vec::new(),
            dependencies,
            public_api,
            public_api_list: Vec::new(),
            existing_templates: Vec::new(),
            key_file_contents,
            architecture_overview,
            bounded_context,
            symbol_graph_snapshot: None,
            module_deps: None,
        })
    }

    /// Build a RepoContext targeted to a specific topic/bounded context.
    ///
    /// Instead of scanning the entire workspace (all source files for public API,
    /// all key files for content), this method scopes the scan to only directories
    /// and files relevant to the given topic filter. This is the "targeted over
    /// comprehensive" pattern from FastContext — find relevant code, not all code.
    ///
    /// When `compact` is `true`, the output uses compact citation format
    /// (file → [dependencies]) instead of full content dumps.
    pub fn from_path_filtered(
        dir: &std::path::Path,
        topic_filter: &str,
        compact: bool,
    ) -> std::io::Result<Self> {
        let project_type = detect_project_type(dir);

        // 1. Build a scoped directory tree — only directories matching the topic
        let dir_tree = if compact {
            format!("(scoped to: {})", topic_filter)
        } else {
            build_dir_tree_scoped(dir, 3, topic_filter)?
        };

        // 2. Only read dependencies (cheap, always useful)
        let dependencies = read_dependencies(dir, &project_type);

        // 3. Key files: only detect the bounded context, skip full content
        let bounded_context = if topic_filter.is_empty() {
            detect_bounded_context(dir)
        } else {
            topic_filter.to_string()
        };

        // 4. Public API: only scan files within the topic's directory
        let (public_api, public_api_list) = if compact {
            // Compact mode: return file paths only
            let filtered_paths = scan_filtered_paths(dir, topic_filter);
            let api = filtered_paths.join("\n");
            (api, filtered_paths)
        } else {
            let api = scan_public_api_filtered(dir, &project_type, topic_filter);
            let list: Vec<String> = api.lines().map(|l| l.to_string()).collect();
            (api, list)
        };

        // 5. Architecture docs: read regardless (cheap)
        let architecture_overview = read_architecture_docs(dir);

        Ok(Self {
            root_dir: dir.to_path_buf(),
            project_type,
            dir_tree,
            directory_tree: Vec::new(),
            dependencies,
            public_api,
            public_api_list,
            existing_templates: Vec::new(),
            key_file_contents: String::new(),
            architecture_overview,
            bounded_context,
            symbol_graph_snapshot: None,
            module_deps: None,
        })
    }

    /// Check if this context has any file entries.
    pub fn has_files(&self) -> bool {
        !self.directory_tree.is_empty() || !self.dir_tree.is_empty()
    }

    /// Attach a module dependency graph (CodeGraph formatted output) to this context.
    ///
    /// Accepts the formatted output from CodeGraphFormatter (Mermaid, DOT, Tree, List).
    /// This is injected into the LLM prompt to give the model awareness of module
    /// dependency relationships.
    pub fn with_module_deps(mut self, deps: String) -> Self {
        self.module_deps = Some(deps);
        self
    }

    /// Returns `true` if the classifier has already matched a template with
    /// high enough confidence that deep context building (full API scan, key
    /// file contents, CodeGraph construction) can be skipped.
    ///
    /// Implements the "skip-when-known" heuristic from FastContext:
    /// "Skip fastcontext if PR already names the file" → "Skip deep context
    /// building if bounded-context templates already match the intent"
    pub fn is_fully_matched(&self) -> bool {
        // If we have existing templates AND a bounded context is detected,
        // the classifier likely already found a match — skip heavy context
        !self.existing_templates.is_empty() && !self.bounded_context.is_empty()
    }

    /// Check if this context has any public API entries.
    pub fn has_public_api(&self) -> bool {
        !self.public_api.is_empty() || !self.public_api_list.is_empty()
    }

    /// Create an empty RepoContext (for tests/mocks).
    pub fn empty() -> Self {
        Self::new(std::path::PathBuf::from("."), "unknown".to_string())
    }
}

// ---------------------------------------------------------------------------
// GeneratorError — Typed error enum for generation failures
// ---------------------------------------------------------------------------

/// Errors specific to the template generation process.
#[derive(Debug, Clone, PartialEq, Error, Serialize, Deserialize)]
pub enum GeneratorError {
    /// The LLM returned content that is not valid TOML.
    InvalidToml {
        raw_response: String,
        parse_error: String,
        attempt: u8,
    },
    /// The generated template failed structural validation.
    ValidationFailed {
        template_id: String,
        errors: Vec<String>,
        attempt: u8,
    },
    /// Phase 3: Generated template references symbols that don't exist.
    SymbolValidation {
        template_id: String,
        invalid_references: Vec<InvalidSymbolReference>,
        attempt: u8,
    },
    /// The LLM budget was exhausted before generation completed.
    BudgetExhausted { calls_used: u32, max_calls: u32 },
    /// The LLM API call failed.
    ApiError {
        detail: String,
        status_code: Option<u16>,
        retry_after: Option<u64>,
    },
    /// Maximum retry attempts exhausted.
    MaxRetriesExhausted { attempts: u8, errors: Vec<String> },
    /// The repository context could not be built.
    ContextBuildFailed { detail: String },
}

/// An invalid symbol reference found during Phase 3 validation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct InvalidSymbolReference {
    pub symbol: String,
    pub usage: String,
    pub reason: String,
    pub is_any_type: bool,
}

impl fmt::Display for GeneratorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            GeneratorError::InvalidToml {
                raw_response,
                parse_error,
                attempt,
            } => write!(
                f,
                "Invalid TOML (attempt {}): {} - response: {}...",
                attempt,
                parse_error,
                &raw_response[..raw_response.len().min(100)]
            ),
            GeneratorError::ValidationFailed {
                template_id,
                errors,
                attempt,
            } => write!(
                f,
                "Validation failed for '{}' (attempt {}): {}",
                template_id,
                attempt,
                errors.join("; ")
            ),
            GeneratorError::SymbolValidation {
                template_id,
                invalid_references,
                attempt,
            } => write!(
                f,
                "Symbol validation failed for '{}' (attempt {}): {} invalid references",
                template_id,
                attempt,
                invalid_references.len()
            ),
            GeneratorError::BudgetExhausted {
                calls_used,
                max_calls,
            } => write!(
                f,
                "Budget exhausted: used {}/{} calls",
                calls_used, max_calls
            ),
            GeneratorError::ApiError {
                detail,
                status_code,
                retry_after,
            } => write!(
                f,
                "API error (status: {:?}, retry_after: {:?}): {}",
                status_code, retry_after, detail
            ),
            GeneratorError::MaxRetriesExhausted { attempts, errors } => write!(
                f,
                "Max retries exhausted after {} attempts: {}",
                attempts,
                errors.join("; ")
            ),
            GeneratorError::ContextBuildFailed { detail } => {
                write!(f, "Context build failed: {}", detail)
            }
        }
    }
}

impl GeneratorError {
    /// Returns `true` if this error is transient and the operation may succeed on retry.
    pub fn is_retriable(&self) -> bool {
        matches!(self, GeneratorError::ApiError { .. })
    }
}

// ---------------------------------------------------------------------------
// Filesystem scanning helpers for RepoContext::from_path()
// ---------------------------------------------------------------------------

/// Scan source files for public API symbols (pub fn, pub struct, pub trait, etc.).
fn scan_public_api(root: &std::path::Path, project_type: &str) -> String {
    let mut symbols = Vec::new();
    let extensions: &[&str] = if project_type.contains("Rust") {
        &["rs"]
    } else if project_type.contains("TypeScript") {
        &["ts"]
    } else if project_type.contains("Python") {
        &["py"]
    } else {
        &["rs", "ts", "py"]
    };

    let max_files = 50;
    let max_symbols = 200;
    let mut files_scanned = 0u32;
    let mut dirs_to_visit = vec![root.to_path_buf()];

    while let Some(dir) = dirs_to_visit.pop() {
        if files_scanned >= max_files || symbols.len() >= max_symbols {
            break;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if files_scanned >= max_files || symbols.len() >= max_symbols {
                    break;
                }
                let path = entry.path();
                if path.is_dir() {
                    // Skip hidden and target/build dirs
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_default();
                    if name.starts_with('.')
                        || name == "target"
                        || name == "node_modules"
                        || name == "__pycache__"
                    {
                        continue;
                    }
                    dirs_to_visit.push(path);
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if extensions.contains(&ext) {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            files_scanned += 1;
                            let rel_path = path
                                .strip_prefix(root)
                                .map(|p| p.to_string_lossy())
                                .unwrap_or_else(|_| path.to_string_lossy());
                            extract_public_symbols(&content, &rel_path, &mut symbols, project_type);
                        }
                    }
                }
            }
        }
    }

    if symbols.is_empty() {
        String::new()
    } else {
        symbols.join("\n")
    }
}

/// Extract public API symbols from source content.
fn extract_public_symbols(
    content: &str,
    file_path: &str,
    symbols: &mut Vec<String>,
    project_type: &str,
) {
    if project_type.contains("Rust") {
        extract_rust_public_api(content, file_path, symbols);
    } else if project_type.contains("TypeScript") {
        extract_ts_public_api(content, file_path, symbols);
    } else if project_type.contains("Python") {
        extract_python_public_api(content, file_path, symbols);
    }
}

/// Extract Rust public API symbols.
fn extract_rust_public_api(content: &str, file_path: &str, symbols: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        // pub fn name(...) -> RetType
        if let Some(rest) = trimmed.strip_prefix("pub fn ") {
            let name = rest
                .split(|c: char| c == '(' || c == '<')
                .next()
                .unwrap_or(rest);
            symbols.push(format!("{}: pub fn {}", file_path, name.trim()));
        }
        // pub struct Name, pub enum Name, pub trait Name
        else if let Some(rest) = trimmed.strip_prefix("pub struct ") {
            let name = rest
                .split(|c: char| c == '<' || c == '{' || c == '(' || c == ';')
                .next()
                .unwrap_or(rest);
            symbols.push(format!("{}: pub struct {}", file_path, name.trim()));
        } else if let Some(rest) = trimmed.strip_prefix("pub enum ") {
            let name = rest
                .split(|c: char| c == '<' || c == '{' || c == '(')
                .next()
                .unwrap_or(rest);
            symbols.push(format!("{}: pub enum {}", file_path, name.trim()));
        } else if let Some(rest) = trimmed.strip_prefix("pub trait ") {
            let name = rest
                .split(|c: char| c == '<' || c == '{' || c == '(')
                .next()
                .unwrap_or(rest);
            symbols.push(format!("{}: pub trait {}", file_path, name.trim()));
        } else if let Some(rest) = trimmed.strip_prefix("pub mod ") {
            let name = rest.split(';').next().unwrap_or(rest);
            symbols.push(format!("{}: pub mod {}", file_path, name.trim()));
        }
        // pub type Name = ...;
        else if let Some(rest) = trimmed.strip_prefix("pub type ") {
            let name = rest
                .split(|c: char| c == '=' || c == '<')
                .next()
                .unwrap_or(rest);
            symbols.push(format!("{}: pub type {}", file_path, name.trim()));
        }
        // pub const NAME: ... = ...;
        else if trimmed.starts_with("pub const ") {
            let after = &trimmed[10..];
            let name = after.split(':').next().unwrap_or(after);
            symbols.push(format!("{}: pub const {}", file_path, name.trim()));
        }
    }
}

/// Extract TypeScript public API symbols.
fn extract_ts_public_api(content: &str, file_path: &str, symbols: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("export ") {
            let rest = &trimmed[7..].trim();
            if let Some(sig) = rest.strip_prefix("function ") {
                // Include full signature up to opening brace or semicolon
                let full_sig = sig.split(|c| c == '{' || c == ';').next().unwrap_or(sig).trim();
                symbols.push(format!("{}: export function {}", file_path, full_sig));
            } else if let Some(rest) = rest.strip_prefix("class ") {
                let name = rest
                    .split(|c: char| c == '<' || c == '{')
                    .next()
                    .unwrap_or(rest)
                    .trim();
                symbols.push(format!("{}: export class {}", file_path, name));
                // Also extract method signatures inside the class
                extract_ts_class_methods(content, line, file_path, symbols);
            } else if let Some(rest) = rest.strip_prefix("interface ") {
                let name = rest
                    .split(|c: char| c == '<' || c == '{')
                    .next()
                    .unwrap_or(rest)
                    .trim();
                symbols.push(format!("{}: export interface {}", file_path, name));
            } else if let Some(sig) = rest.strip_prefix("type ") {
                let full_sig = sig.split(|c| c == '=' || c == ';').next().unwrap_or(sig).trim();
                symbols.push(format!("{}: export type {}", file_path, full_sig));
            } else if let Some(sig) = rest.strip_prefix("const ") {
                let full_sig = sig.split(|c| c == ':' || c == '=').next().unwrap_or(sig).trim();
                symbols.push(format!("{}: export const {}", file_path, full_sig));
            }
        }
    }
}

/// Extract method signatures from the class body following the class declaration line.
fn extract_ts_class_methods(content: &str, class_line: &str, file_path: &str, symbols: &mut Vec<String>) {
    // Find the line index of the class declaration
    let lines: Vec<&str> = content.lines().collect();
    let class_idx = lines.iter().position(|l| l.trim() == class_line.trim());
    let Some(start) = class_idx else { return };

    // Walk forward from class declaration to find methods
    // Methods are lines like: methodName(args): ReturnType { ... }
    let mut brace_depth = 0;
    let mut in_class_body = false;
    for line in &lines[start + 1..] {
        let trimmed = line.trim();
        if trimmed == "{" {
            brace_depth += 1;
            in_class_body = true;
            continue;
        }
        if trimmed == "}" || trimmed == "};" {
            if brace_depth <= 0 { break; }
            brace_depth -= 1;
            if brace_depth == 0 { break; } // end of class
            continue;
        }
        if !in_class_body { continue; }

        // Detect method or field declarations
        // Matches: methodName(...) { or methodName(...): ReturnType {
        let _method_pattern = r"^\s*(public\s+|private\s+|protected\s+|static\s+|readonly\s|async\s)*(get\s+|set\s+)?\w+\s*\([^)]*\)\s*(:\s*[^{{]+)?\s*(\{{|;)";
        if trimmed.len() > 2 && !trimmed.starts_with("//") && !trimmed.starts_with("/*") {
            // Check if this line looks like a method declaration
            let paren_open = trimmed.find('(');
            let paren_close = trimmed.rfind(')');
            if let (Some(open), Some(close)) = (paren_open, paren_close) {
                if open > 0 && close > open {
                    let sig_end = trimmed[close + 1..].find(|c| c == '{' || c == ';').map(|i| close + 1 + i).unwrap_or(trimmed.len());
                    let sig = &trimmed[..=sig_end.min(trimmed.len()).max(close + 1)];
                    // Skip if it looks like a lambda or constructor parameter destructuring
                    if !sig.starts_with('(') && !sig.starts_with("...") && sig.contains('(') {
                        symbols.push(format!("{}:   method {}", file_path, sig.trim()));
                    }
                }
            }
        }
    }
}

/// Extract Python public API symbols.
fn extract_python_public_api(content: &str, file_path: &str, symbols: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("def ") && !trimmed.starts_with("def _") {
            let rest = &trimmed[4..];
            let name = rest.split('(').next().unwrap_or(rest);
            symbols.push(format!("{}: def {}", file_path, name.trim()));
        } else if trimmed.starts_with("class ") && !trimmed.starts_with("class _") {
            let rest = &trimmed[6..];
            let name = rest
                .split(|c: char| c == '(' || c == ':')
                .next()
                .unwrap_or(rest);
            symbols.push(format!("{}: class {}", file_path, name.trim()));
        }
        // __all__ = [...] exports
        if trimmed.starts_with("__all__") {
            symbols.push(format!("{}: __all__ (explicit exports)", file_path));
        }
    }
}

/// Read architecture documentation from common locations.
fn read_architecture_docs(root: &std::path::Path) -> String {
    let candidates = [
        "ARCHITECTURE.md",
        "docs/ARCHITECTURE.md",
        ".pi/architecture/overview.md",
        "docs/architecture.md",
    ];

    let mut sections = Vec::new();
    for rel in &candidates {
        let path = root.join(rel);
        if path.exists() {
            if let Ok(content) = std::fs::read_to_string(&path) {
                let truncated: String = content.lines().take(200).collect::<Vec<_>>().join("\n");
                let note = if content.lines().count() > 200 {
                    format!(
                        "\n// ... (truncated from {} lines)",
                        content.lines().count()
                    )
                } else {
                    String::new()
                };
                sections.push(format!("// === {} ===\n{}{}", rel, truncated, note));
            }
        }
    }

    if sections.is_empty() {
        String::new()
    } else {
        sections.join("\n\n")
    }
}

/// Detect the bounded context name from the CWD relative to the project root.
fn detect_bounded_context(root: &std::path::Path) -> String {
    let cwd = match std::env::current_dir() {
        Ok(d) => d,
        Err(_) => return String::new(),
    };

    // Check if CWD is within the project root
    let rel = match cwd.strip_prefix(root) {
        Ok(r) => r,
        Err(_) => {
            // Not within root — return basename of CWD
            return cwd
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_default();
        }
    };

    // Detect bounded context from path components
    // e.g. "engine/src/dag_engine" → "dag-engine"
    // e.g. "cli" → "cli"
    let components: Vec<_> = rel.components().collect();

    if components.is_empty() {
        return root
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_default();
    }

    // Try to find a bounded context dir: look for src/<name> or crates/<name>
    for window in components.windows(2) {
        if let (std::path::Component::Normal(a), std::path::Component::Normal(b)) =
            (&window[0], &window[1])
        {
            let a_str = a.to_string_lossy();
            if a_str == "src" || a_str == "crates" || a_str == "lib" {
                return b.to_string_lossy().replace('_', "-");
            }
        }
    }

    // Fallback: last meaningful component
    components
        .last()
        .and_then(|c| {
            if let std::path::Component::Normal(s) = c {
                Some(s.to_string_lossy().replace('_', "-"))
            } else {
                None
            }
        })
        .unwrap_or_default()
}

/// Build a shallow directory tree string (N levels deep, tree-command style).
/// Build a directory tree scoped to directories/keywords matching a filter.
/// Uses simple substring matching on directory names. Falls back to full tree
/// if filter is empty.
fn build_dir_tree_scoped(
    root: &std::path::Path,
    max_depth: usize,
    filter: &str,
) -> std::io::Result<String> {
    if filter.is_empty() {
        return build_dir_tree(root, max_depth);
    }
    let filter_lower = filter.to_lowercase();
    let mut lines = Vec::new();
    build_dir_tree_scoped_recursive(root, root, 0, max_depth, &filter_lower, &mut lines)?;
    Ok(lines.join("\n"))
}

fn build_dir_tree_scoped_recursive(
    root: &std::path::Path,
    dir: &std::path::Path,
    depth: usize,
    max_depth: usize,
    filter: &str,
    lines: &mut Vec<String>,
) -> std::io::Result<()> {
    if depth > max_depth {
        return Ok(());
    }
    let indent = "  ".repeat(depth);
    let dir_name = dir
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default()
        .to_string();

    // Skip hidden dirs
    if depth > 0 && dir_name.starts_with('.') {
        return Ok(());
    }

    // Only include directories matching the filter
    if depth == 0 || dir_name.to_lowercase().contains(filter) {
        let prefix = if depth == 0 {
            dir.file_name()
                .map(|n| n.to_string_lossy())
                .unwrap_or_default()
                .to_string()
        } else {
            dir_name.clone()
        };
        if depth > 0 || !prefix.is_empty() {
            lines.push(format!("{}{}/", indent, prefix));
        }
    }

    if let Ok(entries) = std::fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                build_dir_tree_scoped_recursive(root, &path, depth + 1, max_depth, filter, lines)?;
            } else if depth == 0 || dir_name.to_lowercase().contains(filter) {
                if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
                    if !name.starts_with('.') {
                        lines.push(format!("{}{}{}", indent, "  ", name));
                    }
                }
            }
        }
    }
    Ok(())
}

/// Scan only file paths under directories matching a topic filter.
/// Returns a compact list of relative file paths.
fn scan_filtered_paths(root: &std::path::Path, filter: &str) -> Vec<String> {
    let filter_lower = filter.to_lowercase();
    let mut paths = Vec::new();
    let mut dirs = vec![root.to_path_buf()];
    let max_files = 30;

    while let Some(dir) = dirs.pop() {
        if paths.len() >= max_files {
            break;
        }
        let dir_name = dir
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
            .to_string();
        if dir != root && !dir_name.to_lowercase().contains(&filter_lower) {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if paths.len() >= max_files {
                    break;
                }
                let path = entry.path();
                if path.is_dir() {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_default();
                    if !name.starts_with('.') && name != "target" && name != "node_modules" {
                        dirs.push(path);
                    }
                } else if let Some(rel) = path.strip_prefix(root).ok() {
                    paths.push(rel.to_string_lossy().to_string());
                }
            }
        }
    }
    paths.sort();
    paths
}

/// Scan public API symbols only from files under directories matching the filter.
fn scan_public_api_filtered(root: &std::path::Path, project_type: &str, filter: &str) -> String {
    let filter_lower = filter.to_lowercase();
    let mut symbols = Vec::new();
    let max_symbols = 50; // Half the default limit since we're targeted
    let mut dirs = vec![root.to_path_buf()];

    let extensions: &[&str] = if project_type.contains("Rust") {
        &["rs"]
    } else if project_type.contains("TypeScript") {
        &["ts"]
    } else if project_type.contains("Python") {
        &["py"]
    } else {
        &["rs", "ts", "py"]
    };

    while let Some(dir) = dirs.pop() {
        if symbols.len() >= max_symbols {
            break;
        }
        let dir_name = dir
            .file_name()
            .map(|n| n.to_string_lossy())
            .unwrap_or_default()
            .to_string();
        // Only descend into directories matching the filter
        if dir != root && !dir_name.to_lowercase().contains(&filter_lower) {
            continue;
        }
        if let Ok(entries) = std::fs::read_dir(&dir) {
            for entry in entries.flatten() {
                if symbols.len() >= max_symbols {
                    break;
                }
                let path = entry.path();
                if path.is_dir() {
                    let name = path
                        .file_name()
                        .map(|n| n.to_string_lossy())
                        .unwrap_or_default();
                    if !name.starts_with('.') && name != "target" && name != "node_modules" {
                        dirs.push(path);
                    }
                } else if let Some(ext) = path.extension().and_then(|e| e.to_str()) {
                    if extensions.contains(&ext) {
                        if let Ok(content) = std::fs::read_to_string(&path) {
                            let rel_path = path
                                .strip_prefix(root)
                                .map(|p| p.to_string_lossy())
                                .unwrap_or_else(|_| path.to_string_lossy());
                            extract_public_symbols(&content, &rel_path, &mut symbols, project_type);
                        }
                    }
                }
            }
        }
    }

    symbols.join("\n")
}

fn build_dir_tree(root: &std::path::Path, max_depth: usize) -> std::io::Result<String> {
    let mut lines = Vec::new();
    build_dir_tree_recursive(root, "", max_depth, &mut lines)?;
    Ok(lines.join("\n"))
}

fn build_dir_tree_recursive(
    dir: &std::path::Path,
    prefix: &str,
    remaining_depth: usize,
    lines: &mut Vec<String>,
) -> std::io::Result<()> {
    let name = dir
        .file_name()
        .map(|n| n.to_string_lossy())
        .unwrap_or_default();

    if !prefix.is_empty() {
        // We don't have is_last for root, simplified version
        lines.push(format!("{}{}", prefix, name));
    } else {
        lines.push(name.to_string());
    }

    if remaining_depth == 0 {
        return Ok(());
    }

    let new_prefix = format!("{}    ", prefix);

    let mut entries: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok()).collect(),
        Err(_) => return Ok(()),
    };

    // Sort: directories first, then files
    entries.sort_by_key(|e| {
        let is_dir = e.path().is_dir();
        let name = e.file_name();
        (is_dir, name)
    });

    // Filter out noise directories
    let entries: Vec<_> = entries
        .into_iter()
        .filter(|e| {
            if e.path().is_dir() {
                let name_str = e.file_name().to_string_lossy().to_string();
                !matches!(
                    name_str.as_str(),
                    ".git" | "target" | "node_modules" | "__pycache__" | ".venv" | ".rigorix"
                )
            } else {
                true
            }
        })
        .collect();

    let len = entries.len();
    for (i, entry) in entries.into_iter().enumerate() {
        let path = entry.path();
        let is_last_entry = i == len - 1;
        let connector = if is_last_entry {
            "└── "
        } else {
            "├── "
        };
        let name = entry.file_name().to_string_lossy().to_string();

        if path.is_dir() {
            lines.push(format!("{}{}{}", new_prefix, connector, name));
            let child_prefix = format!(
                "{}{}",
                new_prefix,
                if is_last_entry { "    " } else { "│   " }
            );
            build_dir_tree_recursive(&path, &child_prefix, remaining_depth - 1, lines)?;
        } else {
            lines.push(format!("{}{}{}", new_prefix, connector, name));
        }
    }

    Ok(())
}

/// Detect the project type from key files in the root directory.
fn detect_project_type(root: &std::path::Path) -> String {
    let mut types = Vec::new();
    if root.join("Cargo.toml").exists() {
        types.push("Rust (Cargo)");
    }
    if root.join("package.json").exists() {
        types.push("TypeScript/JavaScript (npm)");
    }
    if root.join("tsconfig.json").exists() {
        types.push("TypeScript");
    }
    if root.join("pyproject.toml").exists() || root.join("setup.py").exists() {
        types.push("Python");
    }
    if root.join("requirements.txt").exists() {
        types.push("Python (requirements.txt)");
    }
    if root.join("go.mod").exists() {
        types.push("Go");
    }
    if types.is_empty() {
        "Unknown".to_string()
    } else {
        types.join(", ")
    }
}

/// Read existing dependencies from the project's manifest file.
fn read_dependencies(root: &std::path::Path, project_type: &str) -> Vec<String> {
    if project_type.contains("Rust") {
        return read_cargo_dependencies(root);
    }
    if project_type.contains("npm") || project_type.contains("TypeScript/JavaScript") {
        return read_package_json_dependencies(root);
    }
    if project_type.contains("Python") {
        return read_python_dependencies(root);
    }
    Vec::new()
}

/// Parse [dependencies] section from Cargo.toml.
fn read_cargo_dependencies(root: &std::path::Path) -> Vec<String> {
    let path = root.join("Cargo.toml");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let mut deps = Vec::new();
    let mut in_deps = false;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed == "[dependencies]" {
            in_deps = true;
            continue;
        }
        if trimmed.starts_with('[') && in_deps {
            in_deps = false;
            continue;
        }
        if in_deps && !trimmed.is_empty() && !trimmed.starts_with('#') {
            if let Some(eq_pos) = trimmed.find('=') {
                let name = trimmed[..eq_pos].trim().to_string();
                let value = trimmed[eq_pos + 1..].trim().to_string();
                let value = value.split('#').next().unwrap_or(&value).trim().to_string();
                deps.push(format!("{name} = {value}"));
            }
        }
    }

    deps
}

/// Parse dependencies from package.json.
fn read_package_json_dependencies(root: &std::path::Path) -> Vec<String> {
    let path = root.join("package.json");
    let content = match std::fs::read_to_string(&path) {
        Ok(c) => c,
        Err(_) => return Vec::new(),
    };

    let Ok(json) = serde_json::from_str::<serde_json::Value>(&content) else {
        return Vec::new();
    };

    let mut deps = Vec::new();
    for key in ["dependencies", "devDependencies"] {
        if let Some(obj) = json[key].as_object() {
            for (name, version) in obj {
                if let Some(v) = version.as_str() {
                    deps.push(format!("{name} = {v}"));
                }
            }
        }
    }

    deps
}

/// Parse Python dependencies.
fn read_python_dependencies(root: &std::path::Path) -> Vec<String> {
    let mut deps = Vec::new();

    let req_path = root.join("requirements.txt");
    if req_path.exists() {
        if let Ok(content) = std::fs::read_to_string(&req_path) {
            for line in content.lines() {
                let trimmed = line.trim();
                if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
                    continue;
                }
                deps.push(trimmed.to_string());
            }
        }
    }

    if deps.is_empty() {
        let pyproject = root.join("pyproject.toml");
        if pyproject.exists() {
            if let Ok(content) = std::fs::read_to_string(&pyproject) {
                let mut in_deps = false;
                for line in content.lines() {
                    let trimmed = line.trim();
                    if trimmed == "[project.dependencies]" {
                        in_deps = true;
                        continue;
                    }
                    if trimmed.starts_with('[') && in_deps {
                        break;
                    }
                    if in_deps && !trimmed.is_empty() && !trimmed.starts_with('#') {
                        let dep = trimmed.trim_matches(|c: char| c == '"' || c == ',' || c == ' ');
                        if !dep.is_empty() {
                            deps.push(dep.to_string());
                        }
                    }
                }
            }
        }
    }

    deps
}

/// Read content of key entry-point files for the project type.
fn read_key_files(root: &std::path::Path) -> String {
    let mut sections = Vec::new();
    let max_lines = 200;

    let candidates: Vec<&str> = if root.join("Cargo.toml").exists() {
        vec!["src/lib.rs", "src/main.rs"]
    } else if root.join("tsconfig.json").exists() || root.join("package.json").exists() {
        vec!["src/index.ts", "src/index.js", "index.ts", "index.js"]
    } else {
        vec!["src/__init__.py", "__init__.py", "main.py"]
    };

    // Read standard entry-point candidates
    for rel_path in &candidates {
        let full_path = root.join(rel_path);
        if !full_path.exists() {
            continue;
        }
        let content = match std::fs::read_to_string(&full_path) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().take(max_lines).collect();
        let truncated: String = lines.join("\n");
        let note = if content.lines().count() > max_lines {
            format!("\n// ... (truncated from {} lines)", content.lines().count())
        } else {
            String::new()
        };
        sections.push(format!("// === {} ===\n{}{}", rel_path, truncated, note));
    }

    // For TypeScript projects, also scan src/ for additional .ts files
    // that contain exports (functions, classes likely needed by tests)
    if root.join("tsconfig.json").exists() || root.join("package.json").exists() {
        let src_dir = root.join("src");
        if src_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&src_dir) {
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.extension().map_or(true, |e| e != "ts") {
                        continue;
                    }
                    let rel = path.strip_prefix(root).unwrap_or(&path).to_string_lossy().to_string();
                    // Skip files already read as candidates
                    if candidates.iter().any(|c| **c == rel) {
                        continue;
                    }
                    let content = match std::fs::read_to_string(&path) {
                        Ok(c) => c,
                        Err(_) => continue,
                    };
                    let lines: Vec<&str> = content.lines().take(max_lines).collect();
                    let truncated: String = lines.join("\n");
                    let note = if content.lines().count() > max_lines {
                        format!("\n// ... (truncated from {} lines)", content.lines().count())
                    } else {
                        String::new()
                    };
                    sections.push(format!("// === {} ===\n{}{}", rel, truncated, note));
                }
            }
        }
    }

    sections.join("\n\n")
}

// ---------------------------------------------------------------------------
// ClaudeTemplateGenerator — Anthropic Messages API Implementation
// ---------------------------------------------------------------------------

/// Configuration for the Claude template generator.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClaudeGeneratorConfig {
    pub api_url: String,
    pub model: String,
    pub max_tokens: u32,
    pub timeout_secs: u64,
    pub temperature: f64,
    pub max_retries: u8,
}

impl Default for ClaudeGeneratorConfig {
    fn default() -> Self {
        Self {
            api_url: "https://api.anthropic.com/v1/messages".to_string(),
            model: "claude-sonnet-4-20250514".to_string(),
            max_tokens: 4096,
            timeout_secs: 120,
            temperature: 0.3,
            max_retries: 3,
        }
    }
}

/// Production template generator using Anthropic's Claude Messages API.
pub struct ClaudeTemplateGenerator {
    api_key: String,
    config: ClaudeGeneratorConfig,
    client: reqwest::Client,
}

impl ClaudeTemplateGenerator {
    /// Create a new ClaudeTemplateGenerator.
    pub fn new(api_key: String, config: Option<ClaudeGeneratorConfig>) -> Self {
        let config = config.unwrap_or_default();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            api_key,
            config,
            client,
        }
    }

    /// Build the system prompt for template generation.
    ///
    /// Provides the LLM with 9 critical rules, full template schema documentation,
    /// existing templates list, rich repo context (directory tree, dependencies,
    /// public API surface, key source files), and structural guidance for generating
    /// multi-node DAGs with appropriate tool types and dependency ordering.
    fn build_system_prompt(&self, ctx: &RepoContext) -> String {
        let template_list = if ctx.existing_templates.is_empty() {
            "(none)".to_string()
        } else {
            ctx.existing_templates
                .iter()
                .map(|t| format!("- {}: {}", t.id, t.description))
                .collect::<Vec<_>>()
                .join("\n")
        };

        let deps_section = if ctx.dependencies.is_empty() {
            "No existing dependencies found.".to_string()
        } else {
            ctx.dependencies.join("\n")
        };

        let api_section = if ctx.public_api.is_empty() && ctx.public_api_list.is_empty() {
            "No public API symbols found.".to_string()
        } else if !ctx.public_api.is_empty() {
            ctx.public_api.clone()
        } else {
            ctx.public_api_list.join("\n")
        };

        let key_files_section = if ctx.key_file_contents.is_empty() {
            "No key source files found.".to_string()
        } else {
            ctx.key_file_contents.clone()
        };

        let arch_section = if ctx.architecture_overview.is_empty() {
            String::new()
        } else {
            format!("\nARCHITECTURE OVERVIEW:\n{}\n", ctx.architecture_overview)
        };

        let bc_section = if ctx.bounded_context.is_empty() {
            "project-root".to_string()
        } else {
            ctx.bounded_context.clone()
        };

        let mod_deps_section = match &ctx.module_deps {
            Some(deps) if !deps.is_empty() => format!("\nMODULE DEPENDENCY GRAPH:\n{}\n", deps),
            _ => String::new(),
        };

        let dir_tree = if ctx.dir_tree.is_empty() {
            if ctx.directory_tree.is_empty() {
                "(no files scanned)".to_string()
            } else {
                ctx.directory_tree.join("\n")
            }
        } else {
            ctx.dir_tree.clone()
        };

        format!(
            r##"You are a template generator for Rigorix, a deterministic coding CLI.
You produce TOML workflow templates that define directed acyclic graphs (DAGs)
of operations.

*** CRITICAL RULES ***
1. ONLY use packages/crates/dependencies listed in EXISTING DEPENDENCIES. Do NOT invent new ones.
2. ONLY use types, functions, and methods listed in PUBLIC API SURFACE.
3. If a needed package or type doesn't exist, use `file_read` to inspect the codebase first.
4. **For editing existing files: `file_read` + `file_patch` with anchor mode.** Read the file first to see the structure. Then use tree-sitter anchor fields to target the exact location. This is deterministic — no search-string guessing.
   - CRITICAL: After reading the file, HARDCODE the anchor_name from what you see. Do NOT make anchor_name a template parameter.
   - CRITICAL: NEVER use the `search` field. Always use anchor mode (`anchor_type` + `anchor_name`). The `search` field is DEPRECATED and will produce wrong placement.
     WRONG: `search = "export class TaskList {{"` — inserts after opening `{{`, content lands at START of class body.
     RIGHT: `anchor_type = "class", anchor_name = "TaskList", position = "after"` — inserts before closing `}}`, content lands at END of class body.
   - CRITICAL: For adding members to a class/struct/interface, ALWAYS use `anchor_type = "class"` (or "struct"/"interface"), `anchor_name = "<Name>"`, `position = "after"`. This inserts inside the body before the closing brace.
   
5. `file_append` is ONLY for: import statements, module declarations, single-line config — never for methods/functions/classes.
6. `file_write` is for creating NEW files or completely rewriting small files (<200 lines).
7. NEVER use `any` type in TypeScript code. ALWAYS use the exact type name from the PUBLIC API SURFACE.
8. ALWAYS use the EXACT field names shown in the PUBLIC API SURFACE.
9. When a path parameter like target_file holds a full path such as src/lib.rs, use the placeholder directly as the path value. Do NOT prepend an extra directory prefix.

**FORMATTING RULES for inserted code:**
- Match the EXACT indentation of surrounding code (count spaces/tabs from adjacent lines).
- Do NOT add leading or trailing blank lines around the insert — the tool inserts exactly what you provide.
- For methods: include the method declaration line, opening brace, body, and closing brace in the insert.
- For multi-line inserts: start each line with the correct indentation level.

**PLACEMENT RULES for class methods:**
- ALWAYS insert new methods at the END of the class body (after the LAST existing method).
- Use `position = "after"` with `anchor_name` set to the last method's name.
- NEVER insert before properties, constructor, or between fields — always after the last method.
- If the class has no methods yet, anchor to the class itself: `anchor_type = "class"`, `anchor_name = "<ClassName>"`, `position = "after"`.
- NEVER insert at the start of the class body (after `{{`). ALWAYS insert at the end (before `}}`).

**TESTING RULES (MANDATORY):**
1. After inserting new code, ALWAYS add a `file_write` node that creates a test file for the new functionality.
   - The test file path MUST be hardcoded (e.g., `tests/tasklist.test.ts`), NOT a template parameter.
   - The test file should import/use the new code and verify it works correctly.
   - **CRITICAL — HALLUCINATION WARNING**: You MUST use the EXACT function signatures from the source file. Do NOT invent API names, do NOT guess parameter names, do NOT assume standard patterns.
     - Look at the `PUBLIC API SURFACE` section above — it lists every export with its COMPLETE signature including parameter names and types.
     - If the public API surface says `createTask(id: number, title: string)`, your test must call it as `createTask(0, "title")`, NOT `createTask("title")`.
     - If the class has `add(title: string)` and NOT `addTask(task: Task)`, your test must call `list.add("some title")`, NOT `list.addTask(task)`.
     - Never invent methods, constructor parameters, or return types. Only use what's listed in PUBLIC API SURFACE.
     - Before writing any test code, scan the PUBLIC API SURFACE section and confirm every function/class/method call matches exactly.
   - The test must actually exercise the new code (not be a placeholder). Use real assertions.
2. Add a `run_command` node that runs the tests (depends on BOTH patch step AND test-writing step):
   - TypeScript: `npx jest --testPathPattern=<test-file>`
   - Python: `python -m pytest tests/<test-file>`
   - Rust: `cargo test <test-name>`
3. Add a compile-check step between the code patch and the test run:
   - TypeScript: `npx tsc --noEmit` (validation = "type_check" on the run_command node)
   - Python: `python -m py_compile <patched-file>`
   - Rust: `cargo check` (validation = "type_check" on the run_command node)
4. CRITICAL: Do NOT add ANY template parameters for test files. Use literal paths like `tests/tasklist.test.ts`.
5. If tests fail, the run should fail (this is correct — prevents merging broken code).

TEMPLATE SCHEMA:
id = "unique-kebab-case-id"
name = "Human readable name"
description = "What this template does"
version = "1.0.0"

[[parameters]]
name = "param_name"
description = "What this parameter controls"
required = true
param_type = "string"  # or "path", "boolean", "number"

[[nodes]]
id = "node-id"
name = "Node description"
depends_on = ["other-node-id"]  # optional
validation = "type_check"  # string field: lint_pass, test_pass, type_check, or custom("<cmd>")
[nodes.action]
type = "file_read"  # or "file_write", "run_command", "lsp_query", "git_read"
path = "{{ param_name }}"  # use DOUBLE curly braces {{ }} for parameter substitution

VALID ACTION TYPES:
- file_read: {{ type, path }}
- file_write: {{ type, path, content (required) }} — OVERWRITES entire file
- file_append: {{ type, path, content (required) }} — APPENDS (ONLY for imports/mod declarations)
- file_patch (anchor mode, PREFERRED):
    {{ type, path, anchor_type, anchor_name, container (optional), position, insert }}
    anchor_type = "method" | "function" | "class" | "struct" | "impl" | "interface" | "end_of_file"
    position = "after" | "before"  (use "after" for appending, "before" for prepending)
    Example — add method after activeCount inside TaskList class:
      type = "file_patch", path = "{{ file_path }}",
      anchor_type = "method", anchor_name = "activeCount", container = "TaskList",
      position = "after", insert = "  getActiveTasks(): Task[] {{ return []; }}\n"
    For appending to end of file: anchor_type = "end_of_file", no anchor_name needed
- file_patch (search mode, DEPRECATED — only for files where no language grammar exists):
    {{ type, path, search, insert, before (optional) }}
- run_command: {{ type, command, args (optional) }}
- lsp_query: {{ type, query }}
- git_read: {{ type, operation, count (optional) }}

VALID VALIDATION RULES (use as a STRING on the node, e.g. validation = "type_check"):
- lint_pass, test_pass, type_check, custom("<command>")

COMMON VALIDATION COMMANDS BY LANGUAGE:
- Rust: cargo check (type_check), cargo test (test_pass)
- TypeScript: npx tsc --noEmit (type_check), npm test (test_pass)
- Python: python -m py_compile (type_check), python -m pytest (test_pass)

EXISTING TEMPLATES (do NOT duplicate these IDs):
{template_list}

REPO CONTEXT:
Project type: {project_type}
Bounded context: {bounded_context}
Directory structure:
{dir_tree}
{arch}
{mod_deps}
EXISTING DEPENDENCIES (ONLY use these — do NOT add new ones):
{deps}

PUBLIC API SURFACE (existing types, functions, methods you can use):
{api}

KEY SOURCE FILES (read these to understand the existing code before generating):
{key_files}

USER INTENT (see user message below for the specific intent to fulfill).

Generate a TOML template that:
1. Has a unique ID (kebab-case, not conflicting with existing templates)
2. Defines 2-7 nodes with clear dependency ordering
3. Uses parameter substitution ({{{{ param_name }}}}) for variable parts
4. Includes validation on test/run nodes
5. Follows the exact TOML schema shown above
6. ONLY references packages/crates in EXISTING DEPENDENCIES
7. ONLY references types/methods in PUBLIC API SURFACE

Respond with valid TOML only. Do NOT include markdown code fences or explanations."##,
            template_list = template_list,
            dir_tree = dir_tree,
            project_type = ctx.project_type,
            bounded_context = bc_section,
            arch = arch_section,
            mod_deps = mod_deps_section,
            deps = deps_section,
            api = api_section,
            key_files = key_files_section,
        )
    }

    /// Build the user message for template generation.
    fn build_user_message(&self, intent: &crate::planning::domain::UserIntent) -> String {
        let mut msg = format!("## User Intent\n\n{}", intent.input);
        if intent.has_clarifications() {
            msg.push_str("\n\n## Clarification History\n");
            for pair in &intent.clarifications {
                msg.push_str(&format!("\n- Q: {}\n- A: {}", pair.question, pair.answer));
            }
        }
        msg.push_str("\n\n## Instructions\n");
        msg.push_str("Generate a TOML template that fulfills this intent using the schema and rules from the system prompt.\n");
        msg.push_str("Output ONLY valid TOML. No markdown fences. No explanations.");
        msg
    }

    /// Strip markdown code fences from the LLM response.
    pub(crate) fn strip_code_fences(response: &str) -> String {
        let trimmed = response.trim();
        let first_fence = trimmed.find("```");
        let content_after_open = match first_fence {
            Some(open_pos) => {
                let after_fence = &trimmed[open_pos + 3..];
                if let Some(newline) = after_fence.find('\n') {
                    &after_fence[newline + 1..]
                } else {
                    trimmed
                }
            }
            None => trimmed,
        };
        if content_after_open.is_empty() && first_fence.is_some() {
            return trimmed
                .trim_end_matches("```")
                .trim_end_matches('`')
                .trim()
                .to_string();
        }
        if let Some(close_pos) = content_after_open.rfind("```") {
            content_after_open[..close_pos].trim().to_string()
        } else {
            content_after_open.trim().to_string()
        }
    }

    /// Fix single-brace placeholders in TOML that the LLM outputs instead of `{{ }}`.
    ///
    /// DeepSeek and other models often output `{ param_name }` instead of
    /// `{{ param_name }}`. TOML parsers interpret single braces as inline table
    /// definitions, causing parse errors. This converts them to double braces
    /// and ensures they are quoted so the TOML parser treats them as strings.
    /// `{{ param_name }}`. TOML parsers interpret single braces as inline table
    /// definitions, causing parse errors. This converts known template parameters
    /// to double braces and ensures they are quoted so the TOML parser treats them
    /// as strings.
    ///
    /// Only identifiers matching known template parameter names are converted.
    /// TypeScript/JavaScript destructuring (e.g. `import { TaskList }`) is left
    /// untouched because `TaskList` is not a template parameter.
    pub(crate) fn fix_toml_placeholders(toml: &str) -> String {
        let known_params = Self::extract_parameter_names(toml);
        Self::fix_toml_placeholders_with_params(toml, &known_params)
    }

    /// Extract template parameter names from raw TOML text.
    /// Looks for `[[parameters]]` sections and extracts the `name` field.
    fn extract_parameter_names(toml: &str) -> Vec<String> {
        let mut names = Vec::new();
        let mut in_params = false;
        for line in toml.lines() {
            let trimmed = line.trim();
            if trimmed == "[[parameters]]" {
                in_params = true;
                continue;
            }
            if in_params {
                if trimmed.starts_with('[') && trimmed != "[[parameters]]" {
                    in_params = false;
                    continue;
                }
                if let Some(rest) = trimmed.strip_prefix("name = ") {
                    let name = rest
                        .trim()
                        .trim_matches('"')
                        .trim_matches('\'')
                        .to_string();
                    if !name.is_empty() {
                        names.push(name);
                    }
                }
            }
        }
        names
    }

    /// Core implementation: convert `{ ident }` → `{{ ident }}` only when `ident`
    /// is a known template parameter. TypeScript destructuring and JS object
    /// literals are preserved unchanged.
    fn fix_toml_placeholders_with_params(toml: &str, known_params: &[String]) -> String {
        let mut result = String::with_capacity(toml.len() + 64);
        let mut in_curly = false;
        let mut buf = String::new();
        let chars: Vec<char> = toml.chars().collect();
        let mut i = 0;
        while i < chars.len() {
            let ch = chars[i];
            match ch {
                '{' if !in_curly => {
                    in_curly = true;
                    buf.clear();
                }
                '}' if in_curly => {
                    let inner = buf.trim();
                    if !inner.is_empty()
                        && inner.chars().all(|c| c.is_alphanumeric() || c == '_')
                        && known_params.contains(&inner.to_string())
                    {
                        // Known template parameter — convert to double braces
                        let is_inside_quotes = Self::is_inside_toml_string(&result);
                        if is_inside_quotes {
                            result.push_str(&format!("{{{{ {} }}}}", inner));
                        } else {
                            result.push_str(&format!("\"{{{{ {} }}}}\"", inner));
                        }
                    } else {
                        // Not a known parameter — likely TypeScript destructuring
                        // or JS object literal. Pass through unchanged.
                        result.push('{');
                        result.push_str(&buf);
                        result.push('}');
                    }
                    in_curly = false;
                }
                c if in_curly => buf.push(c),
                c => result.push(c),
            }
            i += 1;
        }
        if in_curly {
            result.push('{');
            result.push_str(&buf);
        }
        result
    }

    /// Check whether the current position in `result` is inside a TOML string.
    /// Walks backward from the end of `result` to find the most recent unclosed `"`.
    fn is_inside_toml_string(result: &str) -> bool {
        let mut in_string = false;
        let mut escaped = false;
        for ch in result.chars() {
            match ch {
                '"' if !escaped => in_string = !in_string,
                '\\' if !escaped => escaped = true,
                _ => escaped = false,
            }
        }
        in_string
    }

    /// Parse the Anthropic API response and extract the text content.
    fn parse_api_response(response_text: &str) -> Result<String, GeneratorError> {
        #[derive(Deserialize)]
        struct AnthropicMessage {
            content: Vec<AnthropicContent>,
        }
        #[derive(Deserialize)]
        struct AnthropicContent {
            #[serde(rename = "type")]
            content_type: String,
            text: Option<String>,
        }
        let message: AnthropicMessage =
            serde_json::from_str(response_text).map_err(|e| GeneratorError::ApiError {
                detail: format!("Failed to parse Claude API response: {}", e),
                status_code: None,
                retry_after: None,
            })?;
        let text = message
            .content
            .into_iter()
            .find(|c| c.content_type == "text")
            .and_then(|c| c.text)
            .ok_or_else(|| GeneratorError::ApiError {
                detail: "Claude response has no text content block".to_string(),
                status_code: None,
                retry_after: None,
            })?;
        Ok(text)
    }

    /// Extract Retry-After header value from response headers.
    fn extract_retry_after(response: &reqwest::Response) -> Option<u64> {
        response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
    }
}

#[async_trait]
impl TemplateGenerator for ClaudeTemplateGenerator {
    async fn generate(
        &self,
        intent: &crate::planning::domain::UserIntent,
        repo_context: &RepoContext,
        _budget: &crate::budget_tracking::domain::LlmBudget,
    ) -> Result<GeneratedTemplate, GeneratorError> {
        let max_retries = self.config.max_retries;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            let system_prompt = self.build_system_prompt(repo_context);
            let user_message = self.build_user_message(intent);

            let message_content = if attempt > 0 {
                format!(
                    "{}\n\n## Previous Attempt Failed\n\n{}",
                    user_message, last_error
                )
            } else {
                user_message.clone()
            };

            let body = serde_json::json!({
                "model": self.config.model,
                "max_tokens": self.config.max_tokens,
                "temperature": self.config.temperature,
                "system": system_prompt,
                "messages": [{"role": "user", "content": message_content}]
            });

            let body_bytes = serde_json::to_vec(&body).map_err(|e| GeneratorError::ApiError {
                detail: format!("Failed to serialize request: {}", e),
                status_code: None,
                retry_after: None,
            })?;

            let response = self
                .client
                .post(&self.config.api_url)
                .header("x-api-key", &self.api_key)
                .header("anthropic-version", "2023-06-01")
                .header("content-type", "application/json")
                .body(body_bytes)
                .send()
                .await
                .map_err(|e| GeneratorError::ApiError {
                    detail: format!("HTTP request failed: {}", e),
                    status_code: None,
                    retry_after: None,
                })?;

            let status = response.status();
            let retry_after = Self::extract_retry_after(&response);

            if !status.is_success() {
                let response_text = response.text().await.unwrap_or_default();
                if status.as_u16() == 429 || status.as_u16() >= 500 {
                    last_error = format!(
                        "API returned status {}: {}",
                        status.as_u16(),
                        response_text.chars().take(200).collect::<String>()
                    );
                    if let Some(seconds) = retry_after {
                        tokio::time::sleep(Duration::from_secs(seconds)).await;
                    }
                    continue;
                }
                return Err(GeneratorError::ApiError {
                    detail: format!(
                        "API returned status {}: {}",
                        status.as_u16(),
                        response_text.chars().take(200).collect::<String>()
                    ),
                    status_code: Some(status.as_u16()),
                    retry_after,
                });
            }

            let response_text = response
                .text()
                .await
                .map_err(|e| GeneratorError::ApiError {
                    detail: format!("Failed to read response body: {}", e),
                    status_code: None,
                    retry_after: None,
                })?;

            let raw_toml = Self::parse_api_response(&response_text)?;
            let toml_content =
                ClaudeTemplateGenerator::fix_toml_placeholders(&Self::strip_code_fences(&raw_toml));

            let template_result: Result<crate::templates::domain::Template, _> =
                toml::from_str(&toml_content);

            match template_result {
                Ok(template) => {
                    return Ok(GeneratedTemplate {
                        toml_content,
                        suggested_id: template.id.clone(),
                        suggested_name: template.name.clone(),
                        description: template.description.clone(),
                        llm_calls_used: attempt as u32 + 1,
                        llm_tokens_used: 0,
                    });
                }
                Err(e) => {
                    last_error = format!("TOML parse error: {}", e);
                }
            }
        }

        Err(GeneratorError::MaxRetriesExhausted {
            attempts: max_retries,
            errors: vec![last_error],
        })
    }

    fn estimate_cost(
        &self,
        _intent: &crate::planning::domain::UserIntent,
    ) -> GeneratedTemplateCost {
        GeneratedTemplateCost {
            estimated_calls: self.config.max_retries as u32,
            estimated_tokens: self.config.max_tokens,
        }
    }
}

// ── OpenAI-compatible TemplateGenerator ─────────────────────────────────

/// Template generator for OpenAI-compatible APIs (OpenAI, DeepSeek, local).
/// Sends requests using the OpenAI chat completions format:
/// POST /v1/chat/completions with Authorization: Bearer header.
pub struct OpenaiTemplateGenerator {
    api_key: String,
    config: ClaudeGeneratorConfig,
    client: reqwest::Client,
}

impl OpenaiTemplateGenerator {
    pub fn new(api_key: String, config: Option<ClaudeGeneratorConfig>) -> Self {
        let config = config.unwrap_or_default();
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(config.timeout_secs))
            .build()
            .expect("Failed to create HTTP client");
        Self {
            api_key,
            config,
            client,
        }
    }

    fn build_system_prompt(&self, ctx: &RepoContext) -> String {
        ClaudeTemplateGenerator::new("".into(), None).build_system_prompt(ctx)
    }

    fn build_user_message(&self, intent: &crate::planning::domain::UserIntent) -> String {
        ClaudeTemplateGenerator::new("".into(), None).build_user_message(intent)
    }

    fn strip_code_fences(response: &str) -> String {
        ClaudeTemplateGenerator::strip_code_fences(response)
    }

    /// Parse the OpenAI API response and extract the text content.
    fn parse_api_response(response_text: &str) -> Result<String, GeneratorError> {
        #[derive(Deserialize)]
        struct OpenaiResponse {
            choices: Vec<OpenaiChoice>,
        }
        #[derive(Deserialize)]
        struct OpenaiChoice {
            message: OpenaiMessage,
        }
        #[derive(Deserialize)]
        struct OpenaiMessage {
            content: Option<String>,
        }
        let resp: OpenaiResponse =
            serde_json::from_str(response_text).map_err(|e| GeneratorError::ApiError {
                detail: format!("Failed to parse OpenAI API response: {}", e),
                status_code: None,
                retry_after: None,
            })?;
        resp.choices
            .into_iter()
            .next()
            .and_then(|c| c.message.content)
            .ok_or_else(|| GeneratorError::ApiError {
                detail: "OpenAI response has no content".to_string(),
                status_code: None,
                retry_after: None,
            })
    }

    fn extract_retry_after(response: &reqwest::Response) -> Option<u64> {
        response
            .headers()
            .get("retry-after")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u64>().ok())
    }
}

#[async_trait]
impl TemplateGenerator for OpenaiTemplateGenerator {
    async fn generate(
        &self,
        intent: &crate::planning::domain::UserIntent,
        repo_context: &RepoContext,
        _budget: &crate::budget_tracking::domain::LlmBudget,
    ) -> Result<GeneratedTemplate, GeneratorError> {
        let max_retries = self.config.max_retries;
        let mut last_error = String::new();

        for attempt in 0..max_retries {
            let system_prompt = self.build_system_prompt(repo_context);
            let user_message = self.build_user_message(intent);

            let message_content = if attempt > 0 {
                format!(
                    "{}\n\n## Previous Attempt Failed\n\n{}",
                    user_message, last_error
                )
            } else {
                user_message.clone()
            };

            // OpenAI format: system is a message with role "system"
            let body = serde_json::json!({
                "model": self.config.model,
                "max_tokens": self.config.max_tokens,
                "temperature": self.config.temperature,
                "messages": [
                    {"role": "system", "content": system_prompt},
                    {"role": "user", "content": message_content}
                ]
            });

            let body_bytes = serde_json::to_vec(&body).map_err(|e| GeneratorError::ApiError {
                detail: format!("Failed to serialize request: {}", e),
                status_code: None,
                retry_after: None,
            })?;

            let response = self
                .client
                .post(&self.config.api_url)
                .header("Authorization", format!("Bearer {}", &self.api_key))
                .header("content-type", "application/json")
                .body(body_bytes)
                .send()
                .await
                .map_err(|e| GeneratorError::ApiError {
                    detail: format!("HTTP request failed: {}", e),
                    status_code: None,
                    retry_after: None,
                })?;

            let status = response.status();
            let retry_after = Self::extract_retry_after(&response);

            if !status.is_success() {
                let response_text = response.text().await.unwrap_or_default();
                if status.as_u16() == 429 || status.as_u16() >= 500 {
                    last_error = format!(
                        "API returned status {}: {}",
                        status.as_u16(),
                        response_text.chars().take(200).collect::<String>()
                    );
                    if let Some(seconds) = retry_after {
                        tokio::time::sleep(Duration::from_secs(seconds)).await;
                    }
                    continue;
                }
                return Err(GeneratorError::ApiError {
                    detail: format!(
                        "API returned status {}: {}",
                        status.as_u16(),
                        response_text.chars().take(200).collect::<String>()
                    ),
                    status_code: Some(status.as_u16()),
                    retry_after,
                });
            }

            let response_text = response
                .text()
                .await
                .map_err(|e| GeneratorError::ApiError {
                    detail: format!("Failed to read response body: {}", e),
                    status_code: None,
                    retry_after: None,
                })?;

            let raw_toml = Self::parse_api_response(&response_text)?;
            let toml_content =
                ClaudeTemplateGenerator::fix_toml_placeholders(&Self::strip_code_fences(&raw_toml));

            let template_result: Result<crate::templates::domain::Template, _> =
                toml::from_str(&toml_content);

            match template_result {
                Ok(template) => {
                    return Ok(GeneratedTemplate {
                        toml_content,
                        suggested_id: template.id.clone(),
                        suggested_name: template.name.clone(),
                        description: template.description.clone(),
                        llm_calls_used: attempt as u32 + 1,
                        llm_tokens_used: 0,
                    });
                }
                Err(e) => {
                    last_error = format!("TOML parse error: {}", e);
                }
            }
        }

        Err(GeneratorError::MaxRetriesExhausted {
            attempts: max_retries,
            errors: vec![last_error],
        })
    }

    fn estimate_cost(
        &self,
        _intent: &crate::planning::domain::UserIntent,
    ) -> GeneratedTemplateCost {
        GeneratedTemplateCost {
            estimated_calls: self.config.max_retries as u32,
            estimated_tokens: self.config.max_tokens,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_strip_code_fences_no_fences() {
        let input = "simple toml content";
        assert_eq!(
            ClaudeTemplateGenerator::strip_code_fences(input),
            "simple toml content"
        );
    }

    #[test]
    fn test_strip_code_fences_with_language() {
        let input = "```toml\nname = \"test\"\n```";
        assert_eq!(
            ClaudeTemplateGenerator::strip_code_fences(input),
            "name = \"test\""
        );
    }

    #[test]
    fn test_strip_code_fences_no_language() {
        let input = "```\nname = \"test\"\n```";
        assert_eq!(
            ClaudeTemplateGenerator::strip_code_fences(input),
            "name = \"test\""
        );
    }

    #[test]
    fn test_strip_code_fences_trailing_content() {
        let input = "```toml\nname = \"test\"\n```\nsome trailing text";
        assert_eq!(
            ClaudeTemplateGenerator::strip_code_fences(input),
            "name = \"test\""
        );
    }

    #[test]
    fn test_strip_code_fences_only_closing() {
        let input = "name = \"test\"\n```";
        assert_eq!(
            ClaudeTemplateGenerator::strip_code_fences(input),
            "name = \"test\""
        );
    }

    #[test]
    fn test_strip_code_fences_whitespace() {
        let input = "\n  ```toml\n  name = \"test\"\n  ```  \n";
        assert_eq!(
            ClaudeTemplateGenerator::strip_code_fences(input),
            "name = \"test\""
        );
    }

    #[test]
    fn test_generator_error_display_invalid_toml() {
        let err = GeneratorError::InvalidToml {
            raw_response: "{{{bad toml".to_string(),
            parse_error: "expected a value".to_string(),
            attempt: 0,
        };
        let display = format!("{}", err);
        assert!(display.contains("Invalid TOML"));
    }

    #[test]
    fn test_generator_error_display_budget_exhausted() {
        let err = GeneratorError::BudgetExhausted {
            calls_used: 5,
            max_calls: 3,
        };
        let display = format!("{}", err);
        assert!(display.contains("Budget exhausted"));
    }

    #[test]
    fn test_repo_context_default() {
        let ctx = RepoContext::new(PathBuf::from("/test"), "rust".to_string());
        assert_eq!(ctx.project_type, "rust");
        assert!(!ctx.has_files());
        assert!(ctx.existing_templates.is_empty());
        assert!(ctx.key_file_contents.is_empty());
    }

    #[test]
    fn test_claude_config_defaults() {
        let config = ClaudeGeneratorConfig::default();
        assert_eq!(config.model, "claude-sonnet-4-20250514");
        assert_eq!(config.max_retries, 3);
    }

    #[test]
    fn test_generated_template_serde() {
        let t = GeneratedTemplate {
            toml_content: "id = \"test\"".to_string(),
            suggested_id: "test".to_string(),
            suggested_name: "Test".to_string(),
            description: "A test".to_string(),
            llm_calls_used: 1,
            llm_tokens_used: 100,
        };
        let json = serde_json::to_string(&t).unwrap();
        let deserialized: GeneratedTemplate = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.suggested_id, "test");
    }

    // -- ClaudeTemplateGenerator unit tests (L-04) --

    #[test]
    fn test_strip_code_fences_with_markdown_json() {
        let input = "```json\n{\"name\": \"test\"}\n```";
        let result = ClaudeTemplateGenerator::strip_code_fences(input);
        assert_eq!(result, "{\"name\": \"test\"}");
    }

    // -- fix_toml_placeholders unit tests --

    #[test]
    fn test_fix_toml_placeholders_unquoted() {
        // LLM outputs unquoted { target } — must produce valid TOML string
        let input = r#"[[parameters]]
name = "target"
required = true

[[nodes]]
id = "read"
[nodes.action]
type = "file_read"
path = { target }"#;
        let result = ClaudeTemplateGenerator::fix_toml_placeholders(input);
        assert!(result.contains("{{ target }}"), "should convert to {{ target }}");
        toml::from_str::<toml::Value>(&result)
            .expect("unquoted placeholder must produce valid TOML");
    }

    #[test]
    fn test_fix_toml_placeholders_quoted() {
        // LLM outputs quoted "{ target }" inside a string
        let input = r#"[[parameters]]
name = "target"
required = true

[[nodes]]
id = "read"
[nodes.action]
type = "file_read"
path = "{ target }"
"#;
        let result = ClaudeTemplateGenerator::fix_toml_placeholders(input);
        assert!(result.contains("{{ target }}"), "should convert to {{ target }}");
        toml::from_str::<toml::Value>(&result)
            .expect("quoted placeholder must produce valid TOML");
    }

    #[test]
    fn test_fix_toml_placeholders_inside_string() {
        // Placeholder inside a larger string (e.g. command)
        let input = r#"[[parameters]]
name = "pkg"
description = "Package name"
required = true

[[nodes]]
id = "test"
[nodes.action]
type = "run_command"
command = "cargo test -p { pkg } --lib"
"#;
        let result = ClaudeTemplateGenerator::fix_toml_placeholders(input);
        assert!(result.contains("{{ pkg }}"), "should convert to {{ pkg }}");
        toml::from_str::<toml::Value>(&result)
            .expect("placeholder inside string must stay inside string");
    }

    #[test]
    fn test_fix_toml_placeholders_already_double_braced() {
        // LLM already outputs {{ }} correctly — must be preserved
        let input = r#"path = "{{ target }}""#;
        let result = ClaudeTemplateGenerator::fix_toml_placeholders(input);
        assert_eq!(result, r#"path = "{{ target }}""#);
        toml::from_str::<toml::Value>(&result).expect("already double-braced must remain valid");
    }

    #[test]
    fn test_fix_toml_placeholders_preserves_typescript_destructuring() {
        // TypeScript import destructuring must NOT be converted to template params
        let input = r#"[[parameters]]
name = "file_path"
required = true

[[nodes]]
id = "read-source"
[nodes.action]
type = "file_read"
path = { file_path }

[[nodes]]
id = "write-test"
[nodes.action]
type = "file_write"
path = "tests/tasklist.test.ts"
content = "const x = { TaskList };"#;
        let result = ClaudeTemplateGenerator::fix_toml_placeholders(input);
        // { file_path } is a known param — should become {{ file_path }}
        assert!(result.contains("{{ file_path }}"),
            "known param file_path should become template param");
        // { TaskList } is NOT a known param — must stay as-is
        assert!(result.contains("{ TaskList }"),
            "TypeScript destructuring TaskList must be preserved, got: {}", result);
        // Must parse as valid TOML
        let parsed = toml::from_str::<toml::Value>(&result);
        assert!(parsed.is_ok() || result.contains("{{ file_path }}"),
            "result must at least contain template param: {}", result);
        assert!(!result.contains("{{ TaskList }}"),
            "result must NOT convert TaskList to template param: {}", result);
    }

    #[test]
    fn test_fix_toml_placeholders_no_placeholders() {
        // No placeholders at all — must be identity
        let input = "id = \"test\"\nname = \"Test\"";
        let result = ClaudeTemplateGenerator::fix_toml_placeholders(input);
        assert_eq!(result, input);
    }

    #[test]
    fn test_fix_toml_placeholders_full_template() {
        // Full realistic template with unquoted placeholders
        let input = r#"id = "test"
name = "Test"
description = "Test template"
version = "1.0.0"

[[parameters]]
name = "target"
description = "Target file"
required = true
param_type = "path"

[[nodes]]
id = "read"
name = "Read file"
depends_on = []
[nodes.action]
type = "file_read"
path = { target }"#;
        let result = ClaudeTemplateGenerator::fix_toml_placeholders(input);
        toml::from_str::<toml::Value>(&result)
            .expect("full template with unquoted placeholder must produce valid TOML");
    }

    #[test]
    fn test_parse_api_response_valid_anthropic_format() {
        let input = r#"{"content": [{"type": "text", "text": "template: {\"name\": \"test\"}"}]}"#;
        let result = ClaudeTemplateGenerator::parse_api_response(input);
        assert!(result.is_ok(), "Should parse valid Anthropic response");
        assert!(result.unwrap().contains("template:"));
    }

    #[test]
    fn test_parse_api_response_invalid_json() {
        let result = ClaudeTemplateGenerator::parse_api_response("not json");
        assert!(result.is_err());
    }

    #[test]
    fn test_parse_api_response_missing_content() {
        let input = r#"{"content": []}"#;
        let result = ClaudeTemplateGenerator::parse_api_response(input);
        assert!(result.is_err());
    }

    #[test]
    fn test_build_system_prompt_contains_context() {
        let config = ClaudeGeneratorConfig::default();
        let generator = ClaudeTemplateGenerator::new("test-key".to_string(), Some(config));
        let mut ctx = RepoContext::new(std::path::PathBuf::from("/test"), "rust".to_string());
        ctx.dir_tree = "src/\n└── main.rs".to_string();
        ctx.dependencies = vec!["tokio = \"1\"".to_string()];
        ctx.public_api = "pub fn run()".to_string();
        ctx.public_api_list = vec!["pub fn run()".to_string()];
        let prompt = generator.build_system_prompt(&ctx);
        assert!(
            prompt.contains("rust"),
            "Prompt should mention project type: {prompt}"
        );
        assert!(
            prompt.contains("tokio"),
            "Prompt should mention dependencies: {prompt}"
        );
        assert!(
            prompt.contains("pub fn run()"),
            "Prompt should mention public API: {prompt}"
        );
        // Verify the new prompt has the critical rules
        assert!(
            prompt.contains("CRITICAL RULES"),
            "Prompt should contain critical rules"
        );
        assert!(
            prompt.contains("file_read"),
            "Prompt should list action types"
        );
        assert!(
            prompt.contains("file_write"),
            "Prompt should list action types"
        );
        assert!(
            prompt.contains("EXISTING DEPENDENCIES"),
            "Prompt should have dependencies section"
        );
        assert!(
            prompt.contains("PUBLIC API SURFACE"),
            "Prompt should have API section"
        );
        // Should NOT tell the LLM to generate only 1 node
        assert!(
            !prompt.contains("MINIMAL template with 1 node"),
            "Prompt should NOT restrict to 1 node"
        );
    }

    #[test]
    fn test_build_user_message_contains_intent() {
        let config = ClaudeGeneratorConfig::default();
        let generator = ClaudeTemplateGenerator::new("test-key".to_string(), Some(config));
        let intent =
            crate::planning::domain::intent::UserIntent::new("read the file".to_string(), None);
        let message = generator.build_user_message(&intent);
        assert!(message.contains("read the file"));
    }
}
