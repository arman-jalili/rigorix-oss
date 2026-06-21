# CI Integration Disaster Recovery Plan

## Scope

This DR plan covers the CI Integration module within the `rigorix-actions` crate.
The module is stateless — it makes HTTP calls to the GitHub REST API to create
commit status checks and post PR comments. There is no database, no persistent
storage, and no long-lived state beyond the shared `GitHubClient`.

## RTO / RPO

| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO (Recovery Time Objective) | < 1 minute | Module is stateless; rebuild is fast |
| RPO (Recovery Point Objective) | N/A | No persistent state to recover |

## Backup Strategy

Not applicable. The CI Integration module has no state to back up:
- No database tables
- No cache files
- No queue messages
- No user data

## Infrastructure Dependencies

| Dependency | Type | Failure Impact | Redundancy |
|------------|------|----------------|------------|
| GitHub API (api.github.com) | External API | Status checks and PR comments fail | GitHub has built-in HA |
| `GITHUB_TOKEN` | Secret | All API calls fail with auth errors | Token rotation, multiple sources |
| Network connectivity | Infrastructure | API calls fail with network errors | Retry with backoff |

## Failure Scenarios

### Scenario 1: GitHub API Outage

**Symptoms:**
- All status check and PR comment operations fail with `NetworkError` or `ApiError`
- CI logs show connection timeouts or 5xx responses

**Impact:**
- Status checks not created → PR merge checks fail
- PR execution summaries not posted → users don't see results
- **No data loss** — the module has no state to corrupt

**Immediate Response:**
1. Verify GitHub status at https://www.githubstatus.com/
2. If confirmed outage, no action needed — the module retries with exponential backoff
3. The `GitHubClient` handles retries transparently

**Recovery:**
1. Once GitHub restores service, operations resume automatically
2. Any failed executions can be retried via `/rigorix retry <execution_id>`
3. No manual intervention needed

**RTO Met:** ✅ Automatic — no operator action needed for recovery

### Scenario 2: GitHub Token Compromise

**Symptoms:**
- Unauthorized status checks or comments appearing on PRs
- Security alert from GitHub

**Impact:**
- An attacker could post misleading status checks or comments
- False execution summaries could be posted to PRs

**Immediate Response:**
1. Revoke the compromised token immediately via GitHub Settings
2. Generate a new token with minimal required scopes
3. Update the `GITHUB_TOKEN` secret
4. Audit recent CI comment activity for anomalies

**Recovery:**
1. Deploy the new token to the action environment
2. Verify token validity via `validate_token()` method
3. Re-run any affected executions

**Prevention:**
- Token is stored in GitHub Actions secrets, never in code
- Token is never logged (Debug impl masks it)
- Token scopes are minimal: `statuses:write`, `pull-requests:write`
- The `GitHubClient` includes a `from_env()` constructor for secure token loading

### Scenario 3: Repository Configuration Change

**Symptoms:**
- Actions failing because repository name or owner changed
- Status checks not appearing on commits after repo rename

**Impact:**
- Status checks fail to create
- PR comments fail to post
- All CI integration operations fail

**Recovery:**
1. Update the `owner` and `repo` parameters in the `StatusCheckServiceImpl` and `PrCommentServiceImpl` constructors
2. Verify the new repository name is correct
3. Re-run the action

**Prevention:**
- Constructor injection of `owner` and `repo` — no hardcoded values in business logic
- The `GITHUB_REPOSITORY` env var can be parsed for automatic detection

### Scenario 4: Partial Module Failure (Status Checks Working, Comments Failing)

**Symptoms:**
- Status checks appear correctly on commits
- PR execution summary comments are not posted

**Impact:**
- Users don't see execution results in PR comments
- Status checks still provide basic pass/fail information

**Recovery:**
1. Check if the token has `pull-requests:write` scope
2. Verify GitHub API is not rate-limited for comment endpoints
3. Check if the bot comment was accidentally deleted by a user
4. The sticky comment pattern will create a new comment if the existing one is gone
5. Re-run the execution

## Failover Plan

The CI Integration module does not support active-active or active-passive failover
because it is tied to a single GitHub instance. However:

1. **Read-only fallback**: If the GitHub API is unavailable, the execution continues
   without status checks or comments. The execution output is still written to
   `GITHUB_OUTPUT` and `GITHUB_STEP_SUMMARY`.
2. **Retry mechanism**: The `GitHubClient` implements exponential backoff with jitter
   for transient failures.
3. **Graceful degradation**: `create_pending()` failure does not block execution;
   the engine still runs. `update_status()` failure after execution is logged but
   does not affect the execution result.

## Testing the DR Plan

| Test | Frequency | Procedure |
|------|-----------|-----------|
| Token rotation | Quarterly | Generate new token, verify operations |
| API availability | Continuous | Monitor GitHub status page |
| Retry mechanism | Per build | Verify error handling in CI logs |
| Sticky comment recovery | Manual | Delete bot comment, re-run, verify new comment created |

## Recovery Script

```bash
#!/usr/bin/env bash
# CI Integration Recovery Script
# Run when the module is not functioning correctly

set -euo pipefail

echo "=== CI Integration Recovery ==="

# 1. Verify token
echo "Checking GITHUB_TOKEN..."
if [[ -z "${GITHUB_TOKEN:-}" ]]; then
    echo "❌ GITHUB_TOKEN is not set"
    echo "   Set it via: export GITHUB_TOKEN=ghp_..."
    exit 1
fi
echo "✅ GITHUB_TOKEN is set"

# 2. Test GitHub API connectivity
echo "Testing GitHub API..."
if curl -s -o /dev/null -w "%{http_code}" \
    -H "Authorization: Bearer $GITHUB_TOKEN" \
    https://api.github.com/user | grep -q "200"; then
    echo "✅ GitHub API is accessible"
else
    echo "❌ GitHub API is not accessible"
    echo "   Check network connectivity and token validity"
    exit 1
fi

# 3. Verify token scopes
echo "Verifying token scopes..."
SCOPES=$(curl -s -I -H "Authorization: Bearer $GITHUB_TOKEN" \
    https://api.github.com/user 2>&1 | grep -i "x-oauth-scopes" || true)
echo "   Scopes: ${SCOPES:-unknown}"

# 4. Test create_status directly
if [[ -n "${TEST_REPO:-}" && -n "${TEST_SHA:-}" ]]; then
    echo "Testing create_status..."
    curl -s -X POST \
        -H "Authorization: Bearer $GITHUB_TOKEN" \
        -H "Content-Type: application/json" \
        -d '{"state":"success","description":"DR test","context":"rigorix/dr-test"}' \
        "https://api.github.com/repos/${TEST_REPO}/statuses/${TEST_SHA}"
    echo ""
    echo "✅ Status check test completed"
fi

echo ""
echo "=== Recovery Complete ==="
```

## DR Contact Information

| Role | Contact | Escalation |
|------|---------|------------|
| Primary on-call | DevOps team | PagerDuty |
| Engineering lead | Engineering manager | Slack #rigorix-eng |
| GitHub support | https://support.github.com/ | Enterprise support ticket |

## DR Document History

| Date | Author | Change |
|------|--------|--------|
| 2026-06-21 | Rigorix Bot | Initial DR plan for ci-integration module |
