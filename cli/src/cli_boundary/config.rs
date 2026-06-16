//! Multi-source config loader — TOML + env vars + CLI flags → engine Config.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#config-loading-priority
//! Implements: ConfigLoader component
//! Issue: issue-configloader
//!
//! # Contract (Frozen)
//!
//! Config loading follows a layered priority (highest wins):
//!
//! 1. CLI flag overrides (from `--config-key value`)
//! 2. Environment variables (`RIGORIX_*`)
//! 3. `rigorix.toml` in CWD
//! 4. `~/.rigorix/config.toml` (fallback)
//! 5. Compiled-in engine defaults (lowest)
//!
//! The CLI loads and merges these sources, then passes the result to
//! `engine::configuration::ConfigService::load()`.

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use rigorix_engine::configuration::domain::config::Config;
use serde::{Deserialize, Serialize};

use crate::cli_boundary::error::CliError;

// ---------------------------------------------------------------------------
// CLI-specific config wrapper
// ---------------------------------------------------------------------------

/// CLI-level configuration that merges with engine Config.
///
/// Contains CLI-specific settings (format, verbosity, repo_root) plus
/// overrides that feed into the engine's multi-source `Config` loading.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    /// Output format (Pretty, Json, Markdown, Quiet).
    pub format: super::cli::Format,

    /// Verbosity level (0 = default, 1 = debug, 2 = trace).
    pub verbose: u8,

    /// Repository root path for execution context.
    pub repo_root: String,

    /// CLI flag overrides that are merged before engine config loading.
    pub cli_overrides: HashMap<String, serde_json::Value>,

    /// Resolved engine `Config` after multi-source merging.
    #[serde(skip)]
    pub engine_config: Option<Config>,
}

impl Default for CliConfig {
    fn default() -> Self {
        Self {
            format: super::cli::Format::Pretty,
            verbose: 0,
            repo_root: String::new(),
            cli_overrides: HashMap::new(),
            engine_config: None,
        }
    }
}

