# OSS Side — Enterprise Integration Plan

> Enterprise config, policy bundle fetch + verify + merge, Action inputs, and audit posting for the rigorix-oss CLI + GitHub Action.

## Context

The OSS GitHub Action is the **trusted enforcement point** — it runs in GitHub infrastructure where employees cannot tamper with enforcement. The CLI is optional best-effort preview only. Branch protection gates on the Action's status check.

```
┌─ Enterprise ────────────────────────┐
│  GET /api/v1/policies/bundle        │
│  POST /api/v1/audit/cli             │
│  POST /api/v1/audit/github-pr       │
└──────────┬──────────────────────────┘
           │
    ┌──────┴─────────────────────────────────┐
    │         OSS GitHub Action               │
    │  1. Fetch policy bundle (GET bundle)    │
    │  2. Verify HMAC signature               │
    │  3. Merge → EnforcementConfig           │
    │  4. Execute DAG with merged config      │
    │  5. POST audit records + violations     │
    └────────────────────────────────────────┘
```

## Task 1: Enterprise Config Section (Engine)

**Files to modify:**
- `engine/src/configuration/domain/config.rs` — add `enterprise: Option<EnterpriseConfig>` field to `Config`

**Files to create:**
- `engine/src/enterprise/domain/config.rs` — `EnterpriseConfig` struct
- `engine/src/enterprise/domain/mod.rs`
- `engine/src/enterprise/mod.rs`

```rust
pub struct EnterpriseConfig {
    pub api_url: String,              // https://rigorix.example.com/api/v1
    pub api_key: String,              // team API key (secret)
    pub team_id: uuid::Uuid,
    pub fetch_policies: bool,         // default: true
    pub enforce_policies: bool,       // default: true
    pub post_audit: bool,             // default: true
    pub policy_cache_ttl_secs: u64,   // default: 300
}
```

**Env var mapping:**
- `RIGORIX__ENTERPRISE__API_URL`
- `RIGORIX__ENTERPRISE__API_KEY`
- `RIGORIX__ENTERPRISE__TEAM_ID`

**Config file (`rigorix.toml`):**
```toml
[enterprise]
api_url = "https://rigorix.example.com/api/v1"
api_key = "rlx_live_..."
team_id = "00000000-0000-0000-0000-000000000001"
fetch_policies = true
enforce_policies = true
post_audit = true
```

## Task 2: Policy Bundle Types + HTTP Client (Engine)

**Files to create:**
- `engine/src/enterprise/domain/bundle.rs` — `PolicyBundle`, `PolicyBundleEntry` structs
- `engine/src/enterprise/domain/error.rs` — `EnterpriseError` enum
- `engine/src/enterprise/infrastructure/http_client.rs` — `HttpEnterpriseClient`

**`PolicyBundle`:**
```rust
pub struct PolicyBundle {
    pub team_id: uuid::Uuid,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub policies: Vec<PolicyBundleEntry>,
    pub signature: String,  // "sha256=<hex>"
}

pub struct PolicyBundleEntry {
    pub id: uuid::Uuid,
    pub name: String,
    pub rule_type: String,
    pub rule_config: serde_json::Value,
    pub enforcement_mode: String,
    pub severity: String,
    pub enabled: bool,
}
```

**`HttpEnterpriseClient`:** wraps `reqwest::Client` with the enterprise API key.

## Task 3: Policy Bundle Fetch + Verify + Merge (Engine)

**Files to create:**
- `engine/src/enterprise/application/service.rs` — `EnterpriseService` trait
- `engine/src/enterprise/application/dto/mod.rs` — `FetchBundleInput`, `FetchBundleOutput`, `MergePoliciesInput`, `MergePoliciesOutput`
- `engine/src/enterprise/application/service_impl.rs` — `EnterpriseServiceImpl`

**Flow:**

1. **Fetch:** GET `{api_url}/policies/bundle?team_id={team_id}` with `Authorization: Bearer {api_key}`
2. **Verify:** HMAC-SHA256 over `team_id + generated_at + canonical JSON of policies[]` using the API key secret as HMAC key. Constant-time comparison.
3. **Map:** Enterprise policies → `EnforcementConfig` overrides
4. **Merge:** Deep-merge with local `EnforcementConfig`. Enterprise always wins.
5. **Cache:** Hold in memory with TTL (default 5 min), re-fetch on expiry.

