# CI Integration Runbook

## Overview

The CI Integration module provides GitHub-specific CI/CD primitives that bridge
engine execution with GitHub's commit status checks and PR review comments. It is
called after the engine completes execution — every execution that runs in a PR
context goes through this module.

## Components

| Component | File | Purpose |
|-----------|------|---------|
| `StatusCheckServiceImpl` | `application/status_check_impl.rs` | Creates/updates GitHub commit status checks |
| `StatusCheckFactoryImpl` | `application/status_check_factory_impl.rs` | Builds `GitHubStatus` payloads with context naming |
| `StatusCheckRepositoryImpl` | `infrastructure/repository/status_check_repository_impl.rs` | GitHub API communication for status checks |
| `PrCommentServiceImpl` | `application/pr_comment_impl.rs` | Posts/updates PR execution summary comments |
| `PrCommentFactoryImpl` | `application/pr_comment_factory_impl.rs` | Builds `ExecutionSummary` and renders markdown |
| `PrCommentRepositoryImpl` | `infrastructure/repository/pr_comment_repository_impl.rs` | GitHub API communication for PR comments |

## Startup Sequence

1. **Module registration** — `ci_integration` module is registered in `actions/src/lib.rs`
2. **`StatusCheckServiceImpl::create_pending()`** — called when execution starts, sets commit status to "pending"
3. **Engine execution** — the rigorix engine runs the intent
4. **`StatusCheckServiceImpl::update_status()`** — called on completion, maps outcome to "success"/"failure"/"error"
5. **`PrCommentServiceImpl::upsert()`** — called to post/update the execution summary comment on the PR

## Dependencies

- **Shared `GitHubClient`** (`actions/src/shared/github_client.rs`) — all GitHub API calls
- **rigorix-engine** — `ValidationOutcome`, `ValidationReport` for status mapping
- **Contract freeze** — all service interfaces frozen in `service.rs`, `factory.rs`, `repository/mod.rs`

## Configuration

| Parameter | Source | Default | Description |
|-----------|--------|---------|-------------|
| `GITHUB_TOKEN` | Environment | required | GitHub API authentication token |
| `API base URL` | GitHubClient | `https://api.github.com` | GitHub API endpoint |
| Repository owner | Constructor | `rigorix` | GitHub org/owner name |
| Repository name | Constructor | `rigorix-oss` | GitHub repo name |
| Context prefix | Constructor | `rigorix` | Prefix for status check contexts |
| `GITHUB_REPOSITORY` | Environment | — | Used for repo detection |
| `GITHUB_RUN_ID` | Environment | — | Used for execution URL linking |

## Graceful Shutdown

The CI Integration module has no long-running processes. All operations are
single-shot HTTP calls to the GitHub API. If a shutdown is requested:

1. **In-flight status checks**: The current status check will complete or fail
   atomically — there is no partial state.
2. **In-flight PR comments**: If the comment is being posted, the API call
   will either complete or be dropped. The sticky comment pattern ensures
   no duplicate comments are created.
3. **Cleanup**: No persistent state to clean up.

## Common Failure Modes

### Scenario 1: GitHub API Rate Limit Exceeded

**Symptoms:**
- `CiIntegrationError::RateLimitExceeded { retry_after_secs }`
- Status checks not updating
- PR comments not posting

**Detection:**
- CI log shows `RateLimitExceeded` error
- GitHub API returns `429 Too Many Requests`

**Recovery:**
1. Wait the `retry_after_secs` duration
2. The caller should implement retry logic (exponential backoff with jitter)
3. If spamming occurs, review the execution rate limit settings

**Prevention:**
- The shared `GitHubClient` handles rate limit headers transparently
- Respects `X-RateLimit-Remaining` and `Retry-After` headers
- Implement per-PR execution tracking for idempotency

### Scenario 2: GitHub Token Expired or Invalid

**Symptoms:**
- `CiIntegrationError::PrCommentFailed` or `CiIntegrationError::GitHubApi`
- Status checks silently failing
- Comments not posting

**Detection:**
- CI log shows `GitHubClientError::AuthFailed`
- `403 Forbidden` or `401 Unauthorized` from GitHub API

**Recovery:**
1. Verify `GITHUB_TOKEN` is set in the environment
2. Check token has required scopes: `statuses:write`, `pull-requests:write`
3. Regenerate the token if expired
4. Re-run the execution

**Prevention:**
- Token should be injected from GitHub Actions `secrets.GITHUB_TOKEN`
- The `validate_token()` method on `GitHubClient` can be used for pre-flight checks

### Scenario 3: Status Check Context Collision

**Symptoms:**
- Status checks with wrong state
- Multiple status checks with same context

**Detection:**
- PR status section shows unexpected status
- Overwritten status contexts

**Recovery:**
1. Identify the conflicting status context
2. Re-run the execution with a unique context prefix
3. Or use the GitHib API to delete stale statuses manually

**Prevention:**
- Status check contexts use the format `rigorix/{suffix}` (e.g., `rigorix/execution`)
- The `StatusCheckFactoryImpl` ensures consistent context naming

### Scenario 4: PR Comment Spam

**Symptoms:**
- Multiple rigorix bot comments on a PR
- Duplicate execution summaries

**Detection:**
- Visual inspection of the PR comment section

**Recovery:**
1. Delete duplicate comments manually via GitHub UI
2. Re-run execution to trigger the sticky comment update

**Prevention:**
- The "sticky comment" pattern (find existing → update in-place) prevents duplicates
- Bot comments are identified by `<!-- rigorix-bot -->` marker

## Monitoring

### Key Metrics

| Metric | Source | Description |
|--------|--------|-------------|
| Status checks created | Application logs | Count of `create_pending()` calls |
| Status checks updated | Application logs | Count of `update_status()` calls |
| PR comments upserted | Application logs | Count of `upsert()` calls |
| GitHub API errors | Error logs | Count of API errors by type |
| Rate limit hits | Error logs | Count of rate limit exceeded errors |

### Alerting Rules

| Alert | Condition | Severity |
|-------|-----------|----------|
| High GitHub API error rate | >5 errors in 5 minutes | Warning |
| Rate limit hit | Any occurrence | Info |
| Token expiry | Validation failure | Critical |
| Status check not updating | Pending > 10 minutes | Warning |

## Troubleshooting

### Quick Diagnosis

```bash
# Check if GitHub API is accessible
curl -H "Authorization: Bearer $GITHUB_TOKEN" https://api.github.com/user

# Verify token scopes
curl -H "Authorization: Bearer $GITHUB_TOKEN" https://api.github.com/user

# Check commit statuses for a specific SHA
curl -H "Authorization: Bearer $GITHUB_TOKEN" \
  https://api.github.com/repos/owner/repo/commits/{sha}/statuses

# List issue comments
curl -H "Authorization: Bearer $GITHUB_TOKEN" \
  https://api.github.com/repos/owner/repo/issues/{number}/comments
```

### Debug Mode

Set `RUST_LOG=debug` to enable detailed logging for the ci_integration module:

```bash
RUST_LOG=debug rigorix-action
```
