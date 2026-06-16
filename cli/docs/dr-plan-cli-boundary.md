# Disaster Recovery Plan: CLI Boundary

> **RTO:** < 1 minute | **RPO:** N/A

## Overview

The CLI boundary is stateless — it parses commands, loads config, and dispatches to handlers. No mutable state beyond the process lifetime.

## Failure Scenarios

### CLI binary corrupted
**Recovery:** `cargo build` in cli/ directory

### Startup crashes
**Recovery:** Check config file syntax. Run with `--config /dev/null` to bypass file config.

## RTO/RPO

| Metric | Target | Notes |
|--------|--------|-------|
| RTO | < 1 minute | Just restart CLI |
| RPO | N/A | No mutable state |
