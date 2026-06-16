//! CliConfigLoader implementation.
//!
//! @canonical .pi/architecture/modules/cli-boundary.md#config
//! Implements: CLI config loading — CLI flags → env vars → rigorix.toml → engine defaults
//! Issue: #237
//!
//! Merges configuration from multiple sources with the following priority
//! (highest wins):
//! 1. CLI flag overrides
//! 2. Environment variables (RIGORIX_*)
//! 3. rigorix.toml config file
//! 4. Engine defaults

use std::collections::HashMap;
use std::path::PathBuf;

use async_trait::async_trait;
use tracing::info;

use crate::domain::config::{CliConfig, ColorMode, LogFormat, LogLevel, OutputFormat};
use crate::domain::error::CliError;
use crate::infrastructure::config::CliConfigLoader;

/// Default CLI configuration file name.
const CONFIG_FILE_NAME: &str = "rigorix.toml";
/// Alternative config directory.
const CONFIG_DIR_NAME: &str = ".rigorix";
/// Alternative config file inside .rigorix/.
const CONFIG_DIR_FILE_NAME: &str = "config.toml";

/// Loads and merges CLI configuration from multiple sources.
pub struct CliConfigLoaderImpl;

impl CliConfigLoaderImpl {
    pub fn new() -> Self {
        Self
    }

    /// Discover candidate config file paths in priority order.
    fn discover_paths(explicit_path: Option<&str>) -> Vec<PathBuf> {
        let cwd = std::env::current_dir().unwrap_or_default();
        let mut paths = Vec::new();

        if let Some(path) = explicit_path {
            paths.push(PathBuf::from(path));
        }

        // rigorix.toml in current directory
        paths.push(cwd.join(CONFIG_FILE_NAME));

        // .rigorix/config.toml in current directory
        paths.push(cwd.join(CONFIG_DIR_NAME).join(CONFIG_DIR_FILE_NAME));

        paths
    }

    /// Load and parse a rigorix.toml config file.
    fn load_config_file(path: &std::path::Path) -> Result<toml::Value, CliError> {
        let contents = std::fs::read_to_string(path).map_err(|e| CliError::ConfigNotFound {
            detail: format!("Failed to read {}: {}", path.display(), e),
        })?;

        contents
            .parse::<toml::Value>()
            .map_err(|e| CliError::ConfigParseError {
                path: path.display().to_string(),
                detail: e.to_string(),
            })
    }

    /// Apply environment variable overrides (RIGORIX_*) to a CliConfig.
    fn apply_env_overrides(config: &mut CliConfig) {
        // Track API key presence
        if std::env::var("RIGORIX_API_KEY").is_ok() {
            config.api_key_configured = true;
        }

        if let Ok(val) = std::env::var("RIGORIX_FORMAT") {
            config.output_format = match val.as_str() {
                "json" => OutputFormat::Json,
                "quiet" => OutputFormat::Quiet,
                _ => OutputFormat::Pretty,
            };
        }
        if let Ok(val) = std::env::var("RIGORIX_LOG") {
            config.log_level = match val.as_str() {
                "trace" => LogLevel::Trace,
                "debug" => LogLevel::Debug,
                "warn" => LogLevel::Warn,
                "error" => LogLevel::Error,
                _ => LogLevel::Info,
            };
        }
        if let Ok(val) = std::env::var("RIGORIX_COLOR") {
            config.color = match val.as_str() {
                "always" => ColorMode::Always,
                "never" => ColorMode::Never,
                _ => ColorMode::Auto,
            };
        }
        if let Ok(val) = std::env::var("RIGORIX_TUI_ENABLED") {
            config.tui_enabled = val == "true" || val == "1";
        }
        if let Ok(val) = std::env::var("RIGORIX_CONFIG") {
            config.config_path = Some(val);
        }
    }

    /// Apply CLI flag overrides to a CliConfig (highest priority).
    fn apply_cli_overrides(config: &mut CliConfig, overrides: CliConfig) {
        if overrides.output_format != OutputFormat::default() {
            config.output_format = overrides.output_format;
        }
        if overrides.log_level != LogLevel::Info {
            config.log_level = overrides.log_level;
        }
        if overrides.log_format != LogFormat::Pretty {
            config.log_format = overrides.log_format;
        }
        if overrides.color != ColorMode::Auto {
            config.color = overrides.color;
        }
        if overrides.config_path.is_some() {
            config.config_path = overrides.config_path;
        }
        config.tui_enabled = overrides.tui_enabled;
        config.force_tui = overrides.force_tui;
    }
}

