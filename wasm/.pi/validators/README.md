# Guardian TOML Validators

Declarative validation rules with inline tests. Inspired by RTK's TOML filter pipeline.

## Quick Start

```bash
# Run all validators
guardian validate

# Run a specific TOML validator
guardian validate --filter test-results

# Verify all inline tests pass
guardian validate --verify

# Run with verbose output
guardian validate --verbose
```

## Pipeline Stages

Each filter applies these stages in order:

1. **strip_ansi** — Remove ANSI escape codes
2. **replace** — Regex substitutions, line-by-line, chainable
3. **match_output** — Short-circuit: if blob matches, return message immediately
4. **strip/keep_lines** — Filter lines by regex
5. **truncate_lines_at** — Truncate each line to N chars
6. **head/tail_lines** — Keep first/last N lines
7. **max_lines** — Absolute line cap
8. **on_empty** — Message if result is empty

## File Location

Validators are loaded from:

1. `.pi/validators/*.toml` — project-local (trust-gated)
2. `~/.config/guardian/filters.toml` — user-global
3. Built-in — from Guardian templates

## Trust Model

Project-local validators require explicit trust:

```bash
# Review and trust
guardian trust .pi/validators/custom.toml

# Revoke trust
guardian untrust .pi/validators/custom.toml

# List trusted files
guardian trust --list
```

Or bypass for CI: `GUARDIAN_TRUST_OVERRIDE=1 guardian validate`

## Filter Schema

```toml
schema_version = 1

[filters.my-validator]
command = "build"                    # Command to match
description = "Filter build output"   # Human-readable description
strip_ansi = true                     # Remove ANSI codes
replace = [                           # Regex substitutions (stage 2)
  { pattern = "^noise$", replacement = "" }
]
match_output = [                      # Short-circuit rules (stage 3)
  { pattern = "Build failed", message = "❌ Build failed" }
]
keep_lines_matching = ["error"]       # Keep only error lines (stage 4)
truncate_lines_at = 120               # Max chars per line (stage 5)
head_lines = 10                       # Keep first N lines (stage 6)
tail_lines = 5                        # Keep last N lines (stage 6)
max_lines = 50                        # Absolute cap (stage 7)
on_empty = "✅ All clean"            # Empty result message (stage 8)

# Inline tests (self-verifying)
[[tests.my-validator]]
name = "strips noise"
input = "noise line\nerror: something\nmore noise"
expected = "error: something"
```

## Adding Custom Validators

1. Create `.pi/validators/custom.toml`
2. Define filters with `[[tests.*]]` blocks
3. Run `guardian validate --verify` to test
4. Run `guardian trust .pi/validators/custom.toml` to enable
