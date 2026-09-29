# feat: #912 OSS-ANCHOR-WIRE-E2E — live anchored-path E2E + demo

Fixture parity is not wire parity. This adds the cross-process proof that the OSS
`HttpAnchorClient` talks to the **real enterprise anchor** over HTTP, plus a
CI-able in-process wire test.

## What changed

| Artifact | What it proves |
|----------|----------------|
| `engine/tests/anchored_wire_e2e.rs` | CI-able: real HTTP server returns a genuinely Ed25519-signed slice on the enterprise's exact contract; asserts valid / unreachable / erroring / forged / scope-mismatch / `local_unanchored` + envelope `anchor_head`/`history_integrity` |
| `engine/tests/anchored_wire_live.rs` | env-gated live E2E against the real enterprise (skips without `RIGORIX_E2E_ANCHOR_URL`) |
| `demo/anchor-e2e/run.sh` | one command: throwaway Postgres → enterprise → seed prior evidence → the four assertions → teardown (exit 0) |
| `demo/anchor-e2e/docker-compose.yml` + `seed.sql` + `README.md` | the small compose demo + wire contract doc |
| `.github/workflows/anchor-e2e.yml` | dispatched/scheduled cross-repo job (skips without `ENTERPRISE_REPO_TOKEN`) |

## Wire (pinned; enterprise `#215` + OSS client)

```
GET {anchor}/v1/history?scope=<producer_id>&since=<rfc3339>
Authorization: Bearer <rgx_... API key | JWT>
→ 200 { scope, since, actions[], head_hash, sig }   # bare signed HistorySlice
```

## Acceptance criteria

| # | Criterion | Evidence |
|---|-----------|----------|
| 1 | Wire contract pinned + documented | `demo/anchor-e2e/README.md`; enterprise `#215` |
| 2 | Real enterprise-signed slice verifies via `HttpAnchorClient` | `live_valid_slice_verifies_and_enforces` |
| 3 | Valid slice → deny-class run; envelope `anchor_head` + `history_integrity=anchored` | idem + `anchored_envelope_records_mode_and_verified_head` |
| 4 | Anchor unreachable → consequential run fails closed | `live_anchor_down_fails_closed` + `unreachable_anchor_fails_closed` |
| 5 | Forged slice → `verify_slice` rejects → fails closed | `live_forged_slice_fails_closed` + `forged_slice_fails_closed` |
| 6 | `system.version` reports mode + `anchor.{id,head}` | `server/tests/rpc_transport.rs`; `rt.status()` asserted |
| 7 | Host holds only the public key; `local_unanchored` unchanged; demo exit 0 + CI | `run.sh` exit 0; `local_unanchored_is_unchanged`; Integration stage |

## Validation

- **Live, against the real enterprise + throwaway Postgres** (`demo/anchor-e2e/run.sh`): all four outcomes pass.
- `cargo test -p rigorix-engine --test anchored_wire_e2e --test anchored_wire_live` ✅ (11 tests)
- `cargo clippy -p rigorix-engine --all-targets -- -D warnings` ✅
- `cargo fmt --all --check` ✅
- Normal CI: `🔗 Integration` (`cargo test --workspace`) runs the in-process wire E2E; the live test self-skips without env.

## Notes
- DEV/TEST ONLY: `seed.sql` embeds an Argon2id hash for the fixture key `rgx_dev_sk_e2e_secret_value` and the enterprise's dev anchor seed (`[7u8;32]`).
- The live workflow needs the enterprise repo (`ENTERPRISE_REPO_TOKEN`) and builds its debug binary; it is scheduled/dispatched, not per-PR.

Fixes #912
