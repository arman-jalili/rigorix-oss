---
name: commit
description: Produce clean, logical commits during implementation.
---

# Commit Skill

Produce clean, logical commits with meaningful messages following Conventional Commits.

## Protocol

1. **Stage only related changes** — never `git add .` unless all changes belong to one logical commit
2. **Write conventional commit messages**:
   - `feat:` — new feature
   - `fix:` — bug fix
   - `refactor:` — code change that neither fixes a bug nor adds a feature
   - `test:` — adding or updating tests
   - `docs:` — documentation changes
   - `chore:` — maintenance tasks
3. **Include scope when relevant**: `feat(api): add rate limiting`
4. **Keep first line under 72 characters**
5. **Add body for non-trivial changes** explaining the "why", not just the "what"

## Commands

```bash
# Stage specific files
git add <file1> <file2>

# Review what will be committed
git diff --cached

# Commit with conventional message
git commit -m "feat(scope): description" -m "Why: reasoning"

# Amend last commit if needed
git commit --amend
```

## Rules

- NEVER commit generated files (check .gitignore first)
- NEVER commit secrets or credentials
- ALWAYS review `git diff --cached` before committing
- Split large changes into multiple focused commits
- Reference issue numbers in commit messages when applicable: `fix: resolve #123`