**Signature verification:**
```rust
fn verify_bundle_signature(bundle: &PolicyBundle, api_key: &str) -> Result<(), EnterpriseError> {
    let payload = format!(
        "{}{}{}",
        bundle.team_id,
        bundle.generated_at.to_rfc3339(),
        canonical_json(&bundle.policies)
    );
    let expected = hmac_sha256(api_key.as_bytes(), payload.as_bytes());
    let expected_hex = hex::encode(expected);
    // constant-time compare with bundle.signature (strip "sha256=" prefix)
}
```

**Policy merge rules:**

| Scenario | Result |
|---|---|
| Enterprise `block` + local `allowed: true` | Enterprise wins (block) |
| Enterprise `warn` + local any | Enterprise overrides with warn |
| Enterprise `monitor` | Local setting used, violation reported |
| Enterprise `tool_blocklist` | Tools added to `tool_policies` with `allowed: false` |
| Enterprise `llm_budget` | Caps local budget at enterprise limit |
| Enterprise `risk_threshold` | Can only **raise** minimum risk level, never lower |

## Task 4: Action Inputs + Wiring

**Files to modify:**
- `actions/action.yml` — add `enterprise-api-key`, `enterprise-api-url`, `enterprise-team-id` inputs
- `actions/src/action_entrypoint/domain/types.rs` — add `enterprise_config: Option<EnterpriseActionConfig>` to `ActionContext`
- `actions/src/action_entrypoint/infrastructure/env_context_provider.rs` — read `INPUT_ENTERPRISE_API_KEY`, `INPUT_ENTERPRISE_API_URL`, `INPUT_ENTERPRISE_TEAM_ID`
- `actions/src/main.rs` — wire enterprise service into action dispatch

**New Action inputs:**
```yaml
enterprise-api-key:
  description: 'API key for Rigorix Enterprise'
  required: false
enterprise-api-url:
  description: 'Rigorix Enterprise API URL'
  required: false
enterprise-team-id:
  description: 'Team UUID in Rigorix Enterprise'
  required: false
fail-on-violation:
  description: 'Fail workflow when enterprise policies are violated'
  required: false
  default: 'true'
```

**Action flow update** (in `main.rs`):
```
1. Parse INPUT_* env vars (existing)
2. If enterprise inputs are set:
   a. Build EnterpriseConfig from inputs
   b. Fetch policy bundle before execution
   c. Verify bundle signature
   d. Merge enterprise policies into EnforcementConfig
3. Execute DAG with merged config
4. If enterprise configured:
   a. POST audit records to enterprise (via HttpAuditBackend)
   b. POST violations to enterprise
5. If enterprise policies violated + fail-on-violation: exit 1
6. Post step summary with violations if any
```

## Task 5: Audit Backend — Auth Header

**Files to modify:**
- `actions/src/audit_posting/infrastructure/http_audit_backend.rs`

`HttpAuditBackend` currently POSTs with no `Authorization` header. Changes:

1. Add optional `api_key: Option<String>` field to `HttpAuditBackend`
2. Update constructor to accept `api_key`
3. In `post()`: add `Authorization: Bearer {api_key}` header when `api_key` is `Some`
4. Wire enterprise API key from `ActionContext` into `HttpAuditBackend` at construction time

## Verification

```bash
# 1. Unit test: HttpEnterpriseClient::fetch_policy_bundle
# Mock HTTP 200 + valid signature → Ok(policies)
# Mock HTTP 200 + invalid signature → Err(EnterpriseError::SignatureMismatch)
# Mock HTTP 401 → Err(EnterpriseError::Unauthorized)
# Mock HTTP 500 → Err(EnterpriseError::ServerError)

# 2. Unit test: policy merge
# Input: local EnforcementConfig + bundle with tool_blocklist ["bash", "delete_file"]
# Expected: blocked tools have allowed=false, unblocked tools unchanged

# 3. Unit test: merge precedence
# Input: local preset=Standard, enterprise risk_threshold=high
# Expected: merged config uses higher risk level

# 4. Integration test (requires enterprise server running)
# - Create API key, policies in enterprise
# - Run OSS action with enterprise inputs
# - Verify audit record appears in enterprise dashboard
# - Create policy blocking "bash" tool
# - Run action → verify it fails with violation

# 5. Config loading test
# RIGORIX__ENTERPRISE__API_URL=https://example.com → Config.enterprise.api_url == "https://example.com"
# rigorix.toml [enterprise] section → merged correctly with env vars (env wins)

# 6. Cache test
# Fetch bundle → cached
# Second fetch within TTL → uses cache (no HTTP request)
# Fetch after TTL → re-fetches from server
```
