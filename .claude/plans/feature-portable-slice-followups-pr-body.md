Three issues, in the order they depend on each other: the subject of the size gate (#1353), the record it falsified (#1354), and the last unmeasured row (#1355). They are one branch because #1354's verification — "reproduces when re-run from the merged tree" — is only true if the corrected gate lands with it.

---

## #1353 — the size gates measured an artifact no host ships

`wasm-bindgen` emits a custom `name` section — the debug symbol table — of **33 485 B of 243 366 B** (13.8 %) on the empty seam and **341 785 B of 1 512 420 B** (22.6 %) with the engine linked. Both gates counted those bytes.

| | before | after |
|---|---|---|
| subject | the emitted file | the file a host loads |
| strip | none (`wasm-opt` claimed in the budget note; absent here and in CI) | `ci/strip_name_section.py` — a section walk (`id \| LEB128 size \| body`), no binaryen |
| where | — | the packaging path, so smoke, size and host verification all load **one** artifact |
| budget | 243 366 / 304 207 | **209 881 / 262 351** (+25 %), note no longer claims wasm-opt |
| presence | unchecked | the artifact must carry **no `name` section**, asserted before the size check |

The presence assertion is the load-bearing one: with a 262 351 B budget, an *unstripped* 243 366 B file still passes the size check. Seeded by skipping the strip — the module stays at 243 366 B, the size assertion passes, the presence assertion exits 1 and names the artifact. The helper also runs a 9-assertion self-test on every gate invocation, so a parser that could not see a `name` section cannot report it absent.

## #1354 — the verdict, corrected

The report concluded **branch C — that the bundle exceeded the ceiling** — from the emitted file. On the shipped artifact the engine-linked bundle is **1 170 635 B, 78.0 % of the 1 500 000 B ceiling**, and the stripped module passes all fifteen smoke assertions identically. Branch C did not fire; the narrowed-demo decision it produced is withdrawn.

The verdict paragraph, a per-measurement status table, the M3 section, the branch table's C row and the next steps carry the corrected numbers with the commands that produced them. The `regex` trim is recorded as unrun and unnecessary. Trap 9 is added: `wc -c` on a build output is not a size measurement of the deliverable.

## #1355 — M2's CLI half, and the carrier that made it cheap

The spike's `pinned_timestamp` was a public field on a public, literally constructible DTO that `server/tests/audit_scope_check_verify.rs` and `mcp/src/host/mod.rs` both construct — a semver break that compiles here and breaks a downstream user. Replaced by `PinnedInputs` (own type, `#[non_exhaustive]` from day one) through an additive `with_pinned_inputs`.

```
$ rigorix pinned-refusal --pinned-clock 2026-11-03T12:00:00Z --pinned-ids 018f5b8e-…-000000000001
pinned_clock=true timestamp=2026-11-03T12:00:00+00:00 bytes=853 sha256=8b2cc83d5aae…
$ cargo run -q -p rigorix-engine --example pinned_refusal -- --pinned-clock … --pinned-ids …
pinned_clock=true timestamp=2026-11-03T12:00:00+00:00 bytes=853 sha256=8b2cc83d5aae…
```

**Equal, and nothing normalises.** Seeds that prove the comparison can fail:

| seed | digest |
|---|---|
| pinned | `8b2cc83d5aae…` (853 B) |
| one second off | `2ac522bb33fc…` — same 853 B, different bytes |
| different id | `ab90dded46aa…` |
| unpinned control | `559d79d0f58a…` (860 B) |

853 ≠ 860 because a whole-second instant serialises shorter than a nanosecond one — recorded, not normalised. The operation lives in one place (`audit::application::pinned`), so the runtime is the only variable between the two digests.

**M1b now passes under its strict reading.** `git diff --stat` on the two literal-constructing files is empty, and engine 2915 / mcp 376 / server 63 / cli 118 / actions 460 pass with **no test edited** — the eleven edits the spike needed are gone with the field that forced them.

## Validation

```
cargo test --workspace                                4113 passed, 0 failed
cargo clippy --workspace --all-targets -- -D warnings clean
cargo clippy -p rigorix-cli --features spike-m2        clean
cargo fmt --all --check                               clean
wasm/ci/run-wasm-gates.sh                             10/10 (strip in the packaging path)
wasm/ci/run-host-verification.sh                      exits 0 against the stripped artifact
bash .pi/scripts/validate-*.sh                        11/11 exit 0 (incl. guards: 0 new panic sites)
```

Closes #1353
Closes #1354
Closes #1355
