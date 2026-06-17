//! Multi-source config loader — TOML + env vars + CLI flags + models.json → engine Config.
//! @canonical .pi/architecture/modules/cli-boundary.md#config-loading-priority

use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

use rigorix_engine::configuration::domain::config::Config;
use serde::{Deserialize, Serialize};

use crate::cli_boundary::error::CliError;

/// CLI-level configuration that merges with engine Config.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CliConfig {
    pub format: super::cli::Format,
    pub verbose: u8,
    pub repo_root: String,
    pub cli_overrides: HashMap<String, serde_json::Value>,
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
    pub fn engine_config(&self) -> Result<&Config, CliError> {
        self.engine_config.as_ref().ok_or_else(|| {
            CliError::Config("Engine config not loaded. Call load_config() first.".into())
        })
    }
}

// ── File finders ──────────────────────────────────────────────────────────

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

fn find_user_config() -> Option<PathBuf> {
    dirs::home_dir()
        .map(|h| h.join(".rigorix").join("config.toml"))
        .filter(|p| p.is_file())
}

fn find_models_json(cwd: &Path) -> Option<PathBuf> {
    // Check project-level first, then user-level
    let mut current = Some(cwd.to_path_buf());
    while let Some(dir) = current {
        let candidate = dir.join(".rigorix").join("models.json");
        if candidate.is_file() {
            return Some(candidate);
        }
        current = dir.parent().map(|p| p.to_path_buf());
    }
    dirs::home_dir()
        .map(|h| h.join(".rigorix").join("models.json"))
        .filter(|p| p.is_file())
}

// ── Loaders ───────────────────────────────────────────────────────────────

fn load_toml(path: &Path) -> Option<serde_json::Value> {
    let c = fs::read_to_string(path).ok()?;
    serde_json::to_value(toml::from_str::<toml::Value>(&c).ok()?).ok()
}

fn load_json(path: &Path) -> Option<serde_json::Value> {
    serde_json::from_str(&fs::read_to_string(path).ok()?).ok()
}

fn read_env_overrides() -> HashMap<String, String> {
    let mut map = HashMap::new();
    for (key, value) in env::vars() {
        if let Some(suffix) = key.strip_prefix("RIGORIX__") {
            map.insert(suffix.to_lowercase().replace("__", "."), value);
        }
    }
    map
}

/// Look up model settings from a parsed models.json.
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
    // Exact match
    for m in models_arr {
        if m.get("id")?.as_str()? == model_id {
            let max_tokens = m.get("maxTokens").and_then(|t| t.as_u64()).unwrap_or(4096) as u32;
            return Some((base_url, api_key, max_tokens));
        }
    }
    // Provider match fallback
    Some((base_url, api_key, 4096))
}

// ── Public API ────────────────────────────────────────────────────────────

/// Load and merge configuration from all sources.
///
/// Priority (highest wins):
/// 1. CLI flag overrides
/// 2. Environment variables (`RIGORIX__*`)
/// 3. `rigorix.toml` in CWD
/// 4. `~/.rigorix/config.toml` (fallback)
/// 5. `models.json` provider/model defaults (base_url, max_tokens)
/// 6. Compiled-in defaults (lowest)
pub fn load_config() -> CliConfig {
    let cwd = env::current_dir().unwrap_or_default();
    let repo_root = find_repo_root(&cwd).unwrap_or_else(|| cwd.to_string_lossy().to_string());
    let cwd_path = Path::new(&repo_root);

    // 1. Layered TOML merge (low → high priority)
    let mut merged = serde_json::json!({});
    if let Some(cfg) = find_user_config().and_then(|p| load_toml(&p)) {
        deep_merge(&mut merged, cfg);
    }
    if let Some(cfg) = find_project_config(cwd_path).and_then(|p| load_toml(&p)) {
        deep_merge(&mut merged, cfg);
    }

    // 2. Check for models.json and resolve provider/model defaults
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

    // 3. Environment variable overrides (highest text priority)
    for (key, value) in read_env_overrides() {
        set_nested(&mut merged, &key, serde_json::Value::String(value));
    }

    // 4. Deserialize into engine Config
    let engine_config = match Config::deserialize(&merged) {
        Ok(cfg) => Some(cfg),
        Err(_) => {
            let defaults = serde_json::to_value(Config::default()).unwrap_or_default();
            let mut combined = defaults;
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
        let mut t = json!({"k": "old"});
        deep_merge(&mut t, json!({"k": "new"}));
        assert_eq!(t, json!({"k": "new"}));
    }

    #[test]
    fn test_deep_merge_nested() {
        let mut t = json!({"a": {"b": 1, "c": 2}});
        deep_merge(&mut t, json!({"a": {"b": 10, "d": 3}}));
        assert_eq!(t, json!({"a": {"b": 10, "c": 2, "d": 3}}));
    }

    #[test]
    fn test_set_nested_simple() {
        let mut r = json!({});
        set_nested(&mut r, "key", json!("v"));
        assert_eq!(r, json!({"key": "v"}));
    }

    #[test]
    fn test_set_nested_deep() {
        let mut r = json!({});
        set_nested(&mut r, "a.b.c", json!(42));
        assert_eq!(r, json!({"a": {"b": {"c": 42}}}));
    }

    #[test]
    fn test_resolve_model_settings() {
        let m = json!({"providers": {"anthropic": {"baseUrl": "https://api.anthropic.com/v1", "models": [{"id": "claude-sonnet-4-6", "maxTokens": 8192}]}}});
        let (url, key, tokens) =
            resolve_model_settings(&m, "anthropic", "claude-sonnet-4-6").unwrap();
        assert_eq!(url, "https://api.anthropic.com/v1");
        assert_eq!(tokens, 8192);
        assert_eq!(key, "");
    }

    #[test]
    fn test_resolve_model_settings_fallback() {
        let m = json!({"providers": {"local": {"baseUrl": "http://localhost:8080", "models": []}}});
        let (url, _, tokens) = resolve_model_settings(&m, "local", "unknown-model").unwrap();
        assert_eq!(url, "http://localhost:8080");
        assert_eq!(tokens, 4096);
    }

    #[test]
    fn test_resolve_model_settings_missing_provider() {
        let m = json!({"providers": {}});
        assert!(resolve_model_settings(&m, "nonexistent", "any").is_none());
    }
}
