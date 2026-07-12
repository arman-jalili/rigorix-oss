---
description: Execute one issue at a time with strict acceptance-criteria closure
model: gpt-5.4
preflight: scripts/ci/run_preflight.sh
---

# uootstrap implementer Agent

**This is a platform wrapper.** Core content in `.pi/agents/bootstrap-implementer.md`.

## GitHub Models-Specific Notes

- Output validated by `scripts/ci/validate_agent_output.py`
- Integrate with preflight engine: `./scripts/ci/run_preflight.sh`

## Execution Flow

1. Read `.pi/agents/bootstrap-implementer.md` for role definition
2. Follow the relevant workflow for your execution phase
3. Run deterministic validation before handing off
