# feat: consequence-gating batch — spec, providers, fact projection (#1000–#1002)

One related batch (epic **consequence-gating**), three commits in dependency
order: **#1002** (the claim-bounding spec) → **#1000** (the consumer: external
determination) → **#1001** (the producer: attributable facts).

---

## #1002 — Consequence Gating v1.0 specification (docs only)

Turns ADR-017 + the module doc + prose caveats into a **versioned, citable**
specification with an enumerated jurisdiction and a falsifiable protocol.

- `docs/spec/consequence-gating-v1.md` — jurisdiction (**NC-1…NC-8** non-claims),
  evaluation contract, evidence model (SpanPrivacy, redaction-at-construction),
  interfaces, failure semantics, and the three-layer **claim-ceiling model**.
- `docs/spec/validation-protocol-v1.md` — **7 falsifiable cases**, each with an
  explicit *pass* **and** *fail* condition; **non-inheritance is Case 4**, both
  directions.
- `docs/spec/terminology.md` — normative vocabulary (`determination`, `authority`,
  `evidence`, `verified`, `record`, `established`, `unknown`, `fact`).
- `docs/spec/README.md` — index, citation + versioning policy, claim ceilings.
- `README.md` links the spec and bounds the public claims; the engine module doc
  points at it.

No code change.

---

## #1000 — External determination providers, non-inheriting

A precondition may be satisfied by an operator-configured external determination
service (HTTP), evaluated at the **same** dispatch enforcement point as the argv
check.

**Non-inheritance (the load-bearing property).** A provider permit is *evidence,
not authority*: it passes only its own precondition, every other matching
precondition is still evaluated, and a provider deny also refuses. Both
directions are tested (`provider_permit_does_not_override_our_own_refusal`,
`provider_deny_overrides_our_own_permit`). There is no "provider says allow"
short-circuit.

**Fail-closed** — unreachable, timeout, non-2xx, unparseable, and unknown
determination vocabulary all map to `error`/`unknown` → refuse. Unknown never
becomes authority.

**Transport** treats the provider as untrusted: `reqwest` with redirects
disabled, per-request deadline, 64 KiB body cap, credential read from the env
named by `credential_ref` and never logged or recorded, endpoint recorded
redacted (`scheme://host/<path sha256:…>`).

**Evidence** is bound using **in-contract** fields — no SDK schema change
(contract-first sequencing, D-011/D-012): for a provider precondition
`check_digest` is the one-way hash of the **request** bytes and `authority_digest`
of the **response** bytes; the provider id, redacted endpoint, determination and
`observed_at` go into the redacted, signed `summary`.

Files: `engine/src/precondition/domain/provider.rs` (new),
`…/infrastructure/provider_client.rs` (new), `…/application/service_impl.rs`,
`…/domain/{precondition,error,mod}.rs`, TOML surface, ADR-017 §R4 + module doc,
`engine/tests/unit/precondition/provider/provider_test.rs` (new, 8 tests).

---

## #1001 — Authority-fact projection (producer side)

Exposes our own signed, hash-chained history as **attributable present-state
facts** — what authority was checked, when, with what outcome, with what digests,
at what chain position — without asserting present standing.

- `AuthorityFact` + `EpistemicStatus` (`ESTABLISHED` / `NOT_ESTABLISHED`):
  absence is recorded explicitly, never rendered as adverse, never promoted.
- `ClaimCeiling` carried **as data** (`asserts_present_standing: false`).
- `AnchorRef` = chain tip + `history_integrity` + producer.
- `canonical_bytes()` (recursively key-sorted JSON) + `digest()` → the projection
  is offline-checkable and a **tampered projection changes the digest**.
- `ProjectAuthorityFacts::project` derives facts from `precondition_findings[]`.

Files: `engine/src/audit/domain/authority_fact.rs` (new),
`…/application/projection.rs` (new), `engine/tests/authority_fact_projection.rs`
(new, 5 tests).

---

## Acceptance criteria

| Issue | Criterion | Status |
|-------|-----------|--------|
| #1002 | versioned + citable spec; enumerated non-claims; ≥3 falsifiable cases; claim ceiling; terminology; README bounded | ✅ |
| #1000 | 1 declared + invoked at dispatch only | ✅ test |
| #1000 | 2 provider ALLOW + our rule REFUSES → refused | ✅ test |
| #1000 | 3 provider BLOCK + our rule ALLOWS → refused | ✅ test |
| #1000 | 4 unreachable/timeout/non-2xx/unparseable/unknown → refused, fail-closed | ✅ (unit; HTTP client maps each to Unknown/error) |
| #1000 | 5 record binds provider id, redacted endpoint, request+response digests, determination, observed_at | ✅ **via in-contract fields** (`check_digest`/`authority_digest`/`summary`) |
| #1000 | 6 zero absolute paths in the signed record | ✅ (redaction at construction, reused helpers) |
| #1000 | 7 absent attribution unambiguous | ✅ (existing `AttributionReason`) |
| #1000 | 8 additive; old envelopes verify | ✅ no schema change |
| #1001 | 1 offline-verifiable | ⚠️ canonical bytes + digest implemented; **published SDK-verifier check deferred** |
| #1001 | 2 ESTABLISHED vs NOT_ESTABLISHED | ✅ test |
| #1001 | 3 provenance on every fact | ✅ test |
| #1001 | 4 machine-readable claim ceiling | ✅ test |
| #1001 | 5 zero paths/usernames/secrets | ✅ test |
| #1001 | 6 chain position + anchor ref | ✅ test |
| #1001 | 7 tampered projection detected | ✅ test (digest changes) |

## Explicitly deferred (not in this PR)

1. **Structured provider evidence fields** on the envelope
   (`provider_id`/`provider_request_digest`/…) — these are a signed-record
   contract change and must land **SDK-first** (D-011/D-012, as #994 did); this
   PR binds them with existing in-contract fields instead.
2. **#1001 CLI/MCP surface** and verification with the **published** SDK
   verifiers (the projection has its own canonical form + digest; wiring the
   `rigorix-verifier` check is a follow-up).
3. ADR-017 §R4's "signed finding binds …" is satisfied by the
   `check_digest`/`authority_digest`/`summary` mapping above.

## Validation

```
cargo fmt --all --check && cargo clippy --workspace --all-targets -- -D warnings
cargo test --workspace
bash .pi/scripts/validate-guards.sh && bash .pi/scripts/validate-guards.sh --self-test
bash .pi/scripts/validate-{ci,tests,security,operations}.sh
```

All green (validate-ci 5/5, validate-tests 3/3, validate-security 2/2,
validate-operations 3/3; guards pass, no new panic/unsafe/secret sites).

The advisory `validate-architecture.sh` / `validate-canonical.sh` still report
their **pre-existing flat-heuristic false positives** ("no `src/domain/`") — the
engine uses module-layered DDD; unrelated to this change.

Refs #1000 · #1001 · #1002
