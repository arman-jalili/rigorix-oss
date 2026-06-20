# Security Config Disaster Recovery Plan

## Scope
The Security Configuration module is stateless — no database, no cache, no external service dependencies beyond the GitHub API.

## RTO / RPO
| Metric | Target | Rationale |
|--------|--------|-----------|
| RTO | < 1 minute | Stateless module; rebuild is fast |
| RPO | N/A | No persistent state |

## Failure Scenarios

### ForkDetector Failures
- **Symptom:** All PRs detected as forks or none detected
- **Cause:** Missing/corrupt `GITHUB_REPOSITORY` env var
- **Recovery:** Verify GitHub Actions env vars are correctly set

### SecretMasker Failures
- **Symptom:** Secrets appear in workflow logs
- **Cause:** Masking called after logging, or not called at all
- **Recovery:** Ensure `mask()` is called before any `tracing::info!` or `println!`

### TokenValidator Failures
- **Symptom:** Token validation errors or false negatives
- **Cause:** GitHub API rate limiting or network issues
- **Recovery:** Retry with exponential backoff; verify `GITHUB_TOKEN` permissions

### UrlAllowlist Failures
- **Symptom:** All URLs blocked or all allowed
- **Cause:** Missing or corrupted `.rigorix/security.toml`
- **Recovery:** Check security.toml syntax and allowed_hosts entries

### HmacSigner Failures
- **Symptom:** Signature verification failures
- **Cause:** Missing RIGORIX_HMAC_KEY or key rotation mismatch
- **Recovery:** Verify key env var is set and matches between signer/verifier

## Testing
```bash
# All tests
cargo test --lib -p rigorix-actions

# Proofing
bash actions/.pi/scripts/ci/stage_security-config_proofing.sh
```
