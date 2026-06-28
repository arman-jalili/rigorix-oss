# Disaster Recovery Plan — Rigorix

## Recovery Objectives

| Metric | Target |
|--------|--------|
| RTO (Recovery Time Objective) | < 4 hours |
| RPO (Recovery Point Objective) | < 1 hour (last commit) |

## Backup Strategy

- **Source code**: GitHub (always up to date)
- **CI configuration**: In-repo `.github/workflows/`
- **Architecture documentation**: In-repo `.pi/architecture/`
- **Issue tracking**: GitHub Issues

## Restore Procedure

1. Clone repository: `git clone https://github.com/arman-jalili/rigorix-oss`
2. Verify commit history integrity: `git fsck`
3. Run full CI: `bash .pi/scripts/local-ci.sh`
4. Verify all per-crate checks: `cd engine && cargo test`

## Failover

- **CI/CD**: GitHub Actions (no alternate runner configured)
- **Build server**: Local `cargo build` as fallback
- **Issue tracker**: GitHub Issues (no alternate)

## Recovery Testing

- Run `bash .pi/scripts/local-ci.sh` before each release
- Verify `cargo audit` passes
- Check `docs/` are in sync via `validate-architecture.sh`