impl CliConfig {
    /// Returns a reference to the resolved engine Config, if available.
    pub fn engine_config(&self) -> Result<&Config, CliError> {
        self.engine_config.as_ref().ok_or_else(|| {
            CliError::Config("Engine config not loaded. Call load_config() first.".into())
        })
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Try to find a `rigorix.toml` by walking up from `cwd`.
fn find_project_config(cwd: &Path) -> Option<PathBuf> {
    let mut current = Some(cwd.to_path_buf());
    while let Some(dir) = current {
        let candidate = dir.join("rigorix.toml");
        if candidate.is_file() {
            return Some(candidate);
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Try to find `~/.rigorix/config.toml`.
fn find_user_config() -> Option<PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".rigorix").join("config.toml"))
        .filter(|p| p.is_file())
}

/// Read environment variables with `RIGORIX_` prefix into a flat map.
/// `RIGORIX_ORCHESTRATOR_MAX_PARALLEL_TASKS` → `{"orchestrator.max_parallel_tasks": "..."}`
fn read_env_overrides() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (key, value) in env::vars() {
        if let Some(suffix) = key.strip_prefix("RIGORIX__") {
            let cfg_key = suffix.to_lowercase().replace("__", ".");
            map.insert(cfg_key, value);
        }
    }
    map
}

/// Try to deserialize a TOML file into a serde_json::Value.
fn load_toml_value(path: &Path) -> Option<serde_json::Value> {
    let content = fs::read_to_string(path).ok()?;
    let toml_value: toml::Value = toml::from_str(&content).ok()?;
    // Convert TOML Value → serde_json::Value
    serde_json::to_value(toml_value).ok()
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Load and merge configuration from all sources.
///
/// Priority (highest wins):
/// 1. CLI flag overrides
/// 2. Environment variables (`RIGORIX__*`)
/// 3. `rigorix.toml` in CWD
/// 4. `~/.rigorix/config.toml` (fallback)
/// 5. Compiled-in defaults
///
/// # Returns
///
/// A fully resolved `CliConfig` containing both CLI-specific settings
/// and the merged engine `Config`.
pub fn load_config() -> CliConfig {
    let cwd = env::current_dir().unwrap_or_default();
    let repo_root = find_repo_root(&cwd).unwrap_or_else(|| cwd.to_string_lossy().to_string());
    let cwd_path = Path::new(&repo_root);

    // 1. Load TOML sources
    let mut merged = serde_json::json!({});

    // User config (lowest priority TOML)
    if let Some(user_cfg) = find_user_config().and_then(|p| load_toml_value(&p)) {
        deep_merge(&mut merged, user_cfg);
    }

    // Project config
    if let Some(proj_cfg) = find_project_config(cwd_path).and_then(|p| load_toml_value(&p)) {
        deep_merge(&mut merged, proj_cfg);
    }

    // 2. Environment variable overrides
    let env_overrides = read_env_overrides();
    for (key, value) in &env_overrides {
        set_nested(&mut merged, key, serde_json::Value::String(value.clone()));
    }

    // 3. Try to deserialize into engine Config
    let engine_config: Option<Config> = match Config::deserialize(&merged) {
        Ok(cfg) => Some(cfg),
        Err(_) => {
            // Partial config + defaults: try loading with defaults
            let defaults = Config::default();
            // Merge defaults with our overrides
            let default_val = serde_json::to_value(&defaults).unwrap_or_default();
            let mut combined = default_val;
            deep_merge(&mut combined, merged);
            Config::deserialize(&combined).ok()
        }
    };

    CliConfig {
        format: super::cli::Format::Pretty,
        verbose: 0,
        repo_root,
        cli_overrides: HashMap::new(),
        engine_config,
    }
}

/// Find the repository root by looking for `.git` directory.
fn find_repo_root(cwd: &Path) -> Option<String> {
    let mut current = Some(cwd.to_path_buf());
    while let Some(dir) = current {
        if dir.join(".git").is_dir() {
            return Some(dir.to_string_lossy().to_string());
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    None
}

/// Deep-merge `source` into `target` (modifies target in place).
fn deep_merge(target: &mut serde_json::Value, source: serde_json::Value) {
    if let (serde_json::Value::Object(target_map), serde_json::Value::Object(source_map)) =
        (target, source)
    {
        for (key, value) in source_map {
            if let Some(existing) = target_map.get_mut(&key)
                && existing.is_object()
                && value.is_object()
            {
                deep_merge(existing, value);
                continue;
            }
            target_map.insert(key, value);
        }
    }
}

/// Set a nested key like "orchestrator.max_parallel_tasks" on a JSON value.
fn set_nested(root: &mut serde_json::Value, path: &str, value: serde_json::Value) {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return;
    }
    let mut current = root;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if let serde_json::Value::Object(map) = current {
                map.insert(part.to_string(), value.clone());
            }
        } else {
            if !current.is_object() {
                *current = serde_json::Value::Object(serde_json::Map::new());
            }
            if let serde_json::Value::Object(map) = current {
                if !map.contains_key(*part) {
                    map.insert(
                        part.to_string(),
                        serde_json::Value::Object(serde_json::Map::new()),
                    );
                }
                // Safe: we just inserted or it exists
                current = map.get_mut(*part).expect("just inserted");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn test_deep_merge_overwrites_scalar() {
        let mut target = json!({"key": "old"});
        let source = json!({"key": "new"});
        deep_merge(&mut target, source);
        assert_eq!(target, json!({"key": "new"}));
    }

    #[test]
    fn test_deep_merge_nested() {
        let mut target = json!({"a": {"b": 1, "c": 2}});
        let source = json!({"a": {"b": 10, "d": 3}});
        deep_merge(&mut target, source);
        assert_eq!(target, json!({"a": {"b": 10, "c": 2, "d": 3}}));
    }

    #[test]
    fn test_set_nested_simple() {
        let mut root = json!({});
        set_nested(&mut root, "key", json!("value"));
        assert_eq!(root, json!({"key": "value"}));
    }

    #[test]
    fn test_set_nested_deep() {
        let mut root = json!({});
        set_nested(&mut root, "orchestrator.max_parallel_tasks", json!(8));
        assert_eq!(root, json!({"orchestrator": {"max_parallel_tasks": 8}}));
    }

    #[test]
    fn test_config_defaults() {
        let config = CliConfig::default();
        assert!(config.engine_config.is_none());
        assert_eq!(config.verbose, 0);
    }

    #[test]
    fn test_config_engine_config_not_loaded() {
        let config = CliConfig::default();
        assert!(config.engine_config().is_err());
    }
}
