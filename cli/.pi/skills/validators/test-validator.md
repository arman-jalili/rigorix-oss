# Test Validator Skill

Validates test coverage and quality. Post-code only (automated).

## Context Files

- `.pi/context/checklists.md` — test checklist
- `.pi/context/output-formats.md` — report format

## Automated Validation

Run `.pi/scripts/validate-tests.sh` for:
- Unit tests execution
- Integration tests execution
- Coverage measurement
- Test failure analysis

## Manual Validation (LLM)

Focus on:
1. **Edge case coverage** — empty inputs, boundaries, errors
2. **Test quality** — specific assertions, deterministic
3. **Missing tests** for new functionality

## Tools

- Bash: Run tests
- Read: Review test code
- Glob: Find test files

## Output

Use `.pi/context/output-formats.md` → "Validation Report"