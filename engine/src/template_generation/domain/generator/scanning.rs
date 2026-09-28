//! Filesystem-scanning helpers for `RepoContext` (extracted from
//! `generator.rs`, #914): public-API extraction, dir trees, dependency
//! reads. `pub(super)` so the parent module can call them.

// ---------------------------------------------------------------------------
// Filesystem scanning helpers for RepoContext::from_path()
// ---------------------------------------------------------------------------

/// Scan source files for public API symbols (pub fn, pub struct, pub trait, etc.).
pub(super) async fn scan_public_api(root: &std::path::Path, project_type: &str) -> String {
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
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            if files_scanned >= max_files || symbols.len() >= max_symbols {
                break;
            }
            let path = entry.path();
            let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
            if is_dir {
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
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && extensions.contains(&ext)
                && let Ok(content) = tokio::fs::read_to_string(&path).await
            {
                files_scanned += 1;
                let rel_path = path
                    .strip_prefix(root)
                    .map(|p| p.to_string_lossy())
                    .unwrap_or_else(|_| path.to_string_lossy());
                extract_public_symbols(&content, &rel_path, &mut symbols, project_type);
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
pub(super) fn extract_public_symbols(
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
pub(super) fn extract_rust_public_api(content: &str, file_path: &str, symbols: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        // pub fn name(...) -> RetType
        if let Some(rest) = trimmed.strip_prefix("pub fn ") {
            let name = rest.split(['(', '<']).next().unwrap_or(rest);
            symbols.push(format!("{}: pub fn {}", file_path, name.trim()));
        }
        // pub struct Name, pub enum Name, pub trait Name
        else if let Some(rest) = trimmed.strip_prefix("pub struct ") {
            let name = rest.split(['<', '{', '(', ';']).next().unwrap_or(rest);
            symbols.push(format!("{}: pub struct {}", file_path, name.trim()));
        } else if let Some(rest) = trimmed.strip_prefix("pub enum ") {
            let name = rest.split(['<', '{', '(']).next().unwrap_or(rest);
            symbols.push(format!("{}: pub enum {}", file_path, name.trim()));
        } else if let Some(rest) = trimmed.strip_prefix("pub trait ") {
            let name = rest.split(['<', '{', '(']).next().unwrap_or(rest);
            symbols.push(format!("{}: pub trait {}", file_path, name.trim()));
        } else if let Some(rest) = trimmed.strip_prefix("pub mod ") {
            let name = rest.split(';').next().unwrap_or(rest);
            symbols.push(format!("{}: pub mod {}", file_path, name.trim()));
        }
        // pub type Name = ...;
        else if let Some(rest) = trimmed.strip_prefix("pub type ") {
            let name = rest.split(['=', '<']).next().unwrap_or(rest);
            symbols.push(format!("{}: pub type {}", file_path, name.trim()));
        }
        // pub const NAME: ... = ...;
        else if let Some(after) = trimmed.strip_prefix("pub const ") {
            let name = after.split(':').next().unwrap_or(after);
            symbols.push(format!("{}: pub const {}", file_path, name.trim()));
        }
    }
}

/// Extract TypeScript public API symbols.
pub(super) fn extract_ts_public_api(content: &str, file_path: &str, symbols: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("export ") {
            let rest = trimmed.strip_prefix("export ").unwrap_or(trimmed).trim();
            if let Some(sig) = rest.strip_prefix("function ") {
                // Include full signature up to opening brace or semicolon
                let full_sig = sig.split(['{', ';']).next().unwrap_or(sig).trim();
                symbols.push(format!("{}: export function {}", file_path, full_sig));
            } else if let Some(rest) = rest.strip_prefix("class ") {
                let name = rest.split(['<', '{']).next().unwrap_or(rest).trim();
                symbols.push(format!("{}: export class {}", file_path, name));
                // Also extract method signatures inside the class
                extract_ts_class_methods(content, line, file_path, symbols);
            } else if let Some(rest) = rest.strip_prefix("interface ") {
                let name = rest.split(['<', '{']).next().unwrap_or(rest).trim();
                symbols.push(format!("{}: export interface {}", file_path, name));
            } else if let Some(sig) = rest.strip_prefix("type ") {
                let full_sig = sig.split(['=', ';']).next().unwrap_or(sig).trim();
                symbols.push(format!("{}: export type {}", file_path, full_sig));
            } else if let Some(sig) = rest.strip_prefix("const ") {
                let full_sig = sig.split([':', '=']).next().unwrap_or(sig).trim();
                symbols.push(format!("{}: export const {}", file_path, full_sig));
            }
        }
    }
}

/// Extract method signatures from the class body following the class declaration line.
pub(super) fn extract_ts_class_methods(
    content: &str,
    class_line: &str,
    file_path: &str,
    symbols: &mut Vec<String>,
) {
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
            if brace_depth <= 0 {
                break;
            }
            brace_depth -= 1;
            if brace_depth == 0 {
                break;
            } // end of class
            continue;
        }
        if !in_class_body {
            continue;
        }

        // Detect method or field declarations
        // Matches: methodName(...) { or methodName(...): ReturnType {
        let _method_pattern = r"^\s*(public\s+|private\s+|protected\s+|static\s+|readonly\s|async\s)*(get\s+|set\s+)?\w+\s*\([^)]*\)\s*(:\s*[^{{]+)?\s*(\{{|;)";
        if trimmed.len() > 2 && !trimmed.starts_with("//") && !trimmed.starts_with("/*") {
            // Check if this line looks like a method declaration
            let paren_open = trimmed.find('(');
            let paren_close = trimmed.rfind(')');
            if let (Some(open), Some(close)) = (paren_open, paren_close)
                && open > 0
                && close > open
            {
                let sig_end = trimmed[close + 1..]
                    .find(['{', ';'])
                    .map(|i| close + 1 + i)
                    .unwrap_or(trimmed.len());
                let sig = &trimmed[..=sig_end.min(trimmed.len()).max(close + 1)];
                // Skip if it looks like a lambda or constructor parameter destructuring
                if !sig.starts_with('(') && !sig.starts_with("...") && sig.contains('(') {
                    symbols.push(format!("{}:   method {}", file_path, sig.trim()));
                }
            }
        }
    }
}

