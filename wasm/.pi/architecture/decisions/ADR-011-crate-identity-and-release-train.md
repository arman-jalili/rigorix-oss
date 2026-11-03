# ADR-011: Crate Identity, Version Lockstep and Release-Train Membership

**Status:** Accepted
**Date:** 2026-11-03
**Session:** 3baf6e5c-e352-4127-af09-a5e7fe0a2d29

## Context

ADR-001 decided the shape of the crate but never named it, versioned it or licensed
it. Three facts from the surrounding repository make that omission expensive:

1. **An unregistered crate is an unreleased crate.** `rigorix-actions` sat on 1.4.0
   for seven weeks because it was excluded from the version gate: `release.yml`
   iterates `for crate in engine cli mcp actions`, and anything outside that loop is
   never checked against the tag, so `publish-crate.sh` skipped it as "already
   published" on every release. The same hole opens the moment a new member exists
   and is not added to that loop.
2. **The npm name is a public commitment.** `rigorix-gate` and `rigorix-gate-wasm`
   both return `E404` on the registry today (measured), so the name is available and
   also squat-able. The demo will import it, so the name appears in GTM copy.
3. **Versions here denote exactly one artifact.** The workspace already pins
   `license = "MIT OR Apache-2.0"` and the crate must inherit it rather than restate
   it — the SDK's Apache-2.0-only position is a separate open decision and must not
   leak into this crate by copying.

Publication ordering adds a fourth constraint: a `portable` feature can only be
consumed by a publishable dependent **after that feature is itself published**, so the
train has a forced sequence that nothing in the current docs states.

## Decision

| Question | Decision |
|----------|----------|
| Crate name | `rigorix-gate-wasm` (directory `wasm/`, manifest `wasm/Cargo.toml`) |
| npm package | `rigorix-gate` — the name the host imports; the crate name is the Rust artifact |
| Version | Lockstep with the release train: `1.12.x` alongside engine/mcp/cli/actions. Never independent. |
| Licence | Inherited: `license.workspace = true` → `MIT OR Apache-2.0`. Not restated in the manifest. |
| Edition / lints | Inherited from the workspace (`edition = "2024"`, `[lints] workspace = true`), so `disallowed_types = "deny"` — the spawn guard — applies to this crate from its first line |
| Engine dependency | `rigorix-engine = { path = "../engine", version = "<train>", features = ["portable"] }` |
| Publish order | engine (carrying `portable`) → `rigorix-gate-wasm` (crates.io) → `rigorix-gate` (npm) |

Registration is a checklist, not an assumption. A crate that is not in all seven places
is not on the train:

1. `Cargo.toml` → `members += "wasm"`.
2. `wasm/Cargo.toml` exists with the workspace-inherited fields and the engine pin.
3. `release.yml` version-consistency loop: `for crate in engine cli mcp actions wasm`
   — the *directory* name, matching the loop's existing convention. ✅ (#1315)
4. `release.yml`: a publish step `publish-crate.sh rigorix-gate-wasm wasm/Cargo.toml`,
   ordered after engine. ✅ (#1346, `publish-wasm`)
5. `release.yml`: an npm publish step at the tag version — this repo had zero `npm publish`
   invocations (the SDK publishes from its own repo). ✅ registered (#1346,
   `publish-wasm-npm` + `.github/scripts/publish-npm-package.sh`), ⏸ **publishing is held
   back** while `wasm/ci/bundle-budget.txt` says `state: awaiting-packaging`: a module with
   no exports is a version a consumer would pin and could not use.
6. `.pi/panic-baseline.tsv` regenerated (`validate-panic-sites.sh --update`) so the new
   crate's surfaces are classed; `.pi/scripts/validate-workspace-pins.sh` needs no
   change — it reads `cargo metadata` and picks the member up automatically. ✅
7. *(added by #1346)* `.pi/scripts/validate-release-train.sh` derives these points from
   `cargo metadata` and fails when a publishable member is missing any of them — the
   checklist above is prose, and prose cannot notice the next member. It must not be
   read as evidence that this one is on the train; the guard is.

## Consequences

### Positive
- The crate cannot silently fall off the train. The `for crate in …` entry alone did *not*
  achieve that — the crate joined the workspace and the CI while two registration points
  were still missing, and every gate stayed green because none of them looked here. What
  achieves it is `validate-release-train.sh`, which derives the train from the manifests
  rather than trusting the literal that the `actions` defect showed can be wrong.
- Version identity across crates.io and npm is enforced by the same tag check that
  already guards the other four.
- Licence and edition stay single-sourced; the SDK's licence question cannot be
  answered by accident here.

### Negative
- The train now publishes two registries. A crates.io success followed by an npm
  failure leaves the release half-published, so the npm step must be idempotent
  (`npm view` before `npm publish`) exactly as `publish-crate.sh` already is.
- The first published version may lag the engine by one release: the `portable`
  feature must ship in engine `1.12.x` before `rigorix-gate-wasm 1.12.x` can resolve.

### Neutral
- `server` stays outside the train at 0.1.0 and unpublished; this crate joins the
  published four, giving five.

## Alternatives Considered

- **A separate `rigorix-wasm` repository.** Rejected: a path dependency on an
  unpublished engine feature is impossible across repos, and the conformance job needs
  the CLI built from the same commit — a cross-repo job reintroduces the version-skew
  defect this train exists to remove.
- **Own semantic range (`0.x`)** for the wasm crate. Rejected: the whole claim is that
  it produces the same artifacts as the engine; divergent versions make that
  unverifiable.
- **Publish from `rigorix-gtm`** because the demo is the first consumer. Rejected:
  a marketing repo cannot be the distribution channel for a partner-embeddable gate.

## Implementation

1. Add the member and manifest; `cargo metadata` then reports five publications.
2. Extend the `release.yml` loop and add both publish steps (crates.io, npm).
3. Seed `.pi/panic-baseline.tsv` for the new crate; keep the row set empty at first.
4. `validate-workspace-pins.sh` must pass with the new pin before any release tag.

## Validation

- `bash .pi/scripts/validate-workspace-pins.sh` → pins match across all members.
- A dry-run tag bump fails if `wasm/Cargo.toml` disagrees with the tag (loop entry).
- `npm view rigorix-gate version` equals the tag after a release.

## References

- `.github/workflows/release.yml` (version loop; `publish-crate.sh` delegation)
- `Cargo.toml` (`[workspace.package]`, `[workspace.lints.clippy]`)
- ADR-001 (architecture pattern), ADR-009 (conformance harness)
- `.pi/scripts/validate-workspace-pins.sh`, `.pi/scripts/validate-panic-sites.sh`
