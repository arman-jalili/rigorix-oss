# Releasing Rigorix

This is the short release checklist. The changelog format and the release train
are defined in [CONTRIBUTING.md](CONTRIBUTING.md#release-history-and-the-changelog).

## The `Unreleased` section is written at merge time

A pull request that changes behavior, wiring, or a documented claim adds its
entry under `## [Unreleased]` in `CHANGELOG.md` **in the same PR**, with the
issue and PR links. The release manager renames the section; they do not
reconstruct it from git.

The 1.10.0 notes were reconstructed after the fact, and that section is still the
only complete record of the release. An empty `Unreleased` section means the next
release inherits the same work.

## Cutting a release

1. Confirm `## [Unreleased]` is non-empty, and every line names the issue and the
   PR that landed it.
2. Rename the heading to `## [x.y.z] - YYYY-MM-DD` and add the link reference to
   the GitHub Release on the next line.
3. Start a fresh `## [Unreleased]` at the top.
4. `rigorix-actions` is on the train: bump it to the same version. It was excluded
   once because the old gate failed for it — but an excluded crate never gets
   bumped, and `publish-crate.sh` then skips it as already published. A crate
   outside the gate is a crate that is never released.
5. Run `bash .pi/scripts/validate-docs.sh` (it is also part of the gating
   `validate-guards.sh`).

## What goes where

- **Added** — a behavior, surface, or guarantee that did not exist before.
- **Fixed** — a defect, a false claim, or a control that could not fire.
- **Changed / Docs / Internal** — as described in `CONTRIBUTING.md`.

If an entry cannot name the PR that landed it, it is not ready to be written.
