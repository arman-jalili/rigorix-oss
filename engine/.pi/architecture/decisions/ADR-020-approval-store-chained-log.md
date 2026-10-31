# ADR-020: The approval store is a chained log, not a sealed snapshot

**Status:** Accepted (designed), implementation tracked by the `approval-log` epic
**Date:** 2026-10-31
**Supersedes:** the storage shape introduced with the record MAC (#1237) — a design, not an earlier ADR.
**Superseded by:** nothing.
**Related:** ADR-016 (the chained-log pattern, and the compaction mechanism this ADR reuses),
ADR-011 (approval binding), ADR-006 (atomic write-rename, narrowed by this ADR),
ADR-019 (effect commitment), ADR-017 (consequence gating)

## Context

The approval store holds the decisions that authorize dispatch. It is a single JSON map
(`.rigorix/approvals.json`) rewritten in place, plus a second sealed map for consumption markers
(`$HOME/.rigorix/approval-consumption-<fingerprint>.json`). One week of work on that shape produced
eleven defects and six corrective commits, every one of them about how state is stored rather than
about what an approval means:

`#1240` `#1241` `#1243` `#1244` `#1247` `#1249` `#1251` `#1253` `#1254` `#1255` `#1256`

The trend is visible in the commit sequence: MAC material, a whole-set seal, a refusal of unsealed
ledgers, a migration compared against the store, `N of 0` phrasing, a write lock. **Each fix added
machinery to make a snapshot behave like a log.** The store now has a lock, a unique temp path, a
merge-on-write, a whole-set seal, a migration command, three population counts and a yardstick
comparison — and it still cannot answer *when* a record was consumed, *by which run*, or *why* it
expired, because `status` is overwritten rather than appended.

The same repository already contains the alternative, applied to the audit trail:

| store | shape | defects about the shape |
|---|---|---|
| `.rigorix/audit/` envelopes | sequence + `prev_hash`, `next_link_from`, `verify_chain`, chain lock, interior-gap / reorder / head-deletion detection (ADR-016) | **0** |
| approvals + consumption markers | whole-file read-modify-write, whole-set MAC | **11** |

That is not a difference of care. It is a difference of representation. This ADR applies the
representation that held to the store that did not.

### What the code does today

| fact | citation |
|---|---|
| every write reads the whole map, mutates it, and re-serializes it under a file lock | `approval/infrastructure/repository/approval_repository_impl.rs:662-697` (`update_store`) |
| a unique temp path per write (the fix for two writers erasing each other) | same file `:675-687` |
| the ledger is authenticated as a **whole set** — `MAC(version ‖ entries)` — so any write re-signs every marker | same file `:52` (`ledger_mac`), `:163` (`constant_time_eq`) |
| lifecycle is mutated in place: `Pending → Consumed \| Expired \| Superseded` | `approval/domain/record.rs:285, :305, :320, :340`; `approval/application/service_impl.rs:128, :236` |
| the approver identifier is a free string validated only for emptiness | `approval/application/dto/mod.rs:110` |
| the identity contract permits an email as the subject (`"user@org" or sub`) | `identity/domain/claim.rs` (`IdentityClaim::subject`) |
| the audit trail is already a per-producer chain, with the same framing a log would need | `audit/application/chain.rs:28` (`next_link_from`), `:63` (`verify_chain`) |

## Decision

### D1 — The store is an append-only chained log

Approval state is **derived** from a log of events. No record field is ever overwritten and no file
is ever rewritten. Two logs, one format:

| log | path | why it is separate |
|---|---|---|
| decisions | `<repo_root>/.rigorix/approval-log/` | belongs to the repository the run is about |
| consumption | `$HOME/.rigorix/approval-consumption-log-<key-fingerprint>/` | must resolve **outside** agent-writable space; a spent approval must not be restorable by editing the workspace (#1242, #1244) |

The split is the trust boundary, not a convenience. Merging them into the repository would undo the
property that the consumption evidence is out of reach of the process it constrains.

### D2 — Frame (normative)

One event per file, `<seq:08>.json`:

```
{ "version": 1, "seq": u64, "prev": Option<String>, "kind": Kind,
  "at": RFC3339, "payload": <kind-specific>, "kid": Option<String>, "mac": String }
```

- `prev` = `SHA-256` of the predecessor's canonical bytes (the same canonical bytes the MAC signs),
  `null` iff `seq == 0`.
- `mac` = `HMAC-SHA256(k_evidence, canonical_bytes(frame without "mac"))`, hex.
- `version` is the frame version, not the crate version. An unknown version is refused by name; it is
  never guessed at.

### D3 — Append protocol (normative)

Appending event `seq = n` creates `n.json` with `OpenOptions::create_new(true)`, writes, `sync_all`s
the file and `sync_all`s the directory. `AlreadyExists` means another writer took `n`; the appender
re-reads the tail and retries. There is **no file lock**, and no operation reads more than the tail
it needs. The lock introduced by `1acceb83` is deleted with the map it protected.

### D4 — Fold (normative)

| kind | payload | effect on the folded record |
|---|---|---|
| `decided` | step, node, intent hash, provenance, payload, approver reference, authority, decided_at, expires_at, nonce, decision_context, approver_scope | creates the record in `Pending` |
| `consumed` | node, nonce, at | `Pending → Consumed` |
| `expired` | node, at | `Pending → Expired` |
| `superseded` | node, superseded_by, at | `Pending → Superseded` |
| `rolled_back` | node, nonce, at | `Consumed` revert attempt; recorded, and the revert is applied only if the chain shows the consumption it names |

Rules:

1. `decided` for a `(node_id, nonce)` already present is **retained and reported**, never dropped —
   two decisions for one identity are a finding, not a no-op.
2. A transition against a record that is not `Pending` is **retained and reported** as inert. It is
   not refused at write (a refused append is a lost decision) and not silently ignored (a silent
   ignore is what #1255 was).
3. The fold never invents a state. Everything `doctor`, the CLI and MCP read is derived from these
   five kinds, and the derivation is total — an unknown `kind` fails the fold by name.

`ApprovalRecord` keeps its public field set; it becomes a projection. The 31 references to
`ApprovalStatus` and the frozen wire strings (`precondition_checked`, `precondition_findings[]`,
`.rigorix/preconditions.toml`, `RIGORIX_*`, `precondition_denied`) are unchanged.

### D5 — Verification replaces the seal

Integrity is the chain, not a MAC over a mutable set. `verify` walks a log and fails with a named
error for: a gap (naming the missing `seq`), a `prev` mismatch (naming both `seq`s), a duplicate
`seq`, a bad or missing MAC, and an unreadable frame (naming the file). The whole-set seal
(`ledger_mac`), `verify_if_keyed`, and the "unsealed ledger" adoption path are deleted. A legacy
unsealed map is not adopted implicitly: it is refused by name with the migration command, which is
the rule #1247 established and this ADR keeps.

### D6 — Keys

One evidence key signs both logs: the key the binding already resolves (`RIGORIX_HMAC_KEY`), with
the envelope signer's key remaining separate and the doctor still reporting whether they agree.
`kid` is in the frame and **absent by default** — it reserves the slot so a future per-key or
per-identity scheme is not a format break. No key custodian, no KMS, no per-identity keys, and no
key-derivation port is introduced by this ADR.

### D7 — The approver identifier is a subject reference

`approver_id` names an IdP *subject reference*, not a display identity. The boundary refuses a value
that contains `@`, whitespace, or is empty, naming the field. The doc comment, the module doc, and
every fixture state the same rule; the shipped demos stop using email addresses
(`cost.lead@example.com`, `treasury.ops@example.com` are today's fixtures).

This is the erasure decision. **The identity is the personal data; the decision is the evidence.**
The log claims "an authorized principal approved this exact intent at this time" — it does not claim
the ability to name the human, and it must not store anything that resolves to one.

Residual, recorded rather than promised: a caller may still put a resolvable identifier in a field
whose shape we validate but cannot interpret. That limit belongs in the claim ceiling (§ Claim
ceiling), not in a guarantee.

### D8 — Migration

- Read path: log first; if absent, the legacy map (honouring `RIGORIX_APPROVAL_STORE` and
  `RIGORIX_APPROVAL_CONSUMPTION_LEDGER` for **reading only**), then refuse to write until migrated.
- `rigorix approve ledger migrate` backfills each legacy record as one `decided` event and one
  `consumed` event per marker, then seals nothing (there is nothing to seal) and reports the counts
  it wrote.
- An absent or empty legacy file on a host that has no log is a **no-op with exit 0** naming the path
  (#1256).
- The dual-read window ends at 2.0.0. Until then both paths are tested; after it, the legacy reader is
  deleted rather than left to rot.

### D9 — Failure semantics (normative)

| state | behaviour |
|---|---|
| no log directory | empty fold; not an error; `doctor` says the log does not exist yet |
| empty log directory | empty fold; not an error |
| gap in `seq` | `ChainBroken`, naming the missing `seq` and the file that names it |
| corrupt tail frame | refused, naming the file and the parse error; never a partial fold |
| MAC failure | refused, naming `seq` and the keys tried; never "no history yet" |
| legacy map with no log | refused by name, with the migration command (#1247) |
| a terminal record with no consumption event | reported as a finding by the fold, naming node and step — not as a passing count (#1255) |

### D10 — Duplicate the chain primitive; do not extract it yet

`audit/application/chain.rs` is 295 lines and audit-specific. The approval log gets its own ~130-line
framing and verification over `ApprovalEvent`, and the two are kept independent. Extraction into a
shared kernel is triggered by a **third** user (capacity or effect commitment), not by two, and not
by the desire to avoid duplication.

### D11 — Erasure, when it is required, is ADR-016's recorded compaction

If a deployment must erase inside the log, it uses the mechanism ADR-016 already decided for the
audit ledger: key destruction over the payload, a retained
`{ seq, prev, erased: true }` frame, and a signed compaction checkpoint so the chain links across the
gap. This ADR builds none of it, because D7 removes the personal data from the frame. The trigger to
build it: (a) a field that accepts a display identifier is added, (b) a customer requires in-place
erasure proof, or (c) the log outlives the customer's IdP tenant.

## The precise claim (guarantee / non-guarantee)

**Guaranteed:**

1. Every approval decision and every consumption is recorded once and is never rewritten.
2. A deletion, reorder, duplication or insertion **inside** either log is detectable from that log
   alone.
3. A record's lifetime is reconstructible: decided, then consumed/expired/superseded, each with a
   time and an event identity.
4. Two writers produce two appends; neither can erase the other's event.
5. An absent log is distinguishable from an emptied log and from a broken one, by name.

**Not guaranteed:**

- Deletion of the **tail** of a log. Same boundary as ADR-016: closing it needs an external head (the
  anchor, or the enterprise ledger), not a storage change.
- The truthfulness of a host compromised *before* it appends.
- Cross-log atomicity: a `decided` event and its `consumed` event are in two logs, so a crash between
  them leaves a pending record — which is the correct, recoverable state, and is not a torn write.
- Anything about *who* the approver was beyond the reference in the frame.

## Consequences

**Positive**

- Writes are O(1) and contention-free; the lock, the whole-set seal, and the merge-on-write go away.
- `#1254`'s class becomes unrepresentable; `#1244`/`#1255`'s questions are answered by the fold;
  `#1249`/`#1251`/`#1253`'s counts become arithmetic over events; `#1256`'s absence case becomes an
  empty fold.
- The approval lifecycle becomes answerable: when it was consumed, by which run, and what happened
  after — a product capability the current shape cannot express.
- Key rotation becomes possible without re-signing history (`kid`).

**Negative**

- Reads fold events; the current read is already O(n) bytes (one document), so there is no regression
  until the log outgrows a single-document parse. A checkpoint is added when that is measured, not
  before.
- Two new failure modes must be handled with named errors: a corrupt tail, and a chain break that a
  bug could also produce. "Cannot verify" and "tampered" must read differently — the lesson of #1247.
- The dual-read window is a real cost and must end at a named version.
- `event_system` is *not* the home for this: it is an in-memory broadcast bus drained at end of run
  (`event_system/mod.rs`), and evidence may not depend on process lifetime.

**Neutral**

- Net code is roughly flat; what remains is structural rather than corrective.
- The SDK is unaffected: the ref shape (`ApprovalRecordRef`) and the envelope contract do not change.

## Rejected alternatives

- **Keep the map; strengthen the locking.** Rejected: this is the change already made six times. It
  makes writes O(n) and serialized, and it still cannot record a lifecycle transition — the reason
  ten of the eleven defects exist.
- **Per-marker MACs instead of a whole-set seal.** Rejected as insufficient: deleting an entry
  deletes its MAC, so a marker can be removed without detection. Only a chain over entries makes
  interior deletion visible.
- **Crypto-shredding per identity now.** Rejected as premature: it solves a problem D7 removes by
  contract. Deferred with a named trigger, and it will reuse ADR-016's compaction when triggered.
- **Event-source the audit envelope as well.** Rejected: the envelope is the signed, cross-repo
  artifact (SDK verifier, enterprise ingest, `RS-*`), and its position in the chain is already
  immutable. Only its content is mutable on re-save, which is a separate question and a separate
  probe.
- **A new bounded context, or a shared chained-log kernel.** Rejected: the language does not change —
  an approval event is an approval fact. A second owner would force translation between two
  representations of one fact, which is the defect ADR-011's vocabulary precedent exists to prevent
  (`IntentHashProvenance`, #1243).
- **Store the events in `event_system`.** Rejected: in-memory, drained at end of run.

## Implementation

| layer | change |
|---|---|
| domain | `ApprovalEvent` + `fold` + `ApprovalLogError` (named variants, no catch-all) |
| application | port unchanged in shape: `append(event)`, `load(node)`, `load_all()`, `verify()` |
| infrastructure | log repository (append/fold/verify), legacy map reader for the dual-read window, migration backfill |
| cli | `rigorix approve ledger migrate` retargeted; `doctor` reports per-log chain state and population counts |
| docs | `modules/approval.md` contract updated; claim ceiling extended per § Claim ceiling |

Phases, in order: (1) frame + append + verify with seeds; (2) fold + the read path switched behind the
port; (3) migration + dual-read; (4) delete the map, the lock, the seal and the unsealed-adoption path;
(5) fixtures and docs.

## Validation

Every invariant above is proven by a seed that must fail when the property is removed:

| seed | expected |
|---|---|
| two appenders write `seq = n` simultaneously | one succeeds, the other retries onto `n+1`; both events present |
| delete an interior frame | `ChainBroken`, naming the missing `seq` |
| rewrite an interior frame's payload | `prev` mismatch on its successor, naming both |
| delete the tail frame | fold succeeds; the log reports it cannot see beyond its head (not "verified") |
| two `decided` events for one `(node, nonce)` | both retained; fold reports the duplicate as a finding |
| `consumed` against a terminal record | retained; fold reports it as inert |
| no log directory | empty fold, exit 0, name the path |
| legacy map, no log | refused by name with the migration command |
| `approver_id` containing `@` | refused, naming the field |
| the MAC key removed | refused, naming the key and `seq` — never "no history yet" |

## Claim ceiling

New claims this ADR authorizes: an approval decision and its consumption are recorded once and a
modification to the log is detectable; a record's lifecycle is reconstructible.

Claims this ADR does **not** authorize, and which must not appear in docs or marketing: that the log
cannot be deleted (only that a deletion is detectable, and that a tail deletion is not); that the
approver's identity is recoverable; that cross-log atomicity exists; that the enterprise mirror
inherits any of this automatically.

## References

- `.pi/architecture/modules/approval.md` — the module contract this ADR changes
- `engine/src/audit/application/chain.rs` — the pattern, and the framing this ADR mirrors
- `.pi/architecture/decisions/ADR-016-audit-integrity-anchor.md` — the chain, its boundary, and the compaction mechanism reused by D11
- `.pi/architecture/decisions/ADR-011-approval-binding.md` — the binding this store records
- `docs/issues/` entries for `#1240`, `#1241`, `#1243`, `#1244`, `#1247`, `#1249`, `#1251`, `#1253`, `#1254`, `#1255`, `#1256`
