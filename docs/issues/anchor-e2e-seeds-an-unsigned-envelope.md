---
guardian_issue:
  id: "ISSUE-ANCHOR-E2E-SEEDS-AN-UNSIGNED-ENVELOPE"
  title: "demo/anchor-e2e cannot pass: it seeds an envelope with signature null, and ingest rejects unsigned envelopes"
  epic: "docs-and-demos"
  component: "Demo"
  module: "anchor-e2e"
  status: planned
  priority: medium
  tracking_issue: 1278
  dependencies: []

  intent: |
    Step 5 of `demo/anchor-e2e/run.sh` hand-writes a genesis envelope and posts it to the enterprise
    ingest to seed prior evidence:

    ```json
    { …, "signature": null, "producer_id": "local", "sequence": 0, "prev_hash": null,
      "history_integrity": "local_unanchored" }
    ```

    Ingest verifies the envelope HMAC whenever a key is configured, and the development default
    (`AUDIT_ENVELOPE_HMAC_SECRET`) is one. Measured on a fresh throwaway Postgres with migrations
    applied and `seed.sql` loaded:

    ```
    POST /api/v1/audit/oss-envelope   →  HTTP 400
    {"error":{"code":"INVALID_SIGNATURE","message":"Invalid envelope HMAC signature for
      execution_id=… (scheme: HMAC-SHA256 over canonical envelope bytes, keyed by
      AUDIT_ENVELOPE_HMAC_SECRET)"}}
    ```

    The same with `AUDIT_ENVELOPE_HMAC_SECRET=""` — the development default still verifies, so no
    environment removes the check. `curl -fsS` exits non-zero on the 400, and `run.sh` stops before
    step 6, which is the step that actually asserts the four outcomes. So the demo has been unable to
    reach its own assertion for as long as ingest has verified signatures; nothing else about it is
    exercised.

    Note the failure is silent about the contract: the exit is `22` from `curl`, and the body that
    names `INVALID_SIGNATURE` is discarded by `>/dev/null`.

  in_scope:
    - "The seed envelope is produced by the engine's own envelope type and signed with the same key the enterprise verifies against — a small helper (a `cargo run --example` or a bin in `demo/anchor-e2e/`) that builds the genesis envelope, signs it, and prints it, so the canonical bytes are the engine's and not a second copy in a shell heredoc"
    - "`run.sh` sets `AUDIT_ENVELOPE_HMAC_SECRET` for the enterprise AND for the helper from one variable, so the two cannot drift"
    - "A non-2xx from the seed POST prints the response body before exiting"
    - "The demo exits 0 only when the four outcomes in step 6 pass, as documented"

  out_of_scope:
    - "Accepting unsigned envelopes in development mode; ingest verifying the signature is the documented behaviour"
    - "Re-implementing the canonical envelope bytes in Python or in the shell"

  affected_layers:
    domain:
      - "None"
    application:
      - "None"
    infrastructure:
      - "Modified: `demo/anchor-e2e/run.sh` and a new small helper that signs the seed envelope through the engine crate"

  citations:
    - file: "demo/anchor-e2e/run.sh"
      note: ":96-131 the hand-written envelope with `\"signature\": null`; :133 the `curl -fsS` that discards the body"
    - file: "rigorix-enterprise/core/src/audit_ingestion"
      note: "ingest verifies the envelope HMAC; the error names the canonical-bytes scheme"

  verification:
    - "on a clean stack: `bash demo/anchor-e2e/run.sh` exits 0 and prints the four outcomes"
    - "seeding with a deliberately wrong key fails with INVALID_SIGNATURE and prints the body"
---

# ISSUE-ANCHOR-E2E-SEEDS-AN-UNSIGNED-ENVELOPE

## Measured

```
fresh postgres + migrations + seed.sql, then:
POST /api/v1/audit/oss-envelope  →  400
{"error":{"code":"INVALID_SIGNATURE", … "keyed by AUDIT_ENVELOPE_HMAC_SECRET"}}

with AUDIT_ENVELOPE_HMAC_SECRET=""  →  the same 400
```

`run.sh` stops at that point (`curl -fsS`, exit 22) and never reaches step 6, so the demo's four
outcomes are not being verified by anyone who runs it — the failure looks like an environment problem
and the body that explains it is thrown away.

## Why the fix is a signature, not a bypass

The envelope is the artifact the demo exists to exercise; ingest verifying it is the property the demo
demonstrates. The seed has to be signed with the same key, and the bytes have to come from the
engine's canonicaliser rather than a second implementation in the script — the canonical form is the
contract, and a copy of it in shell is a copy that drifts.
