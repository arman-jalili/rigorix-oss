# Security Config Runbook

## Overview
The Security Configuration module enforces operational security for the GitHub Action.
It validates the execution environment before any operation begins.

## Components

| Component | File | Purpose |
|-----------|------|---------|
| ForkDetectorImpl | `application/fork_detector_impl.rs` | Detects fork PRs |
| SecretMaskerImpl | `application/secret_masker_impl.rs` | Masks secrets from logs |
| TokenValidatorImpl | `application/token_validator_impl.rs` | Validates GitHub token permissions |
| UrlAllowlistImpl | `application/url_allowlist_impl.rs` | Validates backend URLs against allowlist |
| HmacSignerImpl | `application/hmac_signer_impl.rs` | HMAC-SHA256 signing and verification |

## Startup Sequence
1. SecurityValidator::validate() — runs all checks in order
2. Fork detection → secret masking → token check → URL check → level resolution

## Dependencies
- GitHub API (token validation)
- hmac + sha2 (HMAC signing)
- reqwest (HTTP calls)

## Failure Modes
| Error | Cause | Recovery |
|-------|-------|----------|
| ForkDetected | PR from fork | Check fork PR workflow rules |
| TokenInsufficient | Missing permissions | Update workflow permissions |
| UrlBlocked | URL not in allowlist | Update security.toml |
| HmacKeyMissing | Key env var not set | Set RIGORIX_HMAC_KEY |
