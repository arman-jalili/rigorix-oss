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


## Approval MAC material v2 and rollback protection (from Unreleased)

- The MAC is now derived from the record's own serialization minus `mac`
  (version 2). Records written by the previous material still verify; no
  re-approval is required for that change.
- Terminal transitions (`consumed`, `expired`, `superseded`) are also recorded in
  a consumption ledger keyed by `(node_id, nonce)`. Restoring an older copy of
  `approvals.json` no longer restores a consumed approval: the load is refused.
- The ledger defaults to `$HOME/.rigorix/approval-consumption-<key-fingerprint>.json`, outside the
  repository. Override with `RIGORIX_APPROVAL_CONSUMPTION_LEDGER` (absolute or
  repo-relative). `rigorix doctor` prints both paths and whether they are inside
  or outside the agent-writable workspace.


## Envelope approval refs state intent-hash provenance (from Unreleased)

- `approvalRecordRef.intent_hash_provenance` is **required** (`derived` | `asserted`).
  Envelopes without it no longer validate.
- The OSS engine emits `derived`; an enterprise L2 deployment that accepts a
  caller-supplied hash emits `asserted`. Only `derived` satisfies RS-011 binding;
  `asserted` is RS-013 attribution.
- Use `rigorix-verifier` **0.9.0** (crates.io, npm; Maven/PyPI source is 0.9.0
  but publication remains blocked under SDK #57). A 0.8.x verifier drops the
  field during canonicalization and fails signature verification.


## The consumption ledger is authenticated as a set (from Unreleased)

- The ledger file is now `{ version: 1, entries, mac }`. The MAC covers the
  version and the whole marker map, so deleting one marker is an integrity
  failure naming the store and ledger paths — not a fresh store.
- A missing ledger is still a valid first run; `rigorix doctor` says
  `no history yet`.
- A pre-#1244 bare-map ledger is **refused** by a keyed deployment. Migrate it
  explicitly with `rigorix approve ledger migrate` (requires `RIGORIX_HMAC_KEY`
  or the configured `audit_hmac_key`). The command reports
  `adopted N of M terminal record(s)`, where M comes from the authenticated
  store. If a terminal record has no marker, migration refuses and names the
  node and step; `--accept-missing` adopts those terminal records into the ledger and names
  each one. With no store present the report names the missing path and says
  nothing was compared; an empty store says it held no terminal records to
  compare. The loader never adopts a bare map, because accepting the unprotected
  form is how a truncation becomes a valid ledger. The report counts populations
  rather than printing a fraction: markers adopted, terminal records covered,
  and markers with no record in the store. The last number distinguishes a
  reset store (or a ledger carried from another store) from a routine match.

Running `rigorix approve ledger migrate` with no ledger, or with an empty one,
is a successful no-op (exit 0) that names the path and says nothing was
migrated. Unreadable, corrupt, unsealed and MAC-mismatched ledgers still fail,
and a ledger failure is reported as such rather than as an unavailable store.

A configured ledger that is absent beside a store with terminal records is a
refusal, not a no-op: the ledger is the only witness that those approvals were
spent, and a fresh host or a restored store cannot prove otherwise. Restore the
ledger at the path named in the error, or re-approve the affected steps. An
absent ledger with no terminal records is allowed.

Consumption markers now record the settling `(dag_id, node_id, attempt)`. Markers
written before this change still verify unchanged; they carry no attribution, so
a replay against one is classified as another settlement (fail-safe) rather than
as a benign retry.
- No configuration change is required. The ledger still defaults outside the
  repository (`$HOME/.rigorix/approval-consumption-<key-fingerprint>.json`); override with
  `RIGORIX_APPROVAL_CONSUMPTION_LEDGER`.
