---
guardian_issue:
  id: "ISSUE-WASM-CRATE-IS-NOT-REGISTERED-ON-THE-RELEASE-TRAIN"
  title: "rigorix-gate-wasm is a workspace member and a mandatory CI stage, but no release step names it — the defect ADR-011 was written to prevent"
  epic: "release-integrity"
  component: "Release"
  module: "release-train"
  status: planned
  priority: high
  tracking_issue: 1346
  dependencies:
    - "wasm ADR-011 — the six registration points"

  intent: |
    `rigorix-gate-wasm` is in `[workspace] members`, is covered by `validate-workspace-pins.sh`
    ("scanned 5 intra-workspace pin(s) across 6 member(s)"), and its gates run as a **mandatory** CI stage
    (`CI_MANDATORY_STAGES="lint build test audit conformance shell wasm-gates"`). Its manifest says
    `version = "1.12.1"` and does **not** set `publish = false`, so it is intended to ship.

    Nothing in `release.yml` knows it exists:

    ```
    $ grep -n 'gate-wasm|wasm/Cargo.toml' .github/workflows/release.yml
    (none)                      # the version-consistency loop is still: for crate in engine cli mcp actions
    $ grep -c 'npm publish' .github/workflows/release.yml
    0
    ```

    Consequence, exactly the one recorded in `rigorix-actions`:

    - **No version check.** On the next tag, `1.12.2` will be compared for four crates and not for this one;
      the crate can sit on `1.12.1` — or drift further — while every gate stays green, because nothing
      asserts the tag equals its version.
    - **No publish.** No step invokes `publish-crate.sh rigorix-gate-wasm wasm/Cargo.toml`, so the version
      can never reach crates.io, and `cargo publish` of a dependent would fail to resolve it.
    - **No npm package.** The host-facing name (`rigorix-gate`) has no publish step anywhere in this
      repository. The demo cannot pin it because it does not exist.

    The crate's own ADR-011 states all six registration points and names this failure mode, with the
    `rigorix-actions` 1.4.0 defect as the reason it exists. Two of the six — a release-train loop entry and
    publishing — are not done, while criteria 24 and 27 in the module doc are marked complete.

  in_scope:
    - "`release.yml`: the version-consistency loop becomes `for crate in engine cli mcp actions gate-wasm`, so a tag whose version does not match `wasm/Cargo.toml` fails before anything publishes"
    - "`release.yml`: a `Publish wasm` step after `Publish engine`, delegating to `publish-crate.sh rigorix-gate-wasm wasm/Cargo.toml` so it is idempotent on replay"
    - "`release.yml`: an npm publish step for `rigorix-gate` at the tag version, idempotent the same way (`npm view` before `npm publish`), and a secret the release has not needed before"
    - "Criteria 24 and 27 are not marked met until a tag has produced both artifacts, and a guard asserts the loop membership from the workspace member list rather than trusting the loop's literal"

  out_of_scope:
    - "Publishing before FR-011 lands: the crate can be registered and still be held back by the `engine` feature being off — registration is about not drifting silently, not about shipping today"
    - "The SDK's separate npm channel, which publishes from its own repository"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: `.github/workflows/release.yml` — loop entry, crates.io step, npm step"

  citations:
    - file: ".github/workflows/release.yml"
      note: "the `for crate in engine cli mcp actions` loop; the publish steps after it"
    - file: "wasm/Cargo.toml"
      note: "version = 1.12.1, no `publish = false`"
    - file: "wasm/.pi/architecture/decisions/ADR-011-crate-identity-and-release-train.md"
      note: "the six registration points and the `rigorix-actions` precedent"
    - file: ".github/scripts/ci_stages.sh"
      note: "CI_MANDATORY_STAGES already includes wasm-gates — CI knows the crate, the release does not"

  verification:
    - "a tag with a mismatched `wasm/Cargo.toml` version fails the version job"
    - "a dry-run release publishes `rigorix-gate-wasm` on crates.io and `rigorix-gate` on npm at the tag version, idempotently on replay"
---

# ISSUE-WASM-CRATE-IS-NOT-REGISTERED-ON-THE-RELEASE-TRAIN

## Measured

```
[workspace] members       = [engine, cli, actions, mcp, server, wasm]        ✅
validate-workspace-pins   = "5 intra-workspace pin(s) across 6 member(s)"     ✅
CI_MANDATORY_STAGES       = "… shell wasm-gates"                             ✅
release.yml version loop  = for crate in engine cli mcp actions              ✗ no gate-wasm
release.yml publish steps = engine, cli, mcp, actions                        ✗ no wasm
npm publish in repo       = 0                                                ✗ no package
wasm/Cargo.toml           = version 1.12.1, publish not disabled             ✗ intended to ship
```

## Why this is the same defect, one week later

`rigorix-actions` sat on 1.4.0 for seven weeks because it was outside the loop: every release skipped it
as "already published", and nothing compared its version to the tag. `ADR-011` was written in this crate
precisely to make that impossible — and the crate then joined the workspace and the CI without joining
the train.

Two of the six registration points are missing, and the module doc marks criteria 24 and 27 complete.
The gates are green because no gate looks at this.

## The required end state

A workspace member that ships is named in the version loop, has a crates.io step, and — when it has a
host-facing npm package — an npm step. Registration is independent of readiness: FR-011 can hold the
`engine` feature off while the train still notices the crate exists and refuses to let its version drift.
