---
description: Verify implementations maintain security controls without regressions
model: gpt-5.4
preflight: scripts/ci/run_preflight.sh
---

# security validator Agent

**This is a platform wrapper.** Core content in `.pi/agents/security-validator.md`.

## GitHub Models-Specific Notes

- Output validated by `scripts/ci/validate_agent_output.py`
- Integrate with preflight engine: `./scripts/ci/run_preflight.sh`

## Execution Flow

1. Read `.pi/agents/security-validator.md` for role definition
2. Follow the relevant workflow for your execution phase
3. Run deterministic validation before handing off
