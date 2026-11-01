# ADR-020: The approval store is a chained log, not a sealed snapshot

**Status:** Proposed (under review). Corrections from review are incorporated; **not accepted, not scheduled.**
Adoption is gated by the triggers in § Adoption triggers.
**Date:** 2026-10-31
**Supersedes:** the storage shape introduced with the record MAC (#1237) — a design, not an earlier ADR.
**Superseded by:** nothing.
**Related:** ADR-016 (the chained-log pattern and its boundary), ADR-011 (approval binding),
ADR-019 (effect commitment — the claim/lease vocabulary this reuses), ADR-006 (atomic write-rename),
ADR-017 (consequence gating)

## Context

The approval store holds the decisions that authorize dispatch. It is a single JSON map
(`.rigorix/approvals.json`) rewritten in place, plus a second sealed map for consumption markers
(`$HOME/.rigorix/approval-consumption-<fingerprint>.json`). One week of work on that shape produced
eleven defects and six corrective commits, every one about how state is stored rather than about what
an approval means:

`#1240` `#1241` `#1243` `#1244` `#1247` `#1249` `#1251` `#1253` `#1254` `#1255` `#1256`

The trend in the commit sequence is visible: MAC material, a whole-set seal, a refusal of unsealed
ledgers, a migration compared against the store, `N of 0` phrasing, a write lock, live reads. **Each
fix added machinery to make a snapshot behave like a log.**

The same repository already contains the alternative, applied to the audit trail: a per-producer
chain (`audit/application/chain.rs:28` `next_link_from`, `:63` `verify_chain`) with no defects about
its own shape.

### What the code does today

| fact | citation |
|---|---|
| every write re-reads the map, mutates it, and re-publishes it under an exclusive `flock` | `approval/infrastructure/repository/approval_repository_impl.rs:743` (`update_store`), `:97` (`with_exclusive_lock`) |
| every read re-reads the store and the ledger under shared locks (#1261) | same file `:618` (`read_snapshot`), `:789` (`load`) |
| a second terminal write for one `(node, nonce)` is refused on write | same file `:678` (`record_consumption`), `ApprovalError::ConsumptionReplay` |
| a restored `Pending` record under a live marker is refused on read | same file `:646` (`reject_rolled_back`), `RecordRollback` |
| **the approval is verified before dispatch and consumed *after* the outcome** | `execution_engine/application/service_impl/dispatch.rs:386` (`verify_intent`), `:568` (`consume_on_terminal`) |
| **a refused consumption is a warning, not a failure** | same file `:568-573` — `ConsumptionReplay` reaches the `Err(e) => tracing::warn!(…"consume failed (non-fatal)")` arm |
| the approver identifier is a free string validated only for emptiness | `approval/application/dto/mod.rs:110` |
| the identity contract permits an email as the subject (`"user@org" or sub`) | `identity/domain/claim.rs` (`IdentityClaim::subject`) |

The two bolded rows are the reason this ADR remains a *proposal* rather than a plan: they are the
actual single-use gap, and they are not a storage problem. See § What the log does not fix.

## Decision

### D1 — The store is an append-only chained log

Approval state is **derived** from a log of events. No record field is overwritten and no file is
rewritten. Two logs, one format:

| log | path | why it is separate |
|---|---|---|
| decisions | `<repo_root>/.rigorix/approval-log` (+ `.head`) | belongs to the repository the run is about |
| consumption | `$HOME/.rigorix/approval-consumption-log-<key-fingerprint>` (+ `.head`) | must resolve **outside** agent-writable space; a spent approval must not be restorable by editing the workspace (#1242, #1244) |

### D2 — File format and frames (normative)

**One file per log**, appended with `O_APPEND`, not one file per event. The review established the
reason: a fold over N files costs N `open`+`read`+`parse`+verify, and `verify_intent` runs in the
dispatch path an agent loops over. One file costs one open and one read.

Frame: `[len: u32 BE][crc32: u32 BE][frame bytes]`, where the frame is

```
{ "v": 1, "seq": u64, "prev": Option<String>, "kind": Kind, "at": RFC3339,
  "kid": Option<String>, "payload": <kind-specific>, "mac": String }
```

- `prev` = `SHA-256` of the predecessor frame's canonical bytes (the bytes the MAC signs), `null` iff
  `seq == 0`.
- `mac` = `HMAC-SHA256(k_evidence, canonical_bytes(frame minus mac))`, hex.
- `len` and `crc32` make a torn tail **detectable rather than a parse error**: the fold refuses a tail
  frame whose length or CRC does not resolve, naming the byte offset, and never guesses.
- An unknown `v` is refused by name. `version` is the frame version, not the crate version.

### D3 — Append protocol (normative)

Appending `seq = n`: read the head (D5), append one framed event at the current end, `sync_all`, then
publish the new head atomically. `O_APPEND` makes the offset update and the write one operation, so
two appenders never interleave inside a frame; a frame that fails its CRC proves the append did not
complete and the appender truncates to the last good boundary before retrying.

### D4 — Fold (normative)

| kind | payload | effect on the folded record |
|---|---|---|
| `decided` | step, node, intent hash, provenance, payload, approver reference, authority, decided_at, expires_at, nonce, decision_context, approver_scope | creates the record in `Pending` |
| `claimed` | node, nonce, attempt, at, lease_until | `Pending → Claimed`; see D6 |
| `released` | node, nonce, attempt, reason, at | `Claimed → Pending`; a retryable failure returns the approval |
| `consumed` | node, nonce, attempt, at | `Claimed → Consumed` |
| `expired` | node, at | `Pending \| Claimed (lease lapsed) → Expired` |
| `superseded` | node, superseded_by, at | `Pending → Superseded` |
| `rolled_back` | node, nonce, evidence, at | **annotation only — never re-arms.** See below |

Rules:

1. `decided` for a `(node_id, nonce)` already present is **retained and reported**, never dropped.
2. A transition that the state machine does not permit is **retained and reported** as inert. It is
   not refused at append (a refused append is a lost decision) and not silently ignored (a silent
   ignore is what #1255 was).
3. **`rolled_back` does not revert.** Reverting `Consumed → Pending` would re-arm a spent approval,
   which is the replay path #1241/#1242 exist to close, and it would let one event undo a dispatch
   that ADR-019 may already hold as settled. `rolled_back` records that a consumption is disputed,
   carrying the evidence that the effect did not happen. **Re-use of an approval requires a new
   `decided` event with a fresh nonce** — a re-decision, not a rewind.
4. The fold never invents a state. Everything `doctor`, the CLI and MCP read is derived from these
   kinds, and an unknown kind fails the fold by name.

`ApprovalRecord` keeps its public field set; it becomes a projection. The frozen wire strings
(`precondition_checked`, `precondition_findings[]`, `.rigorix/preconditions.toml`, `RIGORIX_*`,
`precondition_denied`) are unchanged.

### D5 — Chain **and** sealed head

Integrity is two mechanisms, because the review found the boundary is where they differ:

| mechanism | detects | requires the key? |
|---|---|---|
| chain (`prev`) | interior deletion, reorder, duplication, edit | no |
| sealed head (`<log>.head` = `{seq, head, mac}`, replaced atomically) | **tail** truncation, and the log being swapped wholesale | yes, to *forge*; no, to *check* |

Neither alone is enough. A bare chain cannot see its own tail being cut — for the **consumption** log
that means un-spending the most recent approval, which is precisely #1242/#1244. A bare seal (today's
design) makes *every* edit require the key, including interior ones a chain would catch without it.
The head is written after the fold-visible append and is what a reader compares against.

`verify` walks a log and fails with a named error for: a gap (naming the missing `seq`), a `prev`
mismatch (naming both), a duplicate `seq`, a bad or missing MAC, a head that does not match the log's
last frame (naming both), and a torn tail (naming the byte offset). The whole-set seal
(`ledger_mac`), `verify_if_keyed`, and the "unsealed ledger" adoption path are deleted. A legacy
unsealed map is refused by name with the migration command — the rule #1247 established.

### D6 — Consume protocol (normative)

This is the section the review demanded, and it is a dispatch change, not a storage detail.

Today an approval is verified at `dispatch.rs:386` and consumed at `:568` — **after** the outcome.
Nothing is held in between, so two dispatchers both verify, both dispatch, and the second
`consumed` is refused *afterwards* and only warned about. The refusal is a detector, not a gate.

The log protocol closes that, using the total order the log provides:

1. **Claim before dispatch.** Append `claimed` for `(node, nonce, attempt)`. Re-fold. Proceed **iff
   your `seq` is the first `claimed` for `(node, nonce)`**. The loser does not dispatch, and reports
   the approval as already in flight.
2. **Settle on terminal outcome.** Append `consumed` (success, skipped, exhausted failure) — the
   existing single-use-on-terminal rule, unchanged.
3. **Release on a retryable failure.** Append `released`, returning the approval to `Pending` with the
   attempt number preserved, so a legitimate retry re-verifies the same intent.
4. **Lease expiry.** A `claimed` event older than its `lease_until` folds to `Expired`, so a crash
   after the claim and before the outcome cannot leave an approval permanently in flight.
5. **A crash between the append and the dispatch fails closed**: the approval is spent (or leased and
   then expired) and nothing was dispatched. That is the correct direction for a single-use token and
   it is stated here rather than discovered.

This is the same shape ADR-019 already uses for effects (`Dispatched → Settled/Unsettled` with a
lease) and the capacity ledger uses for capacity (`outstanding → consumed/released/expired`). **Before
implementation, D6 must be reconciled with ADR-019's intent record so the deployment does not end up
with two fencing mechanisms for one dispatch**; the claim's identity must be the effect intent key, or
the two must be explicitly ordered.

### D7 — Approver identity: **deferred to ADR-021**

The review rejected this ADR's own answer, correctly. What was proposed — *"the identifier is a
subject reference, reject `@`"* — is a weak control packaged as an erasure decision:

- `IdentityClaim::subject` is documented as `"user@org" or sub`, so the ADR forbade at the approval
  boundary what the identity contract permits at its source. It never said **where** the conversion
  happens.
- A check for `@` and whitespace catches the obvious case and nothing else.
- "The log stores nothing that resolves to a person" is a **contested legal position**, not a
  technical fact: a stable IdP subject in an append-only log is routinely treated as personal data
  while a mapping exists. That determination belongs to a DPA review, not to this ADR.

What this ADR requires of identity, and nothing more: the `decided` payload carries an **approver
handle**, opaque and stable for the deployment, and the log never carries a display name. Whether that
handle is a keyed pseudonym, a tenant-scoped pseudonym, or a bare subject is **ADR-021's decision**,
along with its erasure mechanism (D11). Until ADR-021 exists, no implementation of this ADR ships.

### D8 — Migration

- Read path: log first; if absent, the legacy map (honouring `RIGORIX_APPROVAL_STORE` and
  `RIGORIX_APPROVAL_CONSUMPTION_LEDGER` for **reading only**), then refuse to write until migrated.
- `rigorix approve ledger migrate` backfills each legacy record as one `decided` event and one
  `consumed` event per marker, then reports the counts it wrote.
- An absent or empty legacy file on a host with no log is a **no-op with exit 0** naming the path (#1256).
- The dual-read window ends at 2.0.0; after it the legacy reader is deleted, not left to rot.

### D9 — Failure semantics (normative)

| state | behaviour |
|---|---|
| no log file | empty fold; not an error; `doctor` says the log does not exist yet |
| log present, head absent | refused: a log without a head cannot report its tail (#D5) |
| torn tail frame | refused, naming the byte offset; never a partial fold |
| gap in `seq` | `ChainBroken`, naming the missing `seq` |
| head mismatch | refused, naming the head's `seq` and the log's last `seq` |
| MAC failure | refused, naming `seq` and the keys tried; never "no history yet" |
| legacy map, no log | refused by name, with the migration command (#1247) |
| terminal record with no consumption event | reported as a finding naming node and step (#1255) |
| `claimed` past its lease | folds to `Expired`, reported |

### D10 — One primitive, two implementations, shared conformance vectors

`audit/application/chain.rs` is 295 lines and audit-specific. The approval log gets its own framing and
verification over `ApprovalEvent`, and the two remain independent implementations. **They must not
drift**: a shared vector set (`engine/tests/conformance/chained-log/*.json` — genesis, gap, reorder,
duplicate, prev mismatch, tail truncation, torn tail, head mismatch) is executed by **both**, so a
divergence is a test failure in both places. Extraction into a shared kernel remains triggered by a
third user (the rule of three), not by two.

### D11 — Erasure: ADR-016's recorded compaction, once ADR-021 says what is being erased

If a deployment must erase inside the log, it uses the mechanism ADR-016 already decided for the audit
ledger: key destruction over the payload, a retained `{seq, prev, erased: true}` frame, and a signed
compaction checkpoint so the chain links across the gap. If ADR-021 chooses a keyed pseudonym, the
erasure action is destroying that key, which is a key-lifecycle operation rather than a log rewrite —
and the chain never has to be re-anchored.

### D12 — Checkpoints, and the measured trigger for them

A checkpoint (`folded state + head`) bounds replay to the events after it. **It is not added
speculatively.** It is added by the first change that makes a fold measurably slow, and the number
comes from a bench, not from taste: `engine/benches/approval_log_fold.rs` must exist before the log
ships and must measure `fold_all` and `fold_one` at 1×, 10× and 100× the demo's event count on the
supported filesystems. The ADR fixes the rule — *checkpoint when `fold_one` p95 exceeds 5 ms, or when
the log exceeds 10 000 events, whichever comes first, and record the measured number in the change
that adds it* — and refuses to fix the number before the artifact exists.

### D13 — Supported filesystems

`O_APPEND`, `create_new`, `fsync` on a file and on a directory are POSIX-local guarantees. Supported:
APFS, ext4, xfs, btrfs, NTFS. **Not supported**: NFS, SMB/CIFS, and any filesystem where `flock` is
advisory-and-ignored or where a directory `fsync` is a no-op — the same filesystems where today's
`flock` design already fails silently. `doctor` reports the filesystem type and refuses a log path on
an unsupported one, naming it; it does not warn and continue.

## The precise claim (guarantee / non-guarantee)

**Guaranteed:**

1. Every approval decision and every consumption is recorded once and never rewritten.
2. A deletion, reorder, duplication or insertion **interior** to either log is detectable from that
   log alone, without a key.
3. A **tail** truncation is detectable while the head is present elsewhere (D5).
4. A record's lifetime is reconstructible: decided, claimed, released, consumed/expired/superseded,
   each with a time and an event identity.
5. Two appenders produce two appends; neither erases the other's event.
6. An absent log is distinguishable from an emptied log, a torn log, and a broken one, by name.

**Not guaranteed:**

- **Replay of the most recent approvals** when the log **and** its head are both removed, or on a host
  whose `$HOME` has no consumption log at all (a fresh CI runner is exactly this). This is the sharpest
  boundary in the design and it is stated here rather than implied.
- The truthfulness of a host compromised *before* it appends.
- Cross-log atomicity: a `claimed`/`consumed` pair lives in one log and its `decided` in another, so a
  crash between them leaves a claim in flight — the correct, recoverable state, not a torn write.
- Anything about *who* the approver was beyond the handle in the frame (D7, ADR-021).
- Any guarantee on an unsupported filesystem (D13).

## What the log does not fix

The review's most useful finding is not about storage at all:

- **Consumption is post-hoc.** `consume_on_terminal` runs after the outcome and its error is a
  `warn!` (`dispatch.rs:568-573`). A `ConsumptionReplay` therefore *records* a double dispatch and
  does not prevent it. A log does not change this; D6 does, and D6 changes the dispatch path.
- **A host without a consumption ledger treats every approval as unspent.** `ledger_present: false`
  makes `reject_rolled_back` skip the terminal check (`:646`). A fresh runner replays. A log makes this
  *visible* (D9) rather than fixing it; the fix is a refusal when a ledger path is configured but
  absent, which is a change to the current design too.

Both are filed against the current design, not against this ADR.

## Adoption triggers

This ADR is **not scheduled**. It is adopted by the first of:

1. **Lifecycle history becomes a requirement** — a customer or an auditor asks *when* an approval was
   consumed, on which attempt, and what happened after; the projection cannot answer it.
2. **Two hosts must write one store.** Today's `flock` does not serialize across hosts on the
   filesystems in D13, so this is a real gap — but the honest answer for multi-host is the enterprise
   ingest, which already owns a hash-chained ledger server-side (`audit_ingestion::ledger`), not a
   replicated file.
3. **D6 is required on its own** — if the dispatch gate needs the claim/lease protocol for reasons that
   have nothing to do with storage, D6 lands independently of the log.

If none holds, the current design stands: it is sealed, doubly checked, live-reading, and covered by
seeds — and every defect the eleven issues named is closed.

## Corrections incorporated from review

| objection | disposition |
|---|---|
| single-use consumption is not atomic | **accepted** — D6 added (claim → re-fold → win), plus the finding that the current design has the same gap and worse: the refusal is post-hoc and non-fatal |
| tail deletion un-spends the most recent approval | **accepted** — D5 adds a sealed head; the boundary is now stated in the non-guarantees, including the empty-`$HOME` case |
| D7 conflicts with the identity contract and relies on a weak control | **accepted** — D7 reduced to a handle, moved to ADR-021 with the legal question explicitly undecided |
| `rolled_back` re-arms spent approvals | **accepted** — D4 rule 3: annotation only; re-use needs a new decision with a fresh nonce |
| reads become n opens and n HMAC checks | **accepted** — D2 switched to one file with framed, CRC-checked appends; D12 adds a bench and a measured checkpoint trigger |
| two chain verifiers can drift | **accepted** — D10 mandates shared conformance vectors |
| filesystem support unstated | **accepted** — D13 names the supported set and makes `doctor` refuse the rest |

## Rejected alternatives

- **Keep the map; strengthen the locking.** This is the change already made six times. It makes writes
  O(n) and serialized across a host, and it cannot record a lifecycle transition — the reason ten of
  the eleven defects exist. Retained as the status quo per § Adoption triggers.
- **Per-marker MACs instead of a whole-set seal.** Insufficient: deleting an entry deletes its MAC.
  Replaced by chain + sealed head (D5).
- **Crypto-shredding per identity now.** Premature: the handle decision belongs to ADR-021, and the
  mechanism, when needed, is ADR-016's compaction.
- **Event-source the audit envelope as well.** Rejected: the envelope is the signed, cross-repo
  artifact, and its position in the chain is already immutable.
- **A new bounded context, or a shared chained-log kernel.** The language does not change — an
  approval event is an approval fact. Extraction waits for a third user (D10).
- **Store the events in `event_system`.** It is an in-memory broadcast bus drained at end of run.

## Implementation

| layer | change |
|---|---|
| domain | `ApprovalEvent` + `fold` + `ApprovalLogError` (named variants, no catch-all); state machine gains `Claimed` |
| application | port: `append`, `claim`, `release`, `consume`, `load`, `verify` |
| infrastructure | framed log (append/fold/verify/head), legacy reader for the dual-read window, migration backfill, filesystem check |
| execution engine | `dispatch.rs`: claim before dispatch, settle on terminal, release on retryable failure (D6) |
| cli | `rigorix approve ledger migrate` retargeted; `doctor` reports per-log chain state, head, filesystem, and populations |
| docs | `modules/approval.md`; claim ceiling extended per below |

## Validation

Every invariant above is proven by a seed that must fail when the property is removed:

| seed | expected |
|---|---|
| two appenders write at once | both frames land whole; both events present; CRC proves neither is torn |
| delete an interior frame | `ChainBroken`, naming the missing `seq` |
| edit an interior frame | `prev` mismatch on its successor, naming both |
| truncate the tail, leave the head | refused: head says `seq N`, log ends at `N-1` |
| truncate the tail **and** the head | fold succeeds and the log reports it cannot see beyond its own head — never "verified" |
| two claims for one `(node, nonce)` | the second does not dispatch; the first wins by `seq` |
| claim, then crash, then the lease lapses | folds to `Expired`; nothing dispatched |
| release after a retryable failure | approval returns to `Pending` with the attempt preserved |
| `rolled_back` on a consumed approval | recorded as an annotation; the approval stays spent |
| no log file | empty fold, exit 0, name the path |
| legacy map, no log | refused by name with the migration command |
| unsupported filesystem | refused, naming the filesystem |

## Claim ceiling

New claims this ADR authorizes: an approval decision, its claim and its consumption are recorded once,
and a modification interior to either log is detectable without a key; a record's lifecycle is
reconstructible; a tail truncation is detectable while the head survives.

Claims this ADR does **not** authorize: that the log cannot be deleted; that the most recent approvals
cannot be replayed when log and head are both removed or on a host with no consumption log; that the
approver's identity is recoverable; that cross-log atomicity exists; that the enterprise mirror
inherits any of this; that any guarantee holds on an unsupported filesystem.

## References

- `.pi/architecture/modules/approval.md` — the module contract this ADR would change
- `engine/src/audit/application/chain.rs` — the pattern, and the framing this ADR mirrors
- `.pi/architecture/decisions/ADR-016-audit-integrity-anchor.md` — the chain, its boundary, and the compaction mechanism reused by D11
- `.pi/architecture/decisions/ADR-019-effect-commitment.md` — the claim/lease vocabulary D6 must reconcile with
- `.pi/architecture/decisions/ADR-011-approval-binding.md` — the binding this store records
- `docs/issues/` entries for `#1240`, `#1241`, `#1243`, `#1244`, `#1247`, `#1249`, `#1251`, `#1253`, `#1254`, `#1255`, `#1256`, `#1261`
