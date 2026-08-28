# Disaster Recovery Plan: identity

## Overview

Recovery plan for the identity module in the event of process crash, IdP outage, or identity-record corruption. The identity module is **stateless between calls** — each attestation is self-contained — but durable identity records (attached to execution state) and the online verification path (JWKS) have recovery considerations.

## RTO/RPO Targets

| Metric | Target | Notes |
|--------|--------|-------|
| RTO (Recovery Time Objective) | < 30s | No startup state to rebuild; service is stateless between calls |
| RPO (Recovery Point Objective) | < 1 execution | Identity records persisted atomically per execution |

## Failure Scenarios

### Scenario 1: Process Crash During Identity Record Write

**Impact:** The identity record for the in-flight execution may be absent or stale. The claim itself is already attached to the run in memory.

**Recovery:**
1. Process supervisor (systemd/launchd) restarts the process
2. On the next audit/closeout of the execution, `IdentityRepository::load(execution_id)` returns `None` (or the previous record — atomic write-rename guarantees the old file survives a crash mid-write)
3. If identity evidence is required for the execution, re-run attestation from the preserved `token_ref` (off-band verification) and `save()` the claim
4. Log: `"Identity record for {execution_id} recovered/absent"`

**Prevention:**
- Atomic write-rename pattern: write `{id}.identity.json.tmp` then `rename()` to `{id}.identity.json`
- Raw tokens never embedded in records — only `token_ref` locators, so a leaked record exposes no credentials

### Scenario 2: IdP Outage (Unreachable / Down)

**Impact:** Online verification (`JwksVerifier`) cannot run. **This is a designed, non-fatal condition** (ADR-012 fail-open with explicit marker).

**Recovery:**
1. **No action required for availability** — attestation proceeds; claims degrade to `IdentitySource::Unverified` (explicit marker, never silent); runs/approvals continue
2. The envelope records `identity: unverified`; audit evidence remains honest
3. **Post-outage**: when the IdP returns, re-attest if verified identity is needed (e.g., for approval binding); the preserved `token_ref` enables off-band verification of previously-issued claims
4. Alert (optional): `"IdP unreachable — identity attestation degraded to Unverified for N runs"`

**Prevention:**
- 5s JWKS fetch timeout bounds the degraded window's latency
- `NullVerifier` default keeps local development fully offline

### Scenario 3: IdP Signing Key Rotation (Unknown `kid`)

**Impact:** Tokens signed with the new key verify as `Unverified { reason: "no JWKS key for kid" }`.

**Recovery:**
1. Confirm the JWKS endpoint is serving the rotated key (the verifier fetches fresh JWKS per verification — no cache to invalidate)
2. If the verifier points at a stale/mirrored JWKS URL, update configuration to the canonical endpoint
3. Re-verify previously-attested claims with the updated verifier for Enterprise authorization

**Prevention:**
- JWKS is fetched per verification (no TTL cache), so rotation takes effect immediately
- Keep `kid` values in the JWKS document aligned with token issuance

### Scenario 4: Identity Record File Corruption

**Impact:** `IdentityRepository::load` fails to deserialize `{id}.identity.json` → `IdentityError::Internal`.

**Recovery:**
1. Treat the execution's identity as absent (`Ok(None)` path) — the run continues; the envelope marks `identity: absent`
2. Remove the corrupt file (prevents repeated failures)
3. If attribution evidence is required, re-attest from `token_ref` and `save()` a fresh record
4. Alert: `"Identity record corruption for {execution_id}: {error}"`

**Prevention:**
- Atomic write-rename eliminates partial-write corruption
- `token_ref`-only records mean no credential exposure in corrupt data

### Scenario 5: RSA/Verification Dependency Failure

**Impact:** `JwksVerifier` cannot build the RSA key from JWKS `n`/`e` or the `rsa` crate errors.

**Recovery:**
1. Malformed JWKS fields → `Unverified` (best-effort) — attestation continues degraded
2. Validate the JWKS document shape against the IdP
3. Update `rsa` crate if a library defect is suspected (pinned in workspace deps)

**Prevention:**
- Best-effort design: verification failure never fails the run
- deny.toml gates dependency licenses (MIT/Apache-2.0 allowed)

## Backup Strategy

| Data | Strategy | Schedule | Retention |
|------|----------|----------|-----------|
| Identity records (`{execution_id}.identity.json`) | Backed up with execution state files (same directory, same snapshot) | Per-execution (atomic write) | Lifetime of the execution record |
| Raw tokens | **Never persisted** — only `token_ref` locators | — | — |
| JWKS documents | Fetched on demand (no cache) | — | — |

## Restore Procedure

1. Restore the state directory snapshot (identity records come with it)
2. Validate a sample record: `IdentityRepository::load` round-trips
3. Re-attest from `token_ref` if a fresher claim is required

## Failover

The identity module has no active/passive topology — each process attests independently against the IdP. Failover concerns are:
- **IdP** → handled by the degradation path (Scenario 2)
- **JWKS** → fetch-on-demand, no cache to fail over

## Validators

| Validator | Status |
|-----------|--------|
| check_identity_contracts.sh | 17/17 pass (all interfaces implemented, no todo!() stubs) |
| check_identity_coverage.sh | 36 tests ≥ 15 minimum |
| validate-operations.sh | Tracing (SpanPrivacy), atomic writes, error handling, no unbounded waits |
