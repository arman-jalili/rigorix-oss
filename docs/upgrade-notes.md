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


## Approval records are authenticated; one run key signs and binds (from Unreleased)

**What changed.**

- Approval records now carry an HMAC over every field, including `status`,
  `expires_at`, `nonce`, `approver_id` and the bound intent. The repository
  verifies it on load. A record edited after it was written — including a
  consumed record set back to `pending`, or an expiry moved later — is refused
  with a diagnosis naming the step and node.
- The binding key and the envelope-signing key are one run key. If
  `RIGORIX_HMAC_KEY` and the configured `audit_hmac_key` are both set and
  differ, startup fails and names both sources (never the values).

**What an operator must do.**

1. Keep `RIGORIX_HMAC_KEY` and `rigorix.toml`'s `audit_hmac_key` equal, or set
   only one. `rigorix doctor` prints a fingerprint of each and whether they
   agree.
2. Optionally set `RIGORIX_APPROVAL_STORE=/var/lib/rigorix/approvals.json`
   (absolute, or relative to the repo root) to keep the authoritative store
   outside the agent-writable workspace.
3. **Re-approve paused runs after upgrading.** Records written before this
   change have no MAC, so they cannot authenticate and are refused. Deletion was
   already fail-closed; the MAC closes the edit case.

**Why.** The lifecycle of an approval — pending, unexpired, unused — is
authority over whether a dispatch may happen. A plaintext file inside the tree
the agent writes cannot be that authority, and two different keys cannot attest
the same decision.