impl Default for CliConfigLoaderImpl {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl CliConfigLoader for CliConfigLoaderImpl {
    async fn load(&self, cli_overrides: CliConfig) -> Result<CliConfig, CliError> {
        let mut config = CliConfig::default();

        // Step 1: Try loading from config file
        let paths = Self::discover_paths(cli_overrides.config_path.as_deref());
        let mut file_loaded = false;

        for path in &paths {
            if path.exists() {
                match Self::load_config_file(path) {
                    Ok(toml_value) => {
                        info!("Loaded config from {}", path.display());
                        apply_toml_to_config(&mut config, &toml_value);
                        file_loaded = true;
                        break;
                    }
                    Err(e) => {
                        // If an explicit path was given and it fails, propagate the error
                        if cli_overrides.config_path.is_some() {
                            return Err(e);
                        }
                        // Otherwise, silently try the next path
                        continue;
                    }
                }
            }
        }

        if let Some(ref config_path) = cli_overrides.config_path
            && !file_loaded
        {
            return Err(CliError::ConfigNotFound {
                detail: format!("Specified config file not found: {}", config_path),
            });
        }

        // Step 2: Apply environment variable overrides
        Self::apply_env_overrides(&mut config);

        // Step 3: Apply CLI flag overrides (highest priority)
        Self::apply_cli_overrides(&mut config, cli_overrides);

        Ok(config)
    }

    async fn load_from_path(
        &self,
        path: &str,
        cli_overrides: CliConfig,
    ) -> Result<CliConfig, CliError> {
        let mut config = CliConfig::default();

        let config_path = std::path::Path::new(path);
        if !config_path.exists() {
            return Err(CliError::ConfigNotFound {
                detail: format!("Config file not found: {}", path),
            });
        }

        let toml_value = Self::load_config_file(config_path)?;
        apply_toml_to_config(&mut config, &toml_value);

        Self::apply_env_overrides(&mut config);
        Self::apply_cli_overrides(&mut config, cli_overrides);

        Ok(config)
    }

    async fn has_default_config(&self) -> bool {
        let paths = Self::discover_paths(None);
        paths.iter().any(|p| p.exists())
    }

    async fn searched_paths(&self) -> Vec<String> {
        Self::discover_paths(None)
            .iter()
            .map(|p| p.display().to_string())
            .collect()
    }
}

/// Apply TOML config values to a CliConfig.
fn apply_toml_to_config(config: &mut CliConfig, toml: &toml::Value) {
    // Parse [cli] section
    if let Some(cli_section) = toml.get("cli").and_then(|v| v.as_table()) {
        if let Some(val) = cli_section.get("output_format").and_then(|v| v.as_str()) {
            config.output_format = match val {
                "json" => OutputFormat::Json,
                "quiet" => OutputFormat::Quiet,
                _ => OutputFormat::Pretty,
            };
        }
        if let Some(val) = cli_section.get("tui_enabled").and_then(|v| v.as_bool()) {
            config.tui_enabled = val;
        }
        if let Some(val) = cli_section.get("force_tui").and_then(|v| v.as_bool()) {
            config.force_tui = val;
        }
        if let Some(val) = cli_section.get("color").and_then(|v| v.as_str()) {
            config.color = match val {
                "always" => ColorMode::Always,
                "never" => ColorMode::Never,
                _ => ColorMode::Auto,
            };
        }
        if let Some(val) = cli_section.get("log_level").and_then(|v| v.as_str()) {
            config.log_level = match val {
                "trace" => LogLevel::Trace,
                "debug" => LogLevel::Debug,
                "warn" => LogLevel::Warn,
                "error" => LogLevel::Error,
                _ => LogLevel::Info,
            };
        }
        if let Some(val) = cli_section.get("log_format").and_then(|v| v.as_str()) {
            config.log_format = match val {
                "json" => LogFormat::Json,
                _ => LogFormat::Pretty,
            };
        }
    }

    // Parse [cli.api_key] for convenience
    if let Some(api_key) = toml
        .get("cli")
        .and_then(|v| v.get("api_key"))
        .and_then(|v| v.as_str())
        .filter(|k| !k.is_empty())
    {
        // SAFETY: Setting env var for LLM API key during CLI startup.
        // This is safe because no other thread reads RIGORIX_API_KEY concurrently.
        unsafe { std::env::set_var("RIGORIX_API_KEY", api_key) };
        config.api_key_configured = true;
    }

    // Parse [cli.logging] section (backward compat)
    if let Some(logging) = toml
        .get("cli")
        .and_then(|v| v.get("logging"))
        .and_then(|v| v.as_table())
    {
        if let Some(val) = logging.get("level").and_then(|v| v.as_str()) {
            config.log_level = match val {
                "trace" => LogLevel::Trace,
                "debug" => LogLevel::Debug,
                "warn" => LogLevel::Warn,
                "error" => LogLevel::Error,
                _ => LogLevel::Info,
            };
        }
        if let Some(val) = logging.get("format").and_then(|v| v.as_str()) {
            config.log_format = match val {
                "json" => LogFormat::Json,
                _ => LogFormat::Pretty,
            };
        }
    }
}

// ---------------------------------------------------------------------------
// Public validation helpers
// ---------------------------------------------------------------------------

/// Commands that require an LLM API key.
pub fn command_requires_api_key(command: &str) -> bool {
    matches!(command, "run" | "plan" | "generate")
}

/// Validate that the API key is configured for commands that need it.
///
/// Returns `None` if validation passes, or `Some(CliError::MissingConfig)`
/// if the command requires an API key but none was found.
pub fn validate_api_key_for_command(config: &CliConfig, command: &str) -> Option<CliError> {
    if command_requires_api_key(command) && !config.api_key_configured {
        return Some(CliError::MissingConfig {
            field: "api_key".into(),
            hint: "Set RIGORIX_API_KEY environment variable, add [cli.api_key] to rigorix.toml, or run `rigorix init` to configure interactively.".into(),
        });
    }
    None
}

/// Build a `HashMap<String, String>` of CLI overrides for the engine's `ConfigService`.
///
/// Bridges the CLI-side `CliConfig` values to the engine's config schema
/// using dot-notation keys (e.g., `logging.level`).
pub fn build_engine_cli_overrides(config: &CliConfig) -> HashMap<String, String> {
    let mut overrides = HashMap::new();

    // Forward log settings to engine
    overrides.insert(
        "logging.level".to_string(),
        config.log_level.as_tracing_filter().to_string(),
    );
    overrides.insert(
        "logging.format".to_string(),
        if config.log_format.is_json() {
            "json"
        } else {
            "text"
        }
        .to_string(),
    );

    overrides
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::infrastructure::config::CliConfigLoader;

    #[tokio::test]
    async fn test_load_returns_defaults_when_no_config() {
        // When no config path is specified and no rigorix.toml exists,
        // load() returns default configuration.
        let config = CliConfig::default();

        let loader = CliConfigLoaderImpl::new();
        let result = loader.load(config).await;
        assert!(
            result.is_ok(),
            "Should succeed with defaults: {:?}",
            result.err()
        );
        let loaded = result.unwrap();
        assert_eq!(loaded.output_format, OutputFormat::Pretty);
        assert!(loaded.tui_enabled);
        assert_eq!(loaded.log_level, LogLevel::Info);
    }

    #[tokio::test]
    async fn test_load_with_explicit_nonexistent_path_returns_error() {
        // When an explicit config path is given but doesn't exist,
        // load() returns ConfigNotFound.
        let config = CliConfig {
            config_path: Some("/nonexistent/rigorix.toml".into()),
            ..CliConfig::default()
        };

        let loader = CliConfigLoaderImpl::new();
        let result = loader.load(config).await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CliError::ConfigNotFound { .. } => {} // expected
            e => panic!("Expected ConfigNotFound, got: {:?}", e),
        }
    }