/// Extract Python public API symbols.
pub(super) fn extract_python_public_api(content: &str, file_path: &str, symbols: &mut Vec<String>) {
    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.starts_with("def ") && !trimmed.starts_with("def _") {
            let rest = &trimmed[4..];
            let name = rest.split('(').next().unwrap_or(rest);
            symbols.push(format!("{}: def {}", file_path, name.trim()));
        } else if trimmed.starts_with("class ") && !trimmed.starts_with("class _") {
            let rest = &trimmed[6..];
            let name = rest.split(['(', ':']).next().unwrap_or(rest);
            symbols.push(format!("{}: class {}", file_path, name.trim()));
        }
        // __all__ = [...] exports
        if trimmed.starts_with("__all__") {
            symbols.push(format!("{}: __all__ (explicit exports)", file_path));
        }
    }
}

/// Read architecture documentation from common locations.
pub(super) async fn read_architecture_docs(root: &std::path::Path) -> String {
    let candidates = [
        "ARCHITECTURE.md",
        "docs/ARCHITECTURE.md",
        ".pi/architecture/overview.md",
        "docs/architecture.md",
    ];

    let mut sections = Vec::new();
    for rel in &candidates {
        let path = root.join(rel);
        if tokio::fs::try_exists(&path).await.unwrap_or(false)
            && let Ok(content) = tokio::fs::read_to_string(&path).await
        {
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

    if sections.is_empty() {
        String::new()
    } else {
        sections.join("\n\n")
    }
}

/// Detect the bounded context name from the CWD relative to the project root.
pub(super) fn detect_bounded_context(root: &std::path::Path) -> String {
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
pub(super) async fn build_dir_tree_scoped(
    root: &std::path::Path,
    max_depth: usize,
    filter: &str,
) -> std::io::Result<String> {
    if filter.is_empty() {
        return build_dir_tree(root, max_depth).await;
    }
    let filter_lower = filter.to_lowercase();
    let mut lines = Vec::new();
    build_dir_tree_scoped_recursive(root, root, 0, max_depth, &filter_lower, &mut lines).await?;
    Ok(lines.join("\n"))
}

#[allow(clippy::only_used_in_recursion)]
pub(super) fn build_dir_tree_scoped_recursive<'a>(
    root: &'a std::path::Path,
    dir: &'a std::path::Path,
    depth: usize,
    max_depth: usize,
    filter: &'a str,
    lines: &'a mut Vec<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
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

        let mut entries = match tokio::fs::read_dir(dir).await {
            Ok(rd) => rd,
            Err(_) => return Ok(()),
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                build_dir_tree_scoped_recursive(root, &path, depth + 1, max_depth, filter, lines)
                    .await?;
            } else if (depth == 0 || dir_name.to_lowercase().contains(filter))
                && let Some(name) = path.file_name().and_then(|n| n.to_str())
                && !name.starts_with('.')
            {
                lines.push(format!("{}{}{}", indent, "  ", name));
            }
        }
        Ok(())
    })
}

