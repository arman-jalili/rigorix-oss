# Upgrade notes

## Approving requires a binding and an approver identity (from Unreleased)

**What changed.** There is no longer an unbound approval mode. `approve_node`
grants a step only when the executor carries the ADR-011 approval binding and
the call carries a non-empty `approver_id`. Otherwise every requested step is
denied, and the response carries `denied_reasons: [{ step, reason }]` naming
what is missing.

| condition | result |
|---|---|
| no binding configured | denied; reason names `RIGORIX_APPROVAL_BINDING`, `RIGORIX_HMAC_KEY`, and `rigorix key` |
| no approver identity | denied; reason names the missing approver identity |
| intent cannot be resolved | that step denied by name, not an internal error |
| binding + key + identity | approved, one `ApprovalRecorded` event bound to the intent hash |

**What an operator must do.**

1. Run `rigorix key` and keep the printed key.
2. Export it as `RIGORIX_HMAC_KEY` in the engine/MCP/CLI/action environment.
3. Set `RIGORIX_APPROVAL_BINDING=1`.
4. Restart the process and confirm `rigorix doctor` reports
   `approval binding: armed` with `mode: intent-bound`.

The key must be the same key the signed envelope uses; approvals and the audit
trail are bound to the same run.

**Why it changed.** A step approved without a pinned intent is a step whose
dispatch may differ from what the human approved. An approval that cannot say
which intent it approved and who approved it is not evidence.
