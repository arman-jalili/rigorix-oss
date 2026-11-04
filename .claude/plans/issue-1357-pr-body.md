The report's M2 witness belonged to the discarded apparatus. Both entry points on the merged tree produce `c47b27f9…` at the same 853 B — a different byte string at the same length, which is why nothing caught it.

## What actually moved the bytes

Two changes, both now named in the report:

1. **The port.** The operation moved from the apparatus into `audit::application::pinned` with renamed fields (`template_id`, `producer_id`) and `effect_key` dropped.
2. **A hand-computed epoch.** The fixture's event instant shipped as `DateTime::from_timestamp(1_793_709_599, 0)` under a comment reading `11:59:59Z`. That epoch is **12:39:59Z** — forty minutes later than its own documentation. Found while fixing this: the constant was written when the panic-surface guard rejected `Utc.with_ymd_and_hms(…).unwrap()`.

The envelope is 853 B in both cases. A length check would not have caught either — the report's own 884-vs-877 lesson, pointed inward.

## Why it was invisible

`pinnedinputs_test.rs` asserted the **property** — two runs agree, one second differs — and never a **value**. Every check stayed green while the witness moved.

| | pins | digest |
|---|---|---|
| the documented pins (the witness) | `…0001` @ `12:00:00Z` | `c47b27f9…`, 853 B |
| one second off | `…0001` @ `12:00:01Z` | `662e92b6…`, 853 B |
| a different id | `…0002` @ `12:00:00Z` | `d5fdedae…` |
| unpinned control | — | `031232ce…` / `6ee9a632…`, 860 B |

The grid that ruled the old value out, recorded in the report as evidence rather than removed:

| `--pinned-ids` | `12:00:00Z` | `18:51:18Z` |
|---|---|---|
| `…0000` | `19a0352a` | `0b7a4f49` |
| `…0001` | **`c47b27f9`** | `5a2f6536` |
| `…0002` | `d5fdedae` | `00a20df9` |
| `…0003` | `8a8371f8` | `16e19303` |

## The fix is one assertion, and the value lives in one place

`tests/unit/pinnedinputs::the_documented_pins_produce_the_golden_digest` pins the literal for the documented pins — and asserts the 853 B length beside it, because a change in both means the subject changed, not just its content.

The report now **cites the test** and does not restate the number. A value kept in two places is two values that drift, which is exactly how this happened.

The event-instant construction is no longer a hand-computed epoch: it reads `NaiveDate::from_ymd_opt(2026, 11, 3).and_hms_opt(12, 39, 59)` — the instant that actually shipped — so the source cannot disagree with the bytes again.

### Seeds

```
template_id by one character  → golden fails:
  left:  "e254591ca4aa…"
  right: "c47b27f9300e…"
restored                      → 7/7 pass
grep -c <withdrawn value> docs/spike-portable-slice-2026-11.md  → 0
```

## One deviation, stated

The issue's criterion reads `grep -c 8b2cc83d docs/` → 0. It is **0 for the report**, and **6 for `docs/` as a whole** — all six in `docs/issues/m2-digest-is-the-apparatuses.md`, the issue's own filing record. Those files are immutable snapshots of what was filed (every one of them has exactly one commit, and `validate-docs.sh` excludes `docs/issues/**` from its scope), and the diagnosis is intelligible only with the old value present. Rewriting that record to satisfy a grep would be editing history to make a check green.

## Also fixed in passing

§2 still said the CLI half was **"not reached"** — a stale paragraph from before #1355 that an earlier edit left behind because the replacement silently matched nothing. That is the same failure mode as the digest itself, in the prose: an edit that reports success without checking it landed.

## Validation

```
cargo test --workspace                                4114 passed, 0 failed
cargo clippy --workspace --all-targets -- -D warnings clean
cargo fmt --all --check                               clean
cargo test -p rigorix-engine --test unit pinned       7 passed (6 + the golden)
validate-docs / guards / tests / canonical / wiring-claims   all exit 0
```

Closes #1357