/// Scan only file paths under directories matching a topic filter.
/// Returns a compact list of relative file paths.
pub(super) async fn scan_filtered_paths(root: &std::path::Path, filter: &str) -> Vec<String> {
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
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            if paths.len() >= max_files {
                break;
            }
            let path = entry.path();
            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    dirs.push(path);
                }
            } else if let Ok(rel) = path.strip_prefix(root) {
                paths.push(rel.to_string_lossy().to_string());
            }
        }
    }
    paths.sort();
    paths
}

/// Scan public API symbols only from files under directories matching the filter.
pub(super) async fn scan_public_api_filtered(
    root: &std::path::Path,
    project_type: &str,
    filter: &str,
) -> String {
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
        let mut entries = match tokio::fs::read_dir(&dir).await {
            Ok(e) => e,
            Err(_) => continue,
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            if symbols.len() >= max_symbols {
                break;
            }
            let path = entry.path();
            if entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false) {
                let name = path
                    .file_name()
                    .map(|n| n.to_string_lossy())
                    .unwrap_or_default();
                if !name.starts_with('.') && name != "target" && name != "node_modules" {
                    dirs.push(path);
                }
            } else if let Some(ext) = path.extension().and_then(|e| e.to_str())
                && extensions.contains(&ext)
                && let Ok(content) = tokio::fs::read_to_string(&path).await
            {
                let rel_path = path
                    .strip_prefix(root)
                    .map(|p| p.to_string_lossy())
                    .unwrap_or_else(|_| path.to_string_lossy());
                extract_public_symbols(&content, &rel_path, &mut symbols, project_type);
            }
        }
    }

    symbols.join("\n")
}

pub(super) async fn build_dir_tree(
    root: &std::path::Path,
    max_depth: usize,
) -> std::io::Result<String> {
    let mut lines = Vec::new();
    build_dir_tree_recursive(root, "", max_depth, &mut lines).await?;
    Ok(lines.join("\n"))
}

/// Boxed-future recursion so the async walker can descend without the
/// compiler rejecting recursive async fns.
pub(super) fn build_dir_tree_recursive<'a>(
    dir: &'a std::path::Path,
    prefix: &'a str,
    remaining_depth: usize,
    lines: &'a mut Vec<String>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = std::io::Result<()>> + Send + 'a>> {
    Box::pin(async move {
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

        // Collect (name, is_dir, path) triples asynchronously, then sort.
        let mut collected: Vec<(String, bool, std::path::PathBuf)> = Vec::new();
        let mut entries = match tokio::fs::read_dir(dir).await {
            Ok(rd) => rd,
            Err(_) => return Ok(()),
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let path = entry.path();
            let name = entry.file_name().to_string_lossy().to_string();
            let is_dir = entry.file_type().await.map(|t| t.is_dir()).unwrap_or(false);
            collected.push((name, is_dir, path));
        }

        // Sort: directories first, then files
        collected.sort_by_key(|(name, is_dir, _)| (*is_dir, name.clone()));

        // Filter out noise directories
        let entries: Vec<_> = collected
            .into_iter()
            .filter(|(name, is_dir, _)| {
                if *is_dir {
                    !matches!(
                        name.as_str(),
                        ".git" | "target" | "node_modules" | "__pycache__" | ".venv" | ".rigorix"
                    )
                } else {
                    true
                }
            })
            .collect();

        let len = entries.len();
        for (i, (name, is_dir, path)) in entries.into_iter().enumerate() {
            let is_last_entry = i == len - 1;
            let connector = if is_last_entry {
                "└── "
            } else {
                "├── "
            };

            if is_dir {
                lines.push(format!("{}{}{}", new_prefix, connector, name));
                let child_prefix = format!(
                    "{}{}",
                    new_prefix,
                    if is_last_entry { "    " } else { "│   " }
                );
                build_dir_tree_recursive(&path, &child_prefix, remaining_depth - 1, lines).await?;
            } else {
                lines.push(format!("{}{}{}", new_prefix, connector, name));
            }
        }

        Ok(())
    })
}

