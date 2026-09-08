# Conformance fixtures (F-20260907)

| File | Source | Provenance |
|------|--------|-----------|
| `conference-demo-sequence-policy.toml` | `arman-jalili/conference-demo` (public) `.rigorix/sequence-policy.toml` | Pinned blob SHA `c3a51116c6926bafcc3e38f5396fca807ca4083a` (2026-09-08). Real operator-authored file incl. the R7 `history` cross-run rule. Re-fetch: `gh api repos/arman-jalili/conference-demo/contents/.rigorix/sequence-policy.toml --jq '.content' \| base64 -d` |

Real signed envelope fixtures (engine-produced, fixed test key) live in
rigorix-sdk `schemas/fixtures/envelope/` — see the SDK PR referenced by
F-20260907-01. This dir carries only inputs the OSS engine needs at test time.
