# Configuration Module Runbook

> **Last updated:** 2026-06-13
> **Module:** Configuration (`engine/src/configuration/`)
> **Components:** Config, Secret

## Startup Sequence

1. **Config loading** happens at process start via `ConfigService::load()`
2. **Priority order** (highest wins):
   - CLI flag overrides
   - Environment variables (`RIGORIX__*`)
   - `rigorix.toml` in CWD
   - `~/.rigorix/config.toml` (fallback)
   - Compiled-in defaults
3. **Validation** against safety hard-caps happens after loading
4. **Secrets** (API keys) are loaded from environment variables via `SecretService`

## Dependencies

| Dependency | Required | Source |
|-----------|----------|--------|
| `tokio::fs` | Yes | Async file I/O for config files |
| `std::env` | Yes | Environment variable reading |
| `toml` crate | Yes | TOML parsing |
| `serde` | Yes | Deserialization |

## Graceful Shutdown

- Configuration is read-only after loading — no shutdown needed
- Cached configuration (if used) is written atomically (`.tmp` → rename pattern)
- No background tasks are spawned by the configuration module

## Common Failure Modes

| Failure | Symptom | Recovery |
|---------|---------|----------|
| Config file not found | `ConfigurationError::NotFound` | Falls through to defaults unless path explicitly required |
| Invalid TOML syntax | `ConfigurationError::ParseError` | Check file syntax with `toml2json` or `cargo run -- check` |
| Value exceeds safety cap | `ValidationError` in output | Adjust config value — check caps in `SafetyCaps` |
| Environment variable missing | `ConfigurationError::EnvVarError` | Set the env var or provide fallback |
| IO error reading file | `ConfigurationError::Io` | Check file permissions and path existence |

## Configuration Reference

See `.pi/architecture/modules/configuration.md` for the full config schema.

Key environment variables:

| Variable | Purpose |
|----------|---------|
| `RIGORIX__LOGGING__LEVEL` | Log level override |
| `RIGORIX__LLM__PROVIDER` | LLM provider override |
| `RIGORIX__LLM__MODEL` | Model override |
| `ANTHROPIC_API_KEY` | Anthropic API key |
| `OPENAI_API_KEY` | OpenAI API key |
