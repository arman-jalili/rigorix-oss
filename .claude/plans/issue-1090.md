# Issue #1090 — the durable artifact for retry-replaces-guard

**Branch:** `fix/1090-retry-guard-test`

## The interaction (each decision correct alone)
| decision | source | consequence |
|---|---|---|
| the service owns the guards | #1049 | `guards: HashMap<ReservationId, ReservationGuardImpl>` |
| an idempotent retry returns the **same** reservation id | #1052 | the retry is not a second hold |

Together: the retry registered a second guard under an existing id, replacing the
owner; the replaced guard's `Drop` released the hold; the caller's `commit` then
hit `Contended { … already Released }`. Scheduling-dependent — the demo failed one
run in three.

## Fix (already the shape #1090 prescribes)
`Entry::Vacant` so the retry path constructs no guard at all: nothing to drop, no
race.

## Tests — the point of this issue
| test | level | why |
|---|---|---|
| `an_idempotent_retry_keeps_exactly_one_guard` | in-module unit | asserts the **invariant** (one owner per id) and drains the runtime with `yield_now` (no sleep, #1081) |
| `an_idempotent_retry_does_not_release_the_hold` | integration (`tests/unit`) | the behaviour through the public API: exposure survives the retry, `commit` succeeds |

Falsified: reverting to plain `insert` fails the unit test deterministically
("a retry must not release the hold it just returned"), no scheduling luck.
