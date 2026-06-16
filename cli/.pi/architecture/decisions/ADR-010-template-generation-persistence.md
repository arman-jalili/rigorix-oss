# ADR-010: Template Generation Persistence Strategy

**Status:** Accepted
**Date:** 2026-06-16

## Context

When the Planning Pipeline's fallback generates a template on-the-fly, or when the user runs `rigorix generate`, the generated template should be persisted so it can be reused.

## Decision

**Persist all generated templates** to `.rigorix/templates/<id>.toml`.

### Two Trigger Paths, Same Persistence

1. **Explicit (`rigorix generate <intent>`)**:
   - Generate TOML → validate → save to `.rigorix/templates/<id>.toml`
   - `--dry-run` flag: preview without saving
   - `--stdout` flag: print to stdout (pipe to file)

2. **Automatic fallback (`rigorix run` with no matching template)**:
   - Generate TOML → validate → save to `.rigorix/templates/<id>.toml`
   - Immediately register in TemplateEngine for current execution
   - Template is available for future runs

### Conflict Resolution

If `.rigorix/templates/<id>.toml` already exists (same template ID):
- Compare content hash
- If identical: skip (idempotent)
- If different: append `-v2`, `-v3`, etc. suffix

## Rationale

1. **Template catalog grows organically** — every fallback generation adds to the library
2. **No wasted LLM calls** — once generated, the template is available for future `run` commands
3. **User-controlled** — `rigorix generate` gives explicit control; fallback is automatic but visible (emits `TemplatePersisted` event shown in TUI)

## Alternatives

| Alternative | Reason Rejected |
|-------------|----------------|
| Discard fallback-generated templates | Wastes LLM calls; user has to re-generate |
| Only persist on explicit `generate` | Loses the organic catalog growth benefit |
| Persist to separate directory | Unnecessary complexity; all templates in one directory |

*Affects: Template Generation, Planning Pipeline, CLI Boundary*