/// Detect the project type from key files in the root directory.
pub(super) fn detect_project_type(root: &std::path::Path) -> String {
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
pub(super) async fn read_dependencies(root: &std::path::Path, project_type: &str) -> Vec<String> {
    if project_type.contains("Rust") {
        return read_cargo_dependencies(root).await;
    }
    if project_type.contains("npm") || project_type.contains("TypeScript/JavaScript") {
        return read_package_json_dependencies(root).await;
    }
    if project_type.contains("Python") {
        return read_python_dependencies(root).await;
    }
    Vec::new()
}

/// Parse [dependencies] section from Cargo.toml.
pub(super) async fn read_cargo_dependencies(root: &std::path::Path) -> Vec<String> {
    let path = root.join("Cargo.toml");
    let content = match tokio::fs::read_to_string(&path).await {
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
        if in_deps
            && !trimmed.is_empty()
            && !trimmed.starts_with('#')
            && let Some(eq_pos) = trimmed.find('=')
        {
            let name = trimmed[..eq_pos].trim().to_string();
            let value = trimmed[eq_pos + 1..].trim().to_string();
            let value = value.split('#').next().unwrap_or(&value).trim().to_string();
            deps.push(format!("{name} = {value}"));
        }
    }

    deps
}

/// Parse dependencies from package.json.
pub(super) async fn read_package_json_dependencies(root: &std::path::Path) -> Vec<String> {
    let path = root.join("package.json");
    let content = match tokio::fs::read_to_string(&path).await {
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
pub(super) async fn read_python_dependencies(root: &std::path::Path) -> Vec<String> {
    let mut deps = Vec::new();

    let req_path = root.join("requirements.txt");
    if req_path.exists()
        && let Ok(content) = tokio::fs::read_to_string(&req_path).await
    {
        for line in content.lines() {
            let trimmed = line.trim();
            if trimmed.is_empty() || trimmed.starts_with('#') || trimmed.starts_with('-') {
                continue;
            }
            deps.push(trimmed.to_string());
        }
    }

    if deps.is_empty() {
        let pyproject = root.join("pyproject.toml");
        if pyproject.exists()
            && let Ok(content) = tokio::fs::read_to_string(&pyproject).await
        {
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

    deps
}

/// Read content of key entry-point files for the project type.
pub(super) async fn read_key_files(root: &std::path::Path) -> String {
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
        let content = match tokio::fs::read_to_string(&full_path).await {
            Ok(c) => c,
            Err(_) => continue,
        };
        let lines: Vec<&str> = content.lines().take(max_lines).collect();
        let truncated: String = lines.join("\n");
        let note = if content.lines().count() > max_lines {
            format!(
                "\n// ... (truncated from {} lines)",
                content.lines().count()
            )
        } else {
            String::new()
        };
        sections.push(format!("// === {} ===\n{}{}", rel_path, truncated, note));
    }

    // For TypeScript projects, also scan src/ for additional .ts files
    // that contain exports (functions, classes likely needed by tests)
    if root.join("tsconfig.json").exists() || root.join("package.json").exists() {
        let src_dir = root.join("src");
        if src_dir.is_dir()
            && let Ok(mut entries) = tokio::fs::read_dir(&src_dir).await
        {
            while let Ok(Some(entry)) = entries.next_entry().await {
                let path = entry.path();
                if path.extension().is_none_or(|e| e != "ts") {
                    continue;
                }
                let rel = path
                    .strip_prefix(root)
                    .unwrap_or(&path)
                    .to_string_lossy()
                    .to_string();
                // Skip files already read as candidates
                if candidates.iter().any(|c| **c == rel) {
                    continue;
                }
                let content = match tokio::fs::read_to_string(&path).await {
                    Ok(c) => c,
                    Err(_) => continue,
                };
                let lines: Vec<&str> = content.lines().take(max_lines).collect();
                let truncated: String = lines.join("\n");
                let note = if content.lines().count() > max_lines {
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

    sections.join("\n\n")
}
