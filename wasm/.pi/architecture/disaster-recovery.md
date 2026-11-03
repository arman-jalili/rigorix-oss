# Disaster Recovery — rigorix-gate-wasm

**Canonical module:** `.pi/architecture/modules/portable-session.md`.
**Related:** ADR-002 (no persistence), ADR-010 (artifact format), ADR-013 (who verifies).

---

## 1. What can actually be lost

The crate is stateless by design. There is no database, no cache and no server-side state, so
there is nothing of ours to recover — and that is the point. What can be lost belongs to
someone else:

| Asset | Owner | Recoverable from |
|-------|-------|------------------|
| The record (signed envelopes + evidence refs) | **host** — localStorage, a download, an upload | the host's own copy; the crate never held it |
| The signing key | host (`KeySource`) | the host's key custody; the crate only ever saw an opaque `key_ref` |
| A live session | nobody | it is meant to be dropped: one run per session (invariant 4) |
| Fixture transcripts | host | the host's fixtures; a vector in git for conformance |
| The scenario | engine | the engine's own store; the crate only holds a digest |

If the host loses the artifact set, the record is gone. There is no "restore from our side",
because our side kept nothing — by design (ADR-002) and by requirement (the crate has no
filesystem in the browser).

## 2. Signature or key loss

`key_ref` in the artifact says which key produced the record, never the key itself.

| Situation | Consequence | Response |
|-----------|-------------|----------|
| The host loses a **private** key | existing records still verify with the matching public key; new records cannot be signed with it | issue a new key, publish its reference, keep the old public key for verification |
| The host loses a **public** key | existing records become `unverifiable(absent_key)` in that host — not `not_verified` | re-publish the public key; the records were never wrong |
| A key is **compromised** | signatures still verify; they just prove less | treat every record signed by that key as attributable to whoever held it; re-sign nothing (a re-signed record is a new record) |

The three-valued vocabulary exists precisely so a lost key does not become an accusation:
*we could not check* is not *it does not match*.

## 3. Format or release disaster

| Situation | Detection | Response |
|-----------|-----------|----------|
| A shipped bundle emits a broken container | host verifier reports `unverifiable(unsupported_artifact_format)` | roll the npm package back to the previous version; old records still verify under their own format's rules |
| A release shipped with the wrong version | release gate (`release.yml` version loop) fails; if it slipped, npm and crate versions disagree | yank the npm version, re-release at the engine's version |
| The comparator stopped comparing | the seeded divergence vector **matches** | revert the comparator change; every conformance result since is suspect |
| A published artifact set turns out to leak (path, PII, key material) | scanner found it in CI, or a report arrived | the bytes are public: revoke the key, publish a corrected record as a **new** record, state what leaked. Do not edit the old bytes — an edited record is not evidence |

## 4. Rollback

1. `npm dist-tag` the previous version as `latest` (or unpublish within the window).
2. Roll the crate version back on crates.io only by publishing a new patch; yanking blocks
   resolution for everyone.
3. Record the incident in `.pi/architecture/CHANGELOG.md` with the validator that should have
   caught it, and add the missing check — a rollback without a new gate invites the repeat.

## 5. What is *not* a disaster

- **A whole session vanishing on reload.** Sessions are not state; the record is.
- **Every run refusing `gate_unavailable`.** Correct until FR-011 links the engine seam: the
  gate could not evaluate, so it did not allow.
- **A record that will not verify on a machine with no public key.** `unverifiable`,
  correctly reported.

## 6. Drill (quarterly, ten minutes)

1. Take a committed vector, produce an artifact set in the browser demo.
2. Verify it in a Node process that has never loaded the module (criterion 13).
3. Delete the local artifact; confirm nothing in the crate can reproduce it (it should not).
4. Force `unverifiable` by removing the public key; confirm the host reports *could not
   check*, not *tampered*.
