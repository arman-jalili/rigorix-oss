# Operations Validator Skill

Validates production readiness. Checks tracing, cancellation, atomic writes.

## Context Files

- `.pi/context/checklists.md` — operations checklist
- `.pi/context/patterns.md` — implementation patterns
- `.pi/context/output-formats.md` — report format

## Automated Validation

Run `.pi/scripts/validate-operations.sh` for:
- Tracing on public functions
- CancellationToken/AbortController usage
- Atomic write pattern
- Blocking calls in async context

## Manual Validation (LLM)

Focus on:
1. **Business logic** reliability
2. **Complex cancellation** scenarios
3. **Error recovery** strategies

## Tools

- Grep: Find patterns
- Read: Review implementation
- Bash: Run validation scripts

## Output

Use `.pi/context/output-formats.md` → "Validation Report"