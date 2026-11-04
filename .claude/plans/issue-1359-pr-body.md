Two residues of #1357's class, one row away in the document the epic rests on. Prose about provenance, not a claim that can be false without CI noticing — which is why it is low priority, and why it was still worth doing: a reader who finds row B contradicting row D discounts the whole table.

## M4 now cites merged-main evidence

| | report said | report now says |
|---|---|---|
| pinned | `32211b9d…` ×2, 877 B | `c47b27f9…` ×2, 853 B — the same witness M2 cites, pointing at the golden test |
| unpinned | `4fe4487d…` / `d1e9cf0d…`, 884 B | **no digest** — "differs every run, 860 B" |

The apparatus values stay, in §2, labelled branch-era: they are the evidence that the fixture moved, and deleting them would leave the correction unexplained. §2's block now opens by saying they come from `examples/spike_m2_envelope.rs` and are not reproducible on `main`, so a reader cannot mistake them for current.

Measured, on merged main:

```
in-crate  run 1  bytes=860 sha256=ad2d6cc1…
in-crate  run 2  bytes=860 sha256=6b5dbdbe…
CLI       run 1  bytes=860 sha256=922ac970…
CLI       run 2  bytes=860 sha256=e6e22ef2…
```

Four runs, four digests, one length. That is the control's *shape* — the digests are deliberately absent from the report, because quoting one would ask a reader to run a command that cannot agree, and the resulting failure is indistinguishable from a broken claim.

**The same defect was in §2's control row from #1357**, one row down from the fix that issue was about: it listed `031232ce…` / `6ee9a632…` as checkable values. Now: "no digest claimed — every run differs, at 860 B in both entry points."

## Row B reads as historical

| | before | after |
|---|---|---|
| **B** | "Fires in part. In-crate: reached… CLI: not reached" | "Fired at the time, superseded (#1355) — not a current verdict… #1355 then reached the CLI half, so the condition no longer holds and D covers this row" |
| **D** | "Fires" | "Fires" — now the only present-tense verdict |

A branch that fired and was then overtaken is part of the record, so B stays in the table rather than being deleted.

## Two traps, because this thread produced one defect three times

- **10** — a measurement against an artifact that predates the edit is not a measurement. A gate passing on a 368-byte module (#1349), a size gate weighing a file no host loads (#1353), a digest read from a stale binary (#1357). One sentence: **no measurement without a rebuild in the same command.**
- **11** — an unpinned run's digest is not evidence and must not be quoted. State what varies, never a value only one run produced.

## Criteria

```
cargo run -q -p rigorix-engine --example pinned_refusal -- --pinned-clock 2026-11-03T12:00:00Z \
  --pinned-ids 018f5b8e-0000-7000-8000-000000000001
→ c47b27f9…, 853 B                              ← every current-era witness is this value

unpinned ×2 → two different digests, 860 B      ← the report claims none
grep -c 32211b9d docs/spike-portable-slice-2026-11.md → 2   ← both inside the branch-era block
row B's verdict cell → contains "superseded"
```

Validators: docs / guards / tests / canonical / wiring-claims / script-drift / shell-exit-status / ci / integration / security / architecture — all exit 0.

Closes #1359
