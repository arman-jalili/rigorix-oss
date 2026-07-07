# Enterprise Integration Architecture

<!--
Canonical Reference: .pi/architecture/modules/enterprise.md
Rationale: OSS action connects to Rigorix Enterprise for policy enforcement and audit posting — extends EnforcementConfig with server-managed rules that always win in conflicts
-->

## Overview

The Enterprise module connects the Rigorix OSS engine to Rigorix Enterprise, enabling server-managed policy enforcement. It fetches signed policy bundles from the enterprise API, verifies their HMAC-SHA256 integrity, and merges them into the local `EnforcementConfig` using a defined conflict-resolution strategy where enterprise rules always take precedence.

This bridges OSS-local enforcement with enterprise-wide governance: organizations define policy centrally (blocked tools, budget caps, risk thresholds) and the OSS engine pulls them down on a configurable cache TTL.

## Adoption Rationale

Rigorix OSS enforces policy via `EnforcementConfig` loaded from local configuration. The Enterprise module extends this with an external policy source:

- **Centralized governance**: policies defined once in enterprise, applied everywhere
- **Signed delivery**: HMAC-SHA256 signature prevents tampering during transit
- **Cache-friendly**: in-memory cache with TTL avoids redundant fetches while staying fresh
- **Conflict resolution**: enterprise always wins — `block` overrides local, `warn` enforces confirmation, `risk_threshold` only raises
- **Audit trail**: enterprise policy state is deterministic and testable
- **Decoupled**: no changes to `EnforcementConfig` — merge produces a new config instance

## Responsibilities

- Fetch signed policy bundles from the enterprise API (`GET /policies/bundle?team_id=...`)
- Verify bundle integrity via HMAC-SHA256 signature (constant-time comparison)
- Cache fetched bundles in-memory with configurable TTL (default 300s)
- Merge enterprise policies into local `EnforcementConfig` — enterprise always wins
- Support rule types: `tool_blocklist`, `llm_budget`, `risk_threshold`, `block`, `warn`, `monitor`
- Skip disabled policies, ignore unknown rule types

## Components

| Component | File Path | Purpose | Canonical Section |
|-----------|-----------|---------|-------------------|
| EnterpriseConfig | `engine/src/enterprise/domain/config.rs` | Domain entity: api_url, api_key, team_id, feature flags, cache TTL | #config |
| PolicyBundle | `engine/src/enterprise/domain/bundle.rs` | Signed bundle from enterprise: team_id, generated_at, policies, signature | #bundle |
| PolicyBundleEntry | `engine/src/enterprise/domain/bundle.rs` | Single policy: id, name, rule_type, rule_config (JSON), enforcement_mode, severity, enabled | #bundle |
| EnterpriseError | `engine/src/enterprise/domain/error.rs` | Error enum: Unauthorized, ServerError, SignatureMismatch, RequestFailed, ConfigError, CacheError | #error |
| HttpEnterpriseClient | `engine/src/enterprise/infrastructure/http_client.rs` | HTTP client: fetch_bundle() with Bearer auth | #http-client |
| EnterpriseService | `engine/src/enterprise/application/service.rs` | Trait: fetch_policy_bundle(), merge_policies(), get_enforcement_config() | #service |
| EnterpriseServiceImpl | `engine/src/enterprise/application/service_impl.rs` | Implementation: cache + verify + merge + full flow orchestration | #service-impl |
| FetchBundleInput/Output | `engine/src/enterprise/application/dto/mod.rs` | DTOs for fetch_policy_bundle() | #dto |
| MergePoliciesInput/Output | `engine/src/enterprise/application/dto/mod.rs` | DTOs for merge_policies() | #dto |

---

## Component Details

### EnterpriseConfig

**Purpose:** Configuration for connecting to Rigorix Enterprise

```rust
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct EnterpriseConfig {
    pub api_url: String,
    pub api_key: String,
    pub team_id: uuid::Uuid,
    pub fetch_policies: bool,       // default: true
    pub enforce_policies: bool,     // default: true
    pub post_audit: bool,           // default: true
    pub policy_cache_ttl_secs: u64, // default: 300 (5 minutes)
}
```

