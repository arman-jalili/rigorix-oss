# ADR-006: Plugin System Deferral to v2

**Status:** Accepted
**Date:** 2026-06-16

## Context

The engine has a `Tool` trait that defines executable operations. Users may want to add custom tools. Should the CLI support a plugin system for third-party tools?

## Decision

**Defer a full plugin system to v2.** For v1, use configurable tool aliases in `rigorix.toml`.

### v1 Solution: Configurable Tool Aliases

```toml
[[tool_aliases]]
name = "my-lint"
command = "./scripts/lint.sh"
risk_level = "low"

[[tool_aliases]]
name = "deploy-staging"
command = "bash deploy.sh --env staging"
risk_level = "high"
timeout_secs = 300
```

These aliases register into the existing `ToolRegistry` and are executed via the `RunCommand` action — no dynamic loading needed.

### v2 Consideration: WASM-Based Plugins

For v2, consider WASM-based plugins with a well-defined ABI:
- Users compile tools to `.wasm` files
- Runtime loads them in a sandboxed WASM environment
- Well-defined interface (import/export functions)

## Alternatives

| Alternative | Reason Rejected |
|-------------|----------------|
| Dynamic loading (dlopen) | Rust has no stable ABI; fragile and unsafe |
| WASM plugins now | Adds `wasmtime` dependency, increases binary size, over-engineering for v1 |
| No extensibility | Too restrictive — command aliases cover 90% of use cases |

*Affects: Tool System, CLI Boundary, Configuration*
