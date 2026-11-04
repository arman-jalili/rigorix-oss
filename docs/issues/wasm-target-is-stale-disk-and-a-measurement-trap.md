---
guardian_issue:
  id: "ISSUE-WASM-TARGET-IS-STALE-DISK-AND-A-MEASUREMENT-TRAP"
  title: "`wasm/target` is 809 MB of unused disk holding a 368-byte stale artifact — and it just produced a false size measurement, mine"
  epic: "portable-slice"
  component: "Tooling"
  module: "wasm-seam"
  status: planned
  priority: low
  tracking_issue: 1398
  dependencies: []

  intent: |
    `wasm` is a member of the root workspace (`Cargo.toml:2`, `members = [ …, "wasm" ]`), so the crate
    builds into the **repository-root** `target/`. `wasm/target/` is a leftover from before that, and
    it still contains `wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm` at **368 bytes** — the
    empty-module state from #1349's era.

    It is a measurement trap, and it worked: while checking C.4's size claim I built the crate, read
    the artifact from `wasm/target/`, measured **368 B**, and briefly concluded the crate no longer
    exports anything. The real artifact, in the root `target/`, is 2 551 280 B raw / 1 706 234 B
    shipped. A directory that holds a plausible-looking artifact for a path nobody builds into is
    worse than no directory.

    It is also 809 MB of dead disk on a machine where the cold-build cost is already the bottleneck.

  in_scope:
    - "`wasm/target` is deleted from the working tree, and `.gitignore` covers it if it is not already"
    - "The runbook states where the crate builds: the workspace-root `target/`, because `wasm` is a workspace member — one sentence, in the same table that lists the build commands"
    - "The packaging script's raw-artifact path is confirmed to be the root target, and a measurement taken from `wasm/target` is impossible because the directory does not exist"

  out_of_scope:
    - "Restoring `wasm` as its own workspace root. ADR-011's separate-root argument was about the release train; the workspace has since chosen membership, and the profile question that motivated a separate root is handled by the packaging path (#1386)"
    - "CI caches, which are keyed on the root target already"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "`wasm/target/` (deleted); `.gitignore`; the runbook's build table"
    interfaces:
      - "None"

  citations:
    - file: "Cargo.toml"
      note: ":2 the workspace members, including `wasm`"
    - file: "wasm/ci/run-module-smoke.sh"
      note: ":36 `RAW=\"$TARGET_DIR/wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm\"` — `TARGET_DIR` is the workspace root's"
    - file: "wasm/.pi/architecture/runbook.md"
      note: "the build-commands table the sentence joins"

  verification:
    - "`test ! -d wasm/target` and the build still produces `target/wasm32-unknown-unknown/release/rigorix_gate_wasm.wasm`"
    - "`git status` stays clean after a build: nothing under `wasm/` is produced"
    - "The runbook's build row names the workspace-root target, and `grep -c 'wasm/target' wasm/.pi wasm/ci` returns 0"
---

# A stale artifact at a path nobody builds into

The trap is not the disk, it is the plausibility: 368 bytes is exactly what the empty-module state
produced (#1349), so it reads as a real measurement of a real problem. I made that mistake and filed
nothing for an hour because the number was *too* believable.

The rule it illustrates is the one this epic keeps re-learning, now with a path in it: **a measurement
must name the artifact it measured, including where it was built.** Two target directories is two
answers to "how big is it", and only one of them is current.
