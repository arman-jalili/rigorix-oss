# CI Validator Skill

Automated CI/merge validator. Runs scripts, minimal LLM reasoning.

## Validation

Run `.pi/scripts/validate-ci.sh` for:
- Build success
- Tests pass
- Lint passes
- Format check
- Security audit

## Enforcement

- **All checks must pass** before merge
- **No partial approvals**
- Report failures with specific errors

## Tools

- Bash: Run all validation scripts
- Read: Check CI configuration

## Output

Script outputs standardized report. No additional LLM processing needed.