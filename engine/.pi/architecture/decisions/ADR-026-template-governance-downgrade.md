# ADR-026 — The template tool may not lower the bar it is judged by

**Status:** Accepted — decided 2026-11-24, from #1655 and the review that said it plainly: what landed
  there makes a self-modifying governance write **visible**, not **approved**. A decision implied by a code
  path is not a decision; this is the one written down.
- **Date:** 2026-11-24
- **Owner:** operator (the rule); implementation: `rigorix-mcp`, `template_tools`.
- **Bounded context:** **Governance** — `template_tools` (the sanctioned writer of `.rigorix/templates`),
  `permission` (the rule that already protects `.rigorix/**` from `file_write`), and `orchestrator`
  (which is judged by the template it is given).
- **Supersedes:** nothing. Extends ADR-017 (preconditions — the gate is the operator's, decided at
  dispatch), ADR-023 (the sink boundary — each mitigation states what it promises and what it does not)
  and #1652's rule that one control answers the same question on every route.
- **Derived from:** two measurements in the tree (§1), not from a preference.

---

## 1. Context — the writer is sanctioned, and it is also the thing being judged

A template declares, per step, the tool it calls and whether a human must approve it. A run is then
judged by that declaration. So a write to `.rigorix/templates/<name>.toml` is not a write to data; it is
a write to the thing that decides what happens to the next run.

Two facts make the current state a defect rather than a gap:

1. **`.rigorix/templates` is already protected from the agent's `file_write`.** The rule is a segment
   test — any path segment equal to `.rigorix` (`engine/src/permission/application/enforcer_impl.rs:59`)
   — so `file_write` and the shell route both refuse it, in every mode (#1493, #1652). The agent has no
   path-based writer for a template.
2. **The template tool writes it anyway.** `FilesystemTemplateRepository::create_reporting`
   (`mcp/src/template_tools/infrastructure/filesystem_repository.rs:594`) writes
   `.rigorix/templates/<name>.toml` with no permission decision and no approval. After #1655 it returns
   `replaced_existing`, the per-step `governance_diff` and the previous definition under
   `.versions/<name>/` — and then writes.

So the only way an agent can change what judges it is the sanctioned tool, and the tool has no rule
about what may change. "Visible" is a property of the record; "approved" is a property of the decision,
and only one of the two exists.

**The asymmetry that settles it.** If the agent could write the same bytes with `file_write`, the write
would be refused. A sanctioned writer that performs a write the ordinary writer is forbidden to perform
is not a capability, it is a bypass with a nicer name — the finding's own words. A capability differs
from a bypass by having a rule about *what* it may do, not only about *how* it is invoked.

## 2. Decision

**D1 — a change that reduces what is gated is refused by the store, and the refusal names the step and
the field.** Exactly three changes are refusals when overwriting an existing template:

| change in the diff | refused | why |
|---|---|---|
| `requires_approval` `true -> false` | **yes** | removes the human from the loop; the audit's executed case |
| a step's `tool` changed | **yes** | the approved action is replaced by a different action; `parameters` follow a tool, so a retarget is a new step wearing the old name |
| a step removed | **yes** | the step took its gate with it |

Everything else is applied and **reported**: adding a step, adding approval, editing a description,
changing parameters. A new template (`previous == None`) is never a downgrade — there is nothing to
lower.

**Decided by derivation, not by taste.** These three are the cases where the *diff itself* states the
reduction, in the store's own classification (`GovernanceChange`), so the rule needs no capability model
and no judgement about which tool is "stronger". A rule that needed a ranking of tools would be a table
somebody maintains wrongly; `true -> false` and `tool A -> tool B` are readable from the record.

**D2 — the remedy is the operator's edit on disk, and that is the whole mechanism.** An operator who
wants a template relaxed opens the file. They are not asked to pass a flag, and there is no
`allow_downgrade: true` parameter: a field on the request is a field the agent can set, and a control
whose subject can satisfy it is not a control. This is also why the rule is in the **store** and not in
the tool's handler — the store is the one door every caller goes through, and a rule in the adapter is a
rule that the next caller (the CLI, a future host, a test fixture) does not have.

**D3 — the report stays, and it is the accountability for what is *not* a refusal.** #1655's
`ReplaceReport` is unchanged in purpose: a parameter edit can widen a step's effect (`run_command` with a
different command) and the diff names it, step by step, field by field, with the previous definition
retained under `.versions/<name>/<epoch>-<digest8>.toml`. The refusal covers what can be classified; the
report covers what cannot. Neither is claimed as the other.

**D4 — the exception is not a mode.** The permission mode does not appear in this rule. `WorkspaceWrite`
and `DangerFullAccess` differ in what the *agent* may do to the workspace; neither says an agent may
rewrite the gate that judges it, and a mode that meant that would be a mode that turns governance off,
which no mode claims to do. If such a posture is ever wanted it must be its own decision with its own
name, not a side effect of a broader mode.

## 3. Consequences

- The MCP `rigorix_create_template` call refuses an overwrite that removes approval, retargets a step or
  drops one, with a message naming the step, the field and the from/to values, and saying what to do
  instead (ask the operator; edit the file on disk).
- A refusal writes nothing: no template file, no version copy. A refused write that left a version behind
  would be a record of a change that did not happen.
- Two of #1655's five tests change meaning rather than disappearing: "an overwrite that removes an
  approval is reported" becomes "…is refused, and the file is untouched", and the tool-change case the
  same. The tests that assert the *report* keep asserting it, on changes that are still applied.
- The `does_not` sentence in the template store's own documentation grows: parameters are reported, not
  classified.

## 4. What this does not decide

- **Parameters.** A `run_command` whose command changed is reported, not refused: "this parameter widens
  the effect" is not readable from a string pair without a per-tool capability model, and inventing one
  here would be a decision this ADR does not make. The report names it, and the previous definition is
  retained.
- **Who may create a template at all.** A new template is not a downgrade, so creation is unaffected; if
  an agent creating its own gates is itself unacceptable, that is a different decision (a policy on the
  tool), and it is not taken here.
- **The `delete` and `rename` paths.** `delete` removes the whole file — a strictly larger reduction —
  and is not part of this decision because #1655 did not touch it. It should be decided next, and until
  it is, the honest statement is that this rule makes the *overwrite* path refuse a downgrade while the
  delete path remains a way to remove a template entirely. Naming that here is the point: an unstated
  remainder is how the next reader concludes the gap was closed.
