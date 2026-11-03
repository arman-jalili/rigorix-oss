# Architecture Validator Skill

Validates architecture compliance. Use for plan review and post-code wiring checks.

## When to Run

| Phase | Scope | Checks |
|-------|-------|--------|
| Plan review | Moderate+ | Design follows patterns, module organization |
| Post-code | All | Wiring checks only (callers, duplicates, imports) |

## Context Files

- `.pi/context/project.md` — scope classification
- `.pi/context/checklists.md` — architecture checklist
- `.pi/context/output-formats.md` — report format

## Post-Code Wiring Checks

```bash
# 1. Callers exist (not dead code)
grep -r "new_function" [src paths]

# 2. No duplicate types
grep -r "struct TypeName|class TypeName" [src paths]

# 3. Module used
grep -r "import.*module" [src paths]

# 4. Tools registered (if applicable)
grep -r "register" [registry file]
```

## Tools

- Read: Read files
- Grep: Search patterns
- Glob: Find files
- Bash: Run validation scripts

## Output

Use `.pi/context/output-formats.md` → "Validation Report"