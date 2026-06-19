# Disaster Recovery Plan: llm-step Module

<!--
Canonical Reference: .pi/architecture/modules/llm-step.md
Last Updated: 2026-06-19
-->

## Scope

This DR plan covers the `llm-step` module — the LLM-based code generation
system that wraps LLM calls within DAG execution. The module is stateless
at runtime (node state is in-memory) but may persist generated outputs and
node state for crash recovery and audit.

## RTO/RPO Targets

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | Module is stateless; nodes are created per execution and can be reconstructed from the execution graph |
| RPO (Recovery Point Objective) | < 1 second | Only the current generation context matters; lost generations can be retried |

## Backup Strategy

### What to Back Up

The llm-step module is primarily in-memory and stateless. If node persistence
is enabled (via `LlmGenerateNodeRepository`), the following should be backed up:

| Directory | Contents | Backup Priority |
|-----------|----------|-----------------|
| `llm_step/nodes/` | Serialized LlmGenerateNode files (`{uuid}.node.json`) | Medium — enables generation replay |
| `llm_step/generations/` | Generation output cache (`{hash}.output.json`) | Low — rebuildable |

### LLM Provider Configuration

The following configuration must be preserved for recovery:

| Item | Source | Criticality |
|------|--------|-------------|
| API keys | Environment variables / secrets manager | Critical |
| Provider endpoint URLs | Configuration file | High |
| Model configurations | Configuration file | High |

### Backup Schedule

| Type | Frequency | Retention | Method |
|------|-----------|-----------|--------|
| Configuration | On change | 30 days | Git history |
| Node state | Per execution | 7 days | Archive `.node.json` files |
| Generation cache | Per generation | 24 hours | Cache TTL expiry |

## Restore Procedure

### Scenario 1: Lost LLM Provider Connection

1. Verify API key is valid and not expired
2. Check provider endpoint URL is correct
3. Verify network connectivity to the provider
4. Restart the service with updated configuration

### Scenario 2: Corrupted Node State

1. Delete corrupted node file from storage
2. Re-create the node via `create_node` with the original configuration
3. Retry the generation from the execution engine

### Scenario 3: Full System Recovery

```bash
#!/bin/bash
# Recovery script for llm-step module

echo "=== llm-step Module Recovery ==="

# Step 1: Verify configuration
echo "[1/4] Verifying configuration..."
if [ -z "$LLM_API_KEY" ]; then
    echo "ERROR: LLM_API_KEY not set"
    exit 1
fi

# Step 2: Verify network connectivity to LLM provider
echo "[2/4] Checking LLM provider connectivity..."
if command -v curl &>/dev/null; then
    curl -s -o /dev/null -w "%{http_code}" \
        -H "x-api-key: $LLM_API_KEY" \
        "https://api.anthropic.com/v1/messages" \
        || echo "WARNING: Provider health check failed (may be expected)"
fi

# Step 3: Clean up stale node state
echo "[3/4] Cleaning stale state..."
if [ -d "$LLM_STEP_STORAGE_DIR" ]; then
    find "$LLM_STEP_STORAGE_DIR" -name "*.node.json" -mtime +7 -delete
    echo "  Cleaned node files older than 7 days"
fi

# Step 4: Verify service health
echo "[4/4] Service health check..."
# Health endpoint will verify provider client is functional

echo "=== Recovery Complete ==="
```

## Failover Plan

### Provider Failover

The module supports multiple LLM providers. In case of provider outage:

1. **Anthropic → OpenAI**: Reconfigure `default_provider.provider_name` to `"openai"`
   and update the `api_key` accordingly
2. **OpenAI → Anthropic**: Reconfigure `default_provider.provider_name` to `"anthropic"`
   and update the `api_key` accordingly
3. **Any → Mock**: For testing/dry-run, set provider to `"mock"` to return
   canned responses without external API calls

### Failover Procedure

```rust
// Example: failover from Anthropic to OpenAI
let new_config = LlmStepFactoryConfig {
    default_provider: LlmProviderConfig {
        provider_name: "openai".to_string(),
        default_model: "gpt-4o".to_string(),
        api_url: "https://api.openai.com/v1/chat/completions".to_string(),
        max_tokens: 4096,
        temperature: 0.7,
    },
    ..LlmStepFactoryConfig::default()
};
```

## RTO/RPO Strategy

### Meeting RTO (< 1 minute)

- All services are stateless — instantiate on demand
- No database migrations or schema changes required
- Provider clients are lightweight HTTP clients

### Meeting RPO (< 1 second)

- Node state is ephemeral per execution
- No persistent state that cannot be reconstructed
- Generation outputs are returned synchronously to the execution engine

## Testing the DR Plan

| Test | Frequency | Success Criteria |
|------|-----------|-----------------|
| Provider failover | Monthly | Switch between providers without data loss |
| Configuration restore | Monthly | Recover from config file backup |
| Node reconstruction | Quarterly | Re-create lost nodes from execution context |

## Recovery Scenarios

### Scenario A: Provider API Returns 429 (Rate Limited)

1. Automatically retry with exponential backoff
2. If rate limit persists, switch to alternative provider
3. If both providers rate-limited, queue and retry later

### Scenario B: Provider API Returns 5xx (Server Error)

1. Immediately retry up to `max_retries` (default 3)
2. If persistent, fail over to alternative provider
3. Log the failure for provider incident tracking

### Scenario C: Generation Produces Invalid Output

1. Re-execute the generation with updated failure context
2. The `assemble_prompt` method appends error details to the prompt
3. Retry up to `max_retries` with progressively augmented context