    #[tokio::test]
    async fn test_load_from_nonexistent_path_returns_error() {
        let loader = CliConfigLoaderImpl::new();
        let result = loader
            .load_from_path("/nonexistent/rigorix.toml", CliConfig::default())
            .await;
        assert!(result.is_err());
        match result.unwrap_err() {
            CliError::ConfigNotFound { .. } => {} // expected
            e => panic!("Expected ConfigNotFound, got: {:?}", e),
        }
    }

    #[test]
    fn test_env_overrides() {
        // SAFETY: Test environment — no concurrent access to env vars.
        unsafe {
            std::env::set_var("RIGORIX_FORMAT", "json");
            std::env::set_var("RIGORIX_LOG", "debug");
        }

        let mut config = CliConfig::default();
        CliConfigLoaderImpl::apply_env_overrides(&mut config);

        assert_eq!(config.output_format, OutputFormat::Json);
        assert_eq!(config.log_level, LogLevel::Debug);

        // SAFETY: Test environment cleanup — no concurrent access.
        unsafe {
            std::env::remove_var("RIGORIX_FORMAT");
            std::env::remove_var("RIGORIX_LOG");
        }
    }

    #[test]
    fn test_cli_overrides_win() {
        let mut config = CliConfig {
            output_format: OutputFormat::Pretty,
            log_level: LogLevel::Info,
            ..CliConfig::default()
        };

        let overrides = CliConfig {
            output_format: OutputFormat::Json,
            log_level: LogLevel::Debug,
            ..CliConfig::default()
        };

        CliConfigLoaderImpl::apply_cli_overrides(&mut config, overrides);

        assert_eq!(config.output_format, OutputFormat::Json);
        assert_eq!(config.log_level, LogLevel::Debug);
    }

    #[test]
    fn test_discover_paths_with_explicit() {
        let paths = CliConfigLoaderImpl::discover_paths(Some("/custom/path.toml"));
        assert_eq!(paths[0].to_str().unwrap(), "/custom/path.toml");
        assert!(paths[1].to_str().unwrap().ends_with("rigorix.toml"));
    }

    #[test]
    fn test_apply_toml_to_config_sets_values() {
        let toml_str = r#"
[cli]
output_format = "json"
tui_enabled = false
log_level = "debug"
log_format = "json"
"#;
        let toml_value: toml::Value = toml_str.parse().unwrap();
        let mut config = CliConfig::default();
        apply_toml_to_config(&mut config, &toml_value);

        assert_eq!(config.output_format, OutputFormat::Json);
        assert!(!config.tui_enabled);
        assert_eq!(config.log_level, LogLevel::Debug);
        assert_eq!(config.log_format, LogFormat::Json);
    }
}