- Loaded from `rigorix.toml` `[enterprise]` section or environment variables
- Injected into `ActionContext` in the GitHub Action via `enterprise-*` inputs
- Feature flags allow granular control (e.g. fetch but don't enforce)

### PolicyBundle

**Purpose:** Signed policy data from the enterprise API

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyBundle {
    pub team_id: uuid::Uuid,
    pub generated_at: chrono::DateTime<chrono::Utc>,
    pub policies: Vec<PolicyBundleEntry>,
    pub signature: String, // "sha256=<hex>" format
}
```

**PolicyBundleEntry:**

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolicyBundleEntry {
    pub id: uuid::Uuid,
    pub name: String,
    pub rule_type: String,      // "block" | "warn" | "monitor" | "tool_blocklist" | "llm_budget" | "risk_threshold"
    pub rule_config: serde_json::Value, // Rule-specific JSON config
    pub enforcement_mode: String,
    pub severity: String,
    pub enabled: bool,
}
```

### HttpEnterpriseClient

**Purpose:** HTTP client for the enterprise API

```rust
pub struct HttpEnterpriseClient {
    client: reqwest::Client,
}

impl HttpEnterpriseClient {
    pub fn new() -> Self;
    pub fn with_timeout(timeout_secs: u64) -> Self;

    pub async fn fetch_bundle(
        &self,
        config: &EnterpriseConfig,
    ) -> Result<PolicyBundle, EnterpriseError>;
}
```

- Sends `GET {api_url}/policies/bundle?team_id={team_id}` with `Authorization: Bearer {api_key}`
- Default timeout: 30 seconds
- Maps HTTP 401/403 → `EnterpriseError::Unauthorized`
- Maps HTTP 5xx → `EnterpriseError::ServerError`
- Network errors (timeout, connect) → `EnterpriseError::RequestFailed`

### EnterpriseService

**Purpose:** Orchestrates policy fetch, verify, and merge

```rust
#[async_trait]
pub trait EnterpriseService: Send + Sync {
    async fn fetch_policy_bundle(
        &self,
        config: &EnterpriseConfig,
    ) -> Result<FetchBundleOutput, EnterpriseError>;

    async fn merge_policies(
        &self,
        bundle: &PolicyBundle,
        local_config: &EnforcementConfig,
    ) -> Result<MergePoliciesOutput, EnterpriseError>;

    async fn get_enforcement_config(
        &self,
        enterprise_config: &EnterpriseConfig,
        local_config: &EnforcementConfig,
    ) -> Result<EnforcementConfig, EnterpriseError>;
}
```

### EnterpriseServiceImpl

**Purpose:** Full implementation with cache, signature verification, and policy merging

**Signature Verification:**

```rust
fn verify_bundle_signature(bundle: &PolicyBundle, api_key: &str) -> Result<(), EnterpriseError> {
    let canonical = canonical_json(&bundle.policies);
    let payload = format!("{}{}{}", bundle.team_id, bundle.generated_at.to_rfc3339(), canonical);
    let mac = Hmac::<Sha256>::new_from_slice(api_key.as_bytes())?;
    mac.update(payload.as_bytes());
    let expected = hex::encode(mac.finalize().into_bytes());
    let received = bundle.signature.strip_prefix("sha256=").unwrap_or(&bundle.signature);
    let (received_bytes, expected_bytes) = (hex::decode(received)?, hex::decode(expected)?);
    if constant_time_eq(&received_bytes, &expected_bytes) { Ok(()) }
    else { Err(EnterpriseError::SignatureMismatch { ... }) }
}
```

**Constant-time comparison** — uses manual XOR to prevent timing side channels:

```rust
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() { return false; }
    let mut result: u8 = 0;
    for (x, y) in a.iter().zip(b.iter()) { result |= x ^ y; }
    result == 0
}
```

**Policy Merge Rules:**

| Enterprise Rule Type | Behavior | Priority |
|---------------------|----------|----------|
| `tool_blocklist` | Sets `allowed: false` on listed tools | Overrides local |
| `block { tool }` | Sets specific tool to `allowed: false` | Overrides local |
| `warn { tool }` | Sets `allowed: true`, `requires_confirmation: true` | Overrides local |
| `monitor { tool }` | No config changes — just reporting | Keeps local |
| `llm_budget { max_tokens }` | Caps `budgets.tokens.hard_limit` at enterprise value | Lowers only |
| `risk_threshold { min_risk_level }` | Raises tool risk levels below threshold | Raises only |

---

## Data Flow

```
ActionContext with EnterpriseConfig
        │
        ▼
EnterpriseServiceImpl::get_enforcement_config()
        │
        ├── Cache hit & fresh?
        │   └── Yes → return cached bundle
        │
        ├── Fetch from API (HttpEnterpriseClient::fetch_bundle)
        │   └── GET /policies/bundle?team_id=...
        │       Authorization: Bearer {api_key}
        │
        ├── Verify HMAC-SHA256 signature
        │   ├── Match → continue
        │   └── Mismatch → return SignatureMismatch error
        │
        ├── Update in-memory cache
        │
        └── EnterpriseServiceImpl::merge_policies()
            │
            For each enabled policy in bundle:
            ├── tool_blocklist → set allowed: false
            ├── block → set allowed: false (single tool)
            ├── warn → set requires_confirmation: true
            ├── monitor → no-op (local setting preserved)
            ├── llm_budget → cap tokens hard_limit
            ├── risk_threshold → raise tool risk levels
            └── unknown → skip with debug log
                │
                ▼
            Merged EnforcementConfig (enterprise always wins)
                │
                ▼
            Passed to Orchestrator for execution
```

---

## Dependencies

### Depends On
- **Enforcement**: `EnforcementConfig`, `ToolPolicy`, `ToolRiskLevel` — the merge targets
- **HTTP Client**: `reqwest` — for API calls to enterprise server
- **HMAC**: `hmac` + `sha2` — for signature verification
- **Configuration**: `EnterpriseConfig` loaded via config pipeline (TOML + env)
- **Action Entrypoint**: `EnterpriseActionConfig` → `EnterpriseConfig` mapping in GitHub Action

### Used By
- **Orchestrator**: Calls `get_enforcement_config()` before execution to resolve the final config
- **Audit Posting**: `HttpAuditBackend` uses enterprise API key for auth header
- **Configuration**: `ConfigDto.enterprise` section loads enterprise connection settings

---

## Configuration

```toml
# rigorix.toml
[enterprise]
api_url = "https://rigorix.example.com/api/v1"
api_key = "${ENTERPRISE_API_KEY}"  # resolved from env
team_id = "00000000-0000-0000-0000-000000000001"
fetch_policies = true
enforce_policies = true
post_audit = true
policy_cache_ttl_secs = 300
```

Or via environment variables:
```bash
RIGORIX__ENTERPRISE__API_URL=https://rigorix.example.com/api/v1
RIGORIX__ENTERPRISE__API_KEY=sk-...
RIGORIX__ENTERPRISE__TEAM_ID=00000000-0000-0000-0000-000000000001
```

Or via GitHub Action inputs:
```yaml
- uses: rigorix/action@v1
  with:
    enterprise-api-key: ${{ secrets.ENTERPRISE_API_KEY }}
    enterprise-api-url: https://rigorix.example.com/api/v1
    enterprise-team-id: "00000000-0000-0000-0000-000000000001"
```

---

## Testing Requirements

| Test Type | Coverage Target | Files |
|-----------|-----------------|-------|
| Unit | 95% | `engine/src/enterprise/` — per-component test modules |

**Key Test Scenarios:**
- `constant_time_eq` — equal slices, different lengths, different content
- `verify_bundle_signature` — valid signature, invalid signature, invalid hex in bundle, invalid hex in computed
- `tool_blocklist` merge — blocks listed tools, leaves others unchanged
- `block` rule — blocks a single tool, preserves other fields
- `warn` rule — sets requires_confirmation on the tool
- `monitor` rule — no changes to config
- `llm_budget` — caps tokens hard_limit, doesn't raise
- `risk_threshold` — raises risk from Low to High, doesn't lower
- Disabled policies — skipped during merge
- Unknown rule type — skipped with no errors
- Empty bundle — returns local config unchanged
- Cache hit (fresh) — returns cached bundle without HTTP call
- Cache miss (expired TTL or empty) — fetches from API
- `fetch_bundle` — success, unauthorized, server error, JSON parse error
- `get_enforcement_config` — full end-to-end: fetch → verify → merge

---

**Status:** Implemented
**Implementation priority:** P1 — enterprise policy enforcement
