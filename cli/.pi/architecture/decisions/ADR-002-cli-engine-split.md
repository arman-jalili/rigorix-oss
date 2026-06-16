# ADR-002: CLI/Engine Architecture Split

**Status:** Accepted
**Date:** 2026-06-16

## Context

The `rigorix-oss` repository has two modules: `engine/` (a Rust library crate) and `cli/` (planned binary). The engine already has 17 bounded contexts with frozen contracts. The CLI needs to surface these capabilities to the terminal.

## Decision

**Binary crate (`cli/`) depends on library crate (`engine/`).** The CLI is a thin wrapper — no business logic lives in the CLI crate. The CLI only handles:
- Command parsing (clap)
- Config loading and merging
- TUI rendering
- Output formatting
- Signal handling (Ctrl+C)

All execution, planning, and domain logic lives in the engine crate.

## Consequences

- CLI crate stays small and focused on user interaction
- Engine crate remains independently testable without terminal dependencies
- Engine frozen contracts can evolve without breaking the CLI (as long as public API is maintained)
- CLI version can be pinned independently of engine version

## Alternatives

| Alternative | Reason Rejected |
|-------------|----------------|
| Monolithic crate | Engine and CLI have different versioning and testing needs |
| Engine as separate repo | Over-engineering for current scope; workspace is simpler |
| CLI as workspace member | Chosen (already the case) — workspace Cargo.toml with engine as member |

*Affects: CLI Boundary*
