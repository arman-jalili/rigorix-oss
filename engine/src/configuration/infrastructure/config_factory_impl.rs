//! Implementation of `ConfigFactory` using serde/Toml parsing.
//!
//! @canonical .pi/architecture/modules/configuration.md#config
//! Implements: ConfigFactory trait — builds ConfigDto from TOML, env, CLI
//! Issue: #3
//!
//! Parses TOML content, applies environment variable overrides, and
//! applies CLI flag overrides with the correct priority order.

use async_trait::async_trait;

use crate::configuration::application::dto::{ConfigDto, LlmConfigDto, ToolsConfigDto};
use crate::configuration::application::factory::ConfigFactory;
use crate::configuration::domain::{
    AuditConfig, ConfigurationError, EnforcementPreset, LoggingConfig, OrchestratorConfig,
    RiskLevel, Secret,
};
use crate::configuration::domain::{LogDestination, LogFormat, LogLevel};

/// Implementation of `ConfigFactory` using serde for TOML deserialization.
pub struct ConfigFactoryImpl;

impl ConfigFactoryImpl {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ConfigFactoryImpl {
    #[tracing::instrument(skip_all)]
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl ConfigFactory for ConfigFactoryImpl {
    #[tracing::instrument(skip_all)]
    async fn build_from_toml(&self, toml_content: &str) -> Result<ConfigDto, ConfigurationError> {
        // Deserialize into a raw ConfigDto via serde
        // We use a generous schema that accepts partial configs
        let raw: serde_json::Value =
            toml::from_str(toml_content).map_err(|e| ConfigurationError::ParseError {
                detail: e.to_string(),
                line: None,
            })?;

        // Convert to ConfigDto merging with defaults
        let defaults = self.defaults();
        Ok(merge_into_defaults(raw, defaults))
    }

    async fn apply_env_overrides(
        &self,
        base: ConfigDto,
        prefix: &str,
    ) -> Result<ConfigDto, ConfigurationError> {
        let mut config = base;
        let env_prefix = if prefix.is_empty() {
            "RIGORIX__"
        } else {
            prefix
        };

        for (key, value) in std::env::vars() {
            if let Some(stripped) = key.strip_prefix(env_prefix) {
                apply_deep_override(&mut config, &stripped.to_lowercase(), &value);
            }
        }

        Ok(config)
    }

    async fn apply_cli_overrides(
        &self,
        base: ConfigDto,
        overrides: std::collections::HashMap<String, String>,
    ) -> Result<ConfigDto, ConfigurationError> {
        let mut config = base;
        for (key, value) in overrides {
            apply_deep_override(&mut config, &key, &value);
        }
        Ok(config)
    }

    #[tracing::instrument(skip_all)]
    fn defaults(&self) -> ConfigDto {
        ConfigDto {
            orchestrator: OrchestratorConfig::default(),
            logging: LoggingConfig::default(),
            tools: ToolsConfigDto {
                tool_overrides: std::collections::HashMap::new(),
                auto_confirm_low: true,
                require_review_medium: true,
                dry_run_high: true,
            },
            enforcement: EnforcementPreset::Default,
            audit: AuditConfig::default(),
            llm: LlmConfigDto {
                provider: "anthropic".to_string(),
                model: "claude-sonnet-4-6".to_string(),
                base_url: None,
                max_tokens: 4096,
                temperature: 0.7,
                api_key: Secret::new(""),
            },
        }
    }
}

/// Merge a partial serde JSON value into a defaults ConfigDto.
#[tracing::instrument(skip_all)]
fn merge_into_defaults(raw: serde_json::Value, mut defaults: ConfigDto) -> ConfigDto {
    if let Some(obj) = raw.as_object() {
        if let Some(orchestrator) = obj.get("orchestrator").and_then(|v| v.as_object()) {
            if let Some(mpt) = orchestrator
                .get("max_parallel_tasks")
                .and_then(|v| v.as_u64())
            {
                defaults.orchestrator.max_parallel_tasks = mpt as u32;
            }
            if let Some(mr) = orchestrator.get("max_retries").and_then(|v| v.as_u64()) {
                defaults.orchestrator.max_retries = mr as u32;
            }
            if let Some(dt) = orchestrator
                .get("default_timeout_secs")
                .and_then(|v| v.as_u64())
            {
                defaults.orchestrator.default_timeout_secs = dt;
            }
        }

        if let Some(logging) = obj.get("logging").and_then(|v| v.as_object()) {
            if let Some(level) = logging.get("level").and_then(|v| v.as_str()) {
                defaults.logging.level = match level.to_lowercase().as_str() {
                    "trace" => LogLevel::Trace,
                    "debug" => LogLevel::Debug,
                    "info" => LogLevel::Info,
                    "warn" => LogLevel::Warn,
                    "error" => LogLevel::Error,
                    _ => LogLevel::Info,
                };
            }
            if let Some(format) = logging.get("format").and_then(|v| v.as_str()) {
                defaults.logging.format = match format.to_lowercase().as_str() {
                    "json" => LogFormat::Json,
                    _ => LogFormat::Text,
                };
            }
            if let Some(dest) = logging.get("destination").and_then(|v| v.as_str()) {
                defaults.logging.destination = match dest.to_lowercase().as_str() {
                    "stdout" => LogDestination::Stdout,
                    "stderr" => LogDestination::Stderr,
                    path => LogDestination::File(path.to_string()),
                };
            }
        }

        if let Some(enforcement) = obj.get("enforcement").and_then(|v| v.as_object()) {
            if let Some(preset) = enforcement.get("preset").and_then(|v| v.as_str()) {
                defaults.enforcement = match preset.to_lowercase().as_str() {
                    "advanced" => EnforcementPreset::Advanced,
                    "aggressive" => EnforcementPreset::Aggressive,
                    _ => EnforcementPreset::Default,
                };
            }
        }

        if let Some(audit) = obj.get("audit").and_then(|v| v.as_object()) {
            if let Some(enabled) = audit.get("enabled").and_then(|v| v.as_bool()) {
                defaults.audit.enabled = enabled;
            }
            if let Some(url) = audit.get("backend_url").and_then(|v| v.as_str()) {
                defaults.audit.backend_url = Some(url.to_string());
            }
            if let Some(mr) = audit.get("max_retries").and_then(|v| v.as_u64()) {
                defaults.audit.max_retries = mr as u32;
            }
        }

        if let Some(llm_section) = obj.get("llm").and_then(|v| v.as_object()) {
            if let Some(provider) = llm_section.get("provider").and_then(|v| v.as_str()) {
                defaults.llm.provider = provider.to_string();
            }
            if let Some(model) = llm_section.get("model").and_then(|v| v.as_str()) {
                defaults.llm.model = model.to_string();
            }
            if let Some(url) = llm_section.get("base_url").and_then(|v| v.as_str()) {
                defaults.llm.base_url = Some(url.to_string());
            }
            if let Some(mt) = llm_section.get("max_tokens").and_then(|v| v.as_u64()) {
                defaults.llm.max_tokens = mt as u32;
            }
            if let Some(temp) = llm_section.get("temperature").and_then(|v| v.as_f64()) {
                defaults.llm.temperature = temp;
            }
            if let Some(key) = llm_section.get("api_key").and_then(|v| v.as_str()) {
                defaults.llm.api_key = Secret::new(key);
            }
        }

        if let Some(tools) = obj.get("tools").and_then(|v| v.as_object()) {
            if let Some(risk) = tools.get("risk").and_then(|v| v.as_object()) {
                if let Some(auto) = risk.get("auto_confirm_low").and_then(|v| v.as_bool()) {
                    defaults.tools.auto_confirm_low = auto;
                }
                if let Some(review) = risk.get("require_review_medium").and_then(|v| v.as_bool()) {
                    defaults.tools.require_review_medium = review;
                }
                if let Some(dry) = risk.get("dry_run_high").and_then(|v| v.as_bool()) {
                    defaults.tools.dry_run_high = dry;
                }
                if let Some(overrides) = risk.get("tool_overrides").and_then(|v| v.as_object()) {
                    for (k, v) in overrides {
                        if let Some(level) = v.as_str() {
                            let risk_level = match level.to_lowercase().as_str() {
                                "low" => RiskLevel::Low,
                                "medium" => RiskLevel::Medium,
                                "high" => RiskLevel::High,
                                _ => continue,
                            };
                            defaults.tools.tool_overrides.insert(k.clone(), risk_level);
                        }
                    }
                }
            }
        }
    }
    defaults
}

/// Apply a single deep override like `orchestrator.max_parallel_tasks=8`
/// or `orchestrator__max_parallel_tasks=8` to a ConfigDto.
#[tracing::instrument(skip_all)]
fn apply_deep_override(config: &mut ConfigDto, key: &str, value: &str) {
    // Support both `.` and `__` as separators
    let separator = if key.contains('.') { "." } else { "__" };
    let parts: Vec<&str> = key.split(separator).collect();

    match parts.as_slice() {
        ["orchestrator", field] => match *field {
            "max_parallel_tasks" => {
                if let Ok(v) = value.parse::<u32>() {
                    config.orchestrator.max_parallel_tasks = v;
                }
            }
            "max_retries" => {
                if let Ok(v) = value.parse::<u32>() {
                    config.orchestrator.max_retries = v;
                }
            }
            "default_timeout_secs" => {
                if let Ok(v) = value.parse::<u64>() {
                    config.orchestrator.default_timeout_secs = v;
                }
            }
            _ => {}
        },
        ["logging", field] => match *field {
            "level" => {
                config.logging.level = match value.to_lowercase().as_str() {
                    "trace" => LogLevel::Trace,
                    "debug" => LogLevel::Debug,
                    "info" => LogLevel::Info,
                    "warn" => LogLevel::Warn,
                    "error" => LogLevel::Error,
                    _ => LogLevel::Info,
                };
            }
            "format" => {
                config.logging.format = match value.to_lowercase().as_str() {
                    "json" => LogFormat::Json,
                    _ => LogFormat::Text,
                };
            }
            _ => {}
        },
        ["enforcement", "preset"] => {
            config.enforcement = match value.to_lowercase().as_str() {
                "advanced" => EnforcementPreset::Advanced,
                "aggressive" => EnforcementPreset::Aggressive,
                _ => EnforcementPreset::Default,
            };
        }
        ["llm", field] => match *field {
            "provider" => config.llm.provider = value.to_string(),
            "model" => config.llm.model = value.to_string(),
            "max_tokens" => {
                if let Ok(v) = value.parse::<u32>() {
                    config.llm.max_tokens = v;
                }
            }
            "temperature" => {
                if let Ok(v) = value.parse::<f64>() {
                    config.llm.temperature = v;
                }
            }
            _ => {}
        },
        ["audit", field] => match *field {
            "enabled" => {
                config.audit.enabled = value.eq_ignore_ascii_case("true");
            }
            "backend_url" => config.audit.backend_url = Some(value.to_string()),
            "max_retries" => {
                if let Ok(v) = value.parse::<u32>() {
                    config.audit.max_retries = v;
                }
            }
            _ => {}
        },
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn test_defaults() {
        let factory = ConfigFactoryImpl::new();
        let config = factory.defaults();
        assert_eq!(config.orchestrator.max_parallel_tasks, 4);
        assert_eq!(config.llm.model, "claude-sonnet-4-6");
        assert_eq!(config.enforcement, EnforcementPreset::Default);
    }

    #[tokio::test]
    async fn test_build_from_toml_full() {
        let toml = r#"
[orchestrator]
max_parallel_tasks = 8
max_retries = 5

[logging]
level = "debug"
format = "json"

[llm]
provider = "openai"
model = "gpt-4o"
max_tokens = 8192
temperature = 0.5
api_key = "sk-test-123"

[tools.risk]
auto_confirm_low = false
require_review_medium = true
dry_run_high = true
"#;
        let factory = ConfigFactoryImpl::new();
        let config = factory.build_from_toml(toml).await.unwrap();

        assert_eq!(config.orchestrator.max_parallel_tasks, 8);
        assert_eq!(config.orchestrator.max_retries, 5);
        assert_eq!(config.llm.provider, "openai");
        assert_eq!(config.llm.model, "gpt-4o");
        assert!(!config.tools.auto_confirm_low);
    }

    #[tokio::test]
    async fn test_build_from_toml_partial() {
        let toml = r#"
[orchestrator]
max_parallel_tasks = 16
"#;
        let factory = ConfigFactoryImpl::new();
        let config = factory.build_from_toml(toml).await.unwrap();

        // TOML fields should merge with defaults
        assert_eq!(config.orchestrator.max_parallel_tasks, 16);
        assert_eq!(config.orchestrator.max_retries, 3); // default
        assert_eq!(config.llm.model, "claude-sonnet-4-6"); // default
    }

    #[tokio::test]
    async fn test_build_from_toml_invalid() {
        let factory = ConfigFactoryImpl::new();
        let result = factory.build_from_toml("not valid toml {{{").await;
        assert!(result.is_err());
    }

    #[tokio::test]
    async fn test_apply_cli_overrides() {
        let factory = ConfigFactoryImpl::new();
        let mut overrides = std::collections::HashMap::new();
        overrides.insert(
            "orchestrator.max_parallel_tasks".to_string(),
            "12".to_string(),
        );
        overrides.insert("logging.level".to_string(), "debug".to_string());

        let config = factory
            .apply_cli_overrides(factory.defaults(), overrides)
            .await
            .unwrap();
        assert_eq!(config.orchestrator.max_parallel_tasks, 12);
    }
}
