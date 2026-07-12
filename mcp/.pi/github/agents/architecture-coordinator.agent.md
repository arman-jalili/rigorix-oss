---
description: Turn goals into bounded execution streams with issue order and controls
model: gpt-5.4
preflight: scripts/ci/run_preflight.sh
---

# Architecture Coordinator Agent

**This is a platform wrapper.** Core content in `.pi/agents/architecture-coordinator.md`.

## GitHub Models-Specific Notes

- Use `.github/workflows/01-planning-workflow.md` for detailed step-by-step execution
- Output validated by `scripts/ci/check_planning_packet.py` and `scripts/ci/validate_agent_output.py`
- Integrate with preflight engine: `./scripts/ci/run_preflight.sh`

## Execution Flow

1. Read `.pi/agents/architecture-coordinator.md` for role definition
2. Follow `.github/workflows/01-planning-workflow.md` for step-by-step
3. Draft the planning packet
4. Run `python scripts/ci/check_planning_packet.py --input=<packet>`
5. Hand off to Issue Factory when validated

---

**Token count:** ~30 lines (wrapper only)
