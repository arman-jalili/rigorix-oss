//! Multi-source config loader — TOML + env vars + CLI flags + models.json → engine Config.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#config-loading-priority
//! Implements: ConfigLoader component — real config loading + models.json resolution
//! Issue: issue-configloader, issue-llm-models-config
//!
//! # Contract
//!
//! Config loading follows a layered priority (highest wins):
//!
//! 1. CLI flag overrides
//! 2. Environment variables (`RIGORIX__*`)
//! 3. `rigorix.toml` in CWD
//! 4. `~/.rigorix/config.toml` (fallback)
//! 5. `models.json` provider/model definitions (base_url, max_tokens)
//! 6. Compiled-in engine defaults (lowest)
//!
//! The `models.json` file follows the same pattern as `~/.pi/agent/models.json`.
//! See `~/.rigorix/models.json` for available providers and models.

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
// File finders
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

/// Try to find a `models.json` — checks `.rigorix/models.json` in CWD first,
/// then falls back to `~/.rigorix/models.json`.
///
/// Follows the same pattern as pi's `~/.pi/agent/models.json`.
fn find_models_json(cwd: &Path) -> Option<PathBuf> {
    // Check project-level first (walk up from cwd)
    let mut current = Some(cwd.to_path_buf());
    while let Some(dir) = current {
        let candidate = dir.join(".rigorix").join("models.json");
        if candidate.is_file() {
            return Some(candidate);
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    // Fallback to user-level
    dirs::home_dir()
        .map(|h| h.join(".rigorix").join("models.json"))
        .filter(|p| p.is_file())
}

// ---------------------------------------------------------------------------
// File loaders
// ---------------------------------------------------------------------------

/// Try to deserialize a TOML file into a serde_json::Value.
fn load_toml(path: &Path) -> Option<serde_json::Value> {
    let content = fs::read_to_string(path).ok()?;
    let toml_value: toml::Value = toml::from_str(&content).ok()?;
    serde_json::to_value(toml_value).ok()
}

/// Try to deserialize a JSON file into a serde_json::Value.
fn load_json(path: &Path) -> Option<serde_json::Value> {
    let content = fs::read_to_string(path).ok()?;
    serde_json::from_str(&content).ok()
}

/// Read environment variables with `RIGORIX__` prefix into a flat map.
///
/// `RIGORIX__LLM__API_KEY` → `{"llm.api_key": "sk-..."}`
/// `RIGORIX__ORCHESTRATOR__MAX_PARALLEL_TASKS` → `{"orchestrator.max_parallel_tasks": "8"}`
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

// ---------------------------------------------------------------------------
// Models.json resolution
// ---------------------------------------------------------------------------

/// Look up the configured provider+model in a parsed `models.json`.
///
/// Returns `(base_url, api_key_hint, max_tokens)` from the model definition.
/// The `api_key_hint` is only used for local providers that don't need auth
/// (e.g. `"not-needed"` for lmstudio/ollama). Real API keys come from env vars.
///
/// # Resolution order
///
/// 1. Exact model ID match within the provider's model list
/// 2. Provider fallback (returns provider's base_url with default tokens)
/// 3. `None` if the provider is not found in models.json
fn resolve_model_settings(
    models: &serde_json::Value,
    provider: &str,
    model_id: &str,
) -> Option<(String, String, u32)> {
    let prov = models.get("providers")?.get(provider)?;
    let base_url = prov.get("baseUrl")?.as_str()?.to_string();
    let api_key = prov
        .get("apiKey")
        .and_then(|k| k.as_str())
        .unwrap_or("")
        .to_string();
    let models_arr = prov.get("models")?.as_array()?;
    // Exact model match
    for m in models_arr {
        if m.get("id")?.as_str()? == model_id {
            let max_tokens = m.get("maxTokens").and_then(|t| t.as_u64()).unwrap_or(4096) as u32;
            return Some((base_url, api_key, max_tokens));
        }
    }
    // Provider fallback
    Some((base_url, api_key, 4096))
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
/// 5. `models.json` provider/model defaults (base_url, max_tokens)
/// 6. Compiled-in defaults (lowest)
///
/// # Models.json integration
///
/// If `~/.rigorix/models.json` or `./.rigorix/models.json` exists, it is parsed
/// and the configured provider+model's base_url and max_tokens are resolved.
/// These serve as low-priority defaults — explicit settings in rigorix.toml
/// or env vars override them.
///
/// # Returns
///
/// A fully resolved `CliConfig` containing both CLI-specific settings
/// and the merged engine `Config`.
pub fn load_config() -> CliConfig {
    let cwd = env::current_dir().unwrap_or_default();
    let repo_root = find_repo_root(&cwd).unwrap_or_else(|| cwd.to_string_lossy().to_string());
    let cwd_path = Path::new(&repo_root);

    // 1. Layered TOML merge (user config first = lowest priority)
    let mut merged = serde_json::json!({});

    // User config (~/.rigorix/config.toml) — lowest priority TOML
    if let Some(cfg) = find_user_config().and_then(|p| load_toml(&p)) {
        deep_merge(&mut merged, cfg);
    }

    // Project config (rigorix.toml in CWD)
    if let Some(cfg) = find_project_config(cwd_path).and_then(|p| load_toml(&p)) {
        deep_merge(&mut merged, cfg);
    }

    // 2. Models.json resolution — look up provider/model for base_url + max_tokens
    if let Some(models_val) = find_models_json(cwd_path).and_then(|p| load_json(&p))
        && let Some(provider_str) = merged.pointer("/llm/provider").and_then(|v| v.as_str())
        && let Some(model_id) = merged.pointer("/llm/model").and_then(|v| v.as_str())
        && let Some((base_url, _api_key, max_tokens)) =
            resolve_model_settings(&models_val, provider_str, model_id)
    {
        let model_defaults = serde_json::json!({
            "llm": {
                "base_url": base_url,
                "max_tokens": max_tokens,
            }
        });
        deep_merge(&mut merged, model_defaults);
    }

    // 3. Environment variable overrides
    for (key, value) in read_env_overrides() {
        set_nested(&mut merged, &key, serde_json::Value::String(value));
    }

    // 4. Build the engine config with layered defaults.
    // Strategy: start with Config::default(), serialize to JSON, then merge user
    // overrides on top. This ensures all required fields are present even when
    // the user provides a partial config (e.g. only [llm] section).
    let mut engine_config_json = serde_json::to_value(Config::default())
        .unwrap_or_else(|_| serde_json::json!({}));
    deep_merge(&mut engine_config_json, merged);

    let engine_config = Config::deserialize(&engine_config_json).ok();

    CliConfig {
        format: super::cli::Format::Pretty,
        verbose: 0,
        repo_root,
        cli_overrides: HashMap::new(),
        engine_config,
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

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
///
/// Recursively merges objects at each key level. Scalar values in `source`
/// overwrite those in `target`. Arrays are replaced, not merged.
fn deep_merge(target: &mut serde_json::Value, source: serde_json::Value) {
    if let (serde_json::Value::Object(t), serde_json::Value::Object(s)) = (target, source) {
        for (k, v) in s {
            if let Some(existing) = t.get_mut(&k)
                && existing.is_object()
                && v.is_object()
            {
                deep_merge(existing, v);
            } else {
                t.insert(k, v);
            }
        }
    }
}

/// Set a nested key like "orchestrator.max_parallel_tasks" on a JSON value.
///
/// Creates intermediate objects as needed. If the path traverses through
/// an existing non-object value, it is replaced with an empty object.
fn set_nested(root: &mut serde_json::Value, path: &str, value: serde_json::Value) {
    let parts: Vec<&str> = path.split('.').collect();
    if parts.is_empty() {
        return;
    }
    let mut cur = root;
    for (i, part) in parts.iter().enumerate() {
        if i == parts.len() - 1 {
            if let serde_json::Value::Object(m) = cur {
                m.insert(part.to_string(), value.clone());
            }
        } else {
            if !cur.is_object() {
                *cur = serde_json::Value::Object(serde_json::Map::new());
            }
            if let serde_json::Value::Object(m) = cur {
                if !m.contains_key(*part) {
                    m.insert(
                        part.to_string(),
                        serde_json::Value::Object(serde_json::Map::new()),
                    );
                }
                cur = m.get_mut(*part).expect("just inserted");
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
    fn test_resolve_model_settings_exact_match() {
        let models = json!({
            "providers": {
                "anthropic": {
                    "baseUrl": "https://api.anthropic.com/v1",
                    "models": [
                        {"id": "claude-sonnet-4-6", "maxTokens": 8192}
                    ]
                }
            }
        });
        let (url, key, tokens) =
            resolve_model_settings(&models, "anthropic", "claude-sonnet-4-6").unwrap();
        assert_eq!(url, "https://api.anthropic.com/v1");
        assert_eq!(tokens, 8192);
        assert_eq!(key, "");
    }

    #[test]
    fn test_resolve_model_settings_fallback_to_provider() {
        let models = json!({
            "providers": {
                "local": {
                    "baseUrl": "http://localhost:8080",
                    "models": []
                }
            }
        });
        let (url, _, tokens) = resolve_model_settings(&models, "local", "unknown-model").unwrap();
        assert_eq!(url, "http://localhost:8080");
        assert_eq!(tokens, 4096);
    }

    #[test]
    fn test_resolve_model_settings_missing_provider() {
        let models = json!({"providers": {}});
        assert!(resolve_model_settings(&models, "nonexistent", "any").is_none());
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
