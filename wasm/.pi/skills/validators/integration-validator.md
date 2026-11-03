# Integration Validator Skill

Validates component integration and wiring.

## Context Files

- `.pi/context/checklists.md` — integration checklist
- `.pi/context/output-formats.md` — report format

## Wiring Checks

```bash
# 1. Find callers
grep -r "function_name" src/

# 2. Check imports
grep -r "import.*module" src/

# 3. Verify registration (if tool/plugin)
grep -r "register" [registry file]
```

## Focus Areas

1. **Interface contracts** — signatures match callers
2. **Data flow** — types consistent across boundaries
3. **Error propagation** — errors handled at integration points

## Tools

- Grep: Find wiring
- LSP: Navigate definitions
- Read: Review interfaces
- Bash: Run integration tests

## Output

Use `.pi/context/output-formats.md` → "Wiring Check Report"