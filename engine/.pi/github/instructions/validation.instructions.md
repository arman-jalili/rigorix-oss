---
description: 'Validation and quality gate requirements'
applyTo: '.pi/scripts/*.sh,tests/**/*,scripts/**/*'
---
<!--
Canonical Reference: .pi/github/instructions/validation.instructions.md
Blueprint Source: Guardian Framework v1.2
DO NOT EDIT DIRECTLY - Source: .pi/scripts/
-->

# Validation Guidelines

## Scope-Based Validators

| Scope | Required Validators |
|-------|---------------------|
| Simple | CI + Canonical |
| Moderate | CI + Architecture + Canonical |
| Complex | CI + Architecture + Security + Canonical |
| Critical | All validators + human approval |

## Validator Scripts

### CI Validator (always required)
```bash
bash .pi/scripts/validate-ci.sh
```
Checks: Build, tests, lint, format, audit

### Architecture Validator (moderate+)
```bash
bash .pi/scripts/validate-architecture.sh
```
Checks: Layer structure, module boundaries, circular deps

### Security Validator (complex+)
```bash
bash .pi/scripts/validate-security.sh
```
Checks: Secrets, injection, path traversal

### Canonical Validator (always required)
```bash
bash .pi/scripts/validate-canonical.sh
```
Checks: Reference integrity, coverage ≥50%, architecture sync

## Validation Workflow

```bash
# Always run these
bash .pi/scripts/validate-ci.sh
bash .pi/scripts/validate-canonical.sh

# Scope determines additional validators
# Check scope: grep "scope:" .pi/INDEX.md
```

## Pre-Commit Checklist

- [ ] CI validator passes
- [ ] Canonical validator passes
- [ ] Coverage ≥50%
- [ ] Architecture CHANGELOG checked
- [ ] Scope-appropriate validators run

---

*Reference: .pi/scripts/validate-*.sh*