# Runbook — Rigorix Operations

## Deployment prerequisites

What the engine relies on the deployment to supply — the sandbox boundary that makes
process spawning safe to enable at all, and the executable-by-executable allowlist
answer — is in [deployment.md](deployment.md). A deployment that has not declared
`RIGORIX_SANDBOX=container` (or `vm`) cannot construct `run-command`; the refusal names
what the child can reach.

## Incident Response

| Severity | Response Time | Escalation Path |
|----------|--------------|-----------------|
| P1 (Critical) | < 15 min | On-call → Engineering Lead |
| P2 (High) | < 1 hour | On-call → Engineering Lead |
| P3 (Medium) | < 4 hours | Engineering Team |
| P4 (Low) | Next business day | Issue triage |

## Common Incidents

### CI Pipeline Failure
1. Check GitHub Actions logs for the failing job
2. Run `bash .pi/scripts/local-ci.sh --stage=<stage>` locally
3. Check `.pi/output/ci-report-*.txt` for details

### Build Failure
1. Verify `cargo check -p rigorix-{engine,cli,actions}` passes
2. Check for dependency issues: `cargo audit`
3. Run `cargo clean && cargo build`

### Test Flakiness
1. Run the failing test 3 times: `cargo test <test_name> -- --count 3`
2. If sporadic, check for filesystem race conditions
3. Add `.flush().await` and `.sync_all().await` to file operations

## Rollback Procedure

1. `git revert HEAD` to undo last commit
2. `git push origin main` to deploy revert
3. Verify CI passes after revert
4. Create a fix branch from the revert point

## On-Call

- Primary: @arman-jalili
- Escalation: Create GitHub issue with `incident` label

## Releasing, and knowing a release is real

Four things this repository learned the expensive way, each now with a check behind it:

- **A workflow job that has never executed is not a job that works.** `publish-wasm-npm` was wired for
  two releases and skipped both times (the bundle state was `packaged`); on its first real run it failed
  in 0.2 s because the job had no `wasm-bindgen` CLI. The CLI version is resolved from `Cargo.lock` by
  `.github/actions/wasm-bindgen`, which both `ci.yml` and the npm job call — one owner, no second pin.
- **The tree agreeing with itself is not the artifact agreeing with the contract.** 1.14.0 shipped
  `kind: "capacityexceeded"` with 207 crate tests, a frozen evidence-kind table and fourteen conformance
  tests passing, because nothing had ever driven the tarball the registry serves. `wasm/ci/verify-published-module.mjs`
  now asks npm for the declared version, replays the committed capability vectors through it, and asserts
  the decision digest and the wire words — every number derived from the vector's own world. The `verify-published`
  job gates the release page.
- **A re-run uses the tag's commit, not `main`.** A fix to the release workflow cannot rescue a run on an
  existing tag: the workflow file comes from the tagged commit, so the sequence is fix → commit → move the
  tag → push (`git tag -f -a vX.Y.Z -F <msg> <commit>`). Every publish script skips an existing version, so
  the re-run publishes only what is missing.
- **A claim about a registry comes from the coordinate's metadata.** `.../crates/<name>/<version>` for
  crates.io, `maven-metadata.xml` plus the artifact itself for Central — never a URL composed from what
  one expects the layout to be. A directory listing that 404s proves nothing; Central serves no listings.
