# Runbook: Configuration Module

> **Module:** `cli/src/configuration/`
> **Version:** 0.1.0
> **Last Updated:** 2026-06-16

## Overview

The Configuration module loads and merges CLI configuration from multiple sources:
1. Engine defaults (built-in)
2. `rigorix.toml` config file
3. Environment variables (`RIGORIX_*`)
4. CLI flags (highest precedence)

## Architecture

```
User → CLI flags → CliConfigLoader (application/)
                        ↓
              Merge with env vars (RIGORIX_*)
                        ↓
              Merge with rigorix.toml
                        ↓
              Merge with engine defaults
                        ↓
                  CliConfig
                        ↓
              → dispatch_command()
              → init_engine_config() (bridges to engine ConfigService)
```

## Startup Sequence

1. `main()` parses CLI args via clap
2. `parse_global_options()` builds `CliConfig` overrides from flags
3. `load_config()` merges: CLI flags → env vars → rigorix.toml → defaults
4. `validate_api_key_for_command()` checks if command needs API key
5. `init_engine_config()` bridges CLI config to engine's `ConfigService`

## Graceful Shutdown

Configuration is loaded once at startup and is immutable. No graceful shutdown
needed — config is read-only after initialization.

## Common Failure Modes

| Failure | Symptom | Recovery |
|---------|---------|----------|
| No config file | `CliError::ConfigNotFound` | Run `rigorix init` to create `.rigorix/` |
| Invalid TOML | `CliError::ConfigParseError` | Check `rigorix.toml` syntax |
| Missing API key | `CliError::MissingConfig` | Set `RIGORIX_API_KEY` or add to rigorix.toml |
| Bad env var value | Invalid config value | Check env var format (`RIGORIX_*`) |

## Configuration Reference

| Setting | Source | Default | Description |
|---------|--------|---------|-------------|
| `output_format` | CLI flag / env / config | `pretty` | Output format: pretty, json, quiet |
| `tui_enabled` | CLI flag / env | `true` | Enable TUI renderer |
| `color` | CLI flag / env | `auto` | Color mode: auto, always, never |
| `log_level` | CLI flag / env | `info` | Log level: trace, debug, info, warn, error |
| `log_format` | CLI flag | `pretty` | Log format: pretty, json |
| `api_key` | Env / config | `None` | API key for LLM access |

## Troubleshooting

### Config not loading
```bash
# Check if config file exists
ls -la rigorix.toml .rigorix/config.toml

# Try with explicit path
rigorix --config /path/to/config.toml run "test"
```

### API key not found
```bash
# Check environment
echo $RIGORIX_API_KEY

# Add to config
echo 'api_key = "sk-..."' >> rigorix.toml
```
