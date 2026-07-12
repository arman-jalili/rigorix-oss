# Security Validator Skill

Validates security requirements. Checks OWASP Top 10, secrets, injection, path traversal.

## Context Files

- `.pi/context/checklists.md` — security checklist
- `.pi/context/output-formats.md` — report format

## Automated Checks

Run `.pi/scripts/validate-security.sh` for:
- Hardcoded secrets detection
- SQL/command injection patterns
- Path traversal vulnerabilities
- Unsafe deserialization

## Manual Checks (LLM)

Focus on:
1. **Logic errors** scripts can't detect
2. **Business logic vulnerabilities**
3. **Complex auth flows**

## OWASP Top 10 Focus

| ID | Issue | Check |
|----|-------|-------|
| A01 | Broken Access Control | Hardcoded credentials, auth checks |
| A03 | Injection | SQL, command, XSS, path traversal |
| A09 | Logging Failures | Sensitive data in logs |

## Tools

- Grep: Find patterns
- Read: Review code
- Bash: Run security scripts

## Output

Use `.pi/context/output-formats.md` → "Security Audit Report"