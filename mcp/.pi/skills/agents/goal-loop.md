---
name: goal-loop
description: Set and manage standing goals that auto-iterate until validated completion.
model: inherit
tools: [Read, Bash, Edit, Write]
---

# Standing Goal Loop (`/goal`)

Set a standing objective that survives across turns. After every turn, Guardian evaluates whether the goal is satisfied using a **dual judge**: deterministic validators + semantic assessment. If not done, a continuation prompt is fed back and the agent keeps working — until the goal is achieved, the turn budget is exhausted, or you pause/clear it.

## Commands

| Command | What it does |
|---------|-------------|
| `/goal <text>` | Set (or replace) the standing goal |
| `/goal <text> --validators=ci,tests,security` | Set goal with specific validators |
| `/goal <text> --validators=all` | Set goal running all available validators |
| `/goal` or `/goal status` | Show current goal, status, and turns used |
| `/goal pause` | Stop auto-continuation without clearing |
| `/goal resume` | Resume (resets turn counter to zero) |
| `/goal clear` | Drop the goal entirely |
| `/goal validators` | Show current goal's validators |
| `/goal validators --discover` | List all available validators (built-in + custom) |
| `/goal validators ci,tests` | Set validators on the active goal |
| `/subgoal <text>` | Add criteria to the active goal |
| `/subgoal list` | List current subgoals |
| `/subgoal remove <N>` | Remove subgoal by 1-based index |
| `/subgoal clear` | Remove all subgoals |

## When to use it

Use `/goal` for tasks where you want the agent to iterate on its own without you re-prompting every turn:

- "Fix every lint error in `src/` and verify CI passes"
- "Add tests for the auth module and get test coverage to 80%"
- "Investigate why the build fails on CI and write up a report"
- "Refactor all print() calls to proper logging across `src/`"

Tasks where the agent does one turn and stops don't need `/goal`. Tasks where **you'd otherwise have to say "keep going" three times** are where this shines.

## How the Judge Works

After every turn, the goal manager runs:

1. **Validator judge (deterministic):** Runs the goal's configured validators (default: `validate-ci.sh` + `validate-canonical.sh`). If any fails → verdict is `continue`.
2. **Semantic judge:** Evaluates whether the agent's response explicitly confirms completion or produces the final deliverable.
3. **Both must pass** for the goal to be marked `done`.

### Available validators

| Validator | Script | Purpose |
|-----------|--------|---------|
| `ci` | `validate-ci.sh` | Build, lint, format, audit |
| `tests` | `validate-tests.sh` | Unit/integration test suite |
| `security` | `validate-security.sh` | Secrets, injection, path traversal |
| `operations` | `validate-operations.sh` | Tracing, cancellation, atomic writes |
| `architecture` | `validate-architecture.sh` | Layer structure, ADR compliance |
| `canonical` | `validate-canonical.sh` | Reference integrity, coverage |
| `integration` | `validate-integration.sh` | Integration test suite |

### Custom validators

Any `validate-*.sh` script you drop in `.pi/scripts/` is automatically discovered:

```bash
# Create a custom validator
printf '#!/bin/bash\nnpm run coverage -- --threshold=80\n' > .pi/scripts/validate-coverage.sh
chmod +x .pi/scripts/validate-coverage.sh

# Use it
/goal Increase coverage --validators ci,tests,coverage
/goal validators --discover  # shows coverage under 'Custom'
```

Custom validators run the same way as built-in ones — exit code 0 = pass, non-zero = fail.

### Custom validators

Any `validate-*.sh` script you drop in `.pi/scripts/` automatically becomes available:

```bash
printf '#!/bin/bash\nnpm run coverage -- --threshold=80\n' > .pi/scripts/validate-coverage.sh
chmod +x .pi/scripts/validate-coverage.sh
/goal Increase coverage --validators ci,tests,coverage
```

Custom validators are discovered at session start. Exit code 0 = pass, non-zero = fail.

### Fail-Open Semantics

If the judge errors (network blip, unavailable), the verdict defaults to `continue` — a broken judge never wedges progress. The **turn budget** is the real backstop.

## Continuation Prompt Format

When the goal continues, the agent receives:

```
[Continuing toward your standing goal]
Goal: <your goal text>

Additional criteria (all must be satisfied):
  1. <subgoal 1>
  2. <subgoal 2>

Continue working toward this goal. Take the next concrete step.
If you believe the goal is complete, state so explicitly and stop.
If you are blocked and need input from the user, say so clearly and stop.
```

## Turn Budget

Default is 20 continuation turns. When the budget is hit:

```
⏸ Goal paused — 20/20 turns used. Use /goal resume to keep going, or /goal clear to stop.
```

`/goal resume` resets the counter to zero.

## Tools

| Tool | Description |
|------|-------------|
| `guardian_goal_evaluate` | Evaluate the standing goal after a turn. Returns verdict, validator status, and whether to continue. |
