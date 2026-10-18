# Issue #1071 (reopened) — conformance guard for the integrity vocabulary

**Branch:** `issue/1071-conformance`

## State on entry
The code half landed in PR #1077 (`AnchoredPending`, decided from `(mode, head)`).
Reopened because the value was emitted before `rigorix-sdk` knew it, and **no
conformance case set `history_integrity` at all** — `grep -rn history_integrity
engine/tests/conformance/` was empty, so the divergence was untested rather than
absent. SDK #48 (`7c0a6c4`) has since added the value.

## Work
`engine/tests/conformance/envelope.rs` — three cases through the real factory:

| case | asserts |
|---|---|
| `startup_window_envelope_is_anchored_pending_and_conforms` | anchored mode + no head ⇒ `anchored_pending`, `anchor_head` null, validates, still HMAC-verifies |
| `anchored_label_requires_and_carries_a_verified_head` | head ⇒ `anchored`, head bound, validates |
| `history_integrity_vocabulary_is_closed_and_the_labels_are_distinct` | all three emitted variants validate; an invented value does **not**; pending ≠ anchored on the wire |

Needs `async-trait` as a dev-dep (an anchored runtime takes an async history
source). CI checks out `rigorix-sdk@main`, which carries the value.

Docs: module doc (cross-repo contract + ordering), runbook (release-order section).
