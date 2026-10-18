# Batch 4 — Issues #1053 + #1054

**Branch:** `feature/capacity-guarantee-accuracy`
**Theme:** make the documented guarantee match the implemented one.

## #1053 — single-process, not single-execution
- Docs: `InProcessLedger` provides **single-process atomicity** — every execution
  in the process that shares the instance contends on one aggregate. Cross-session
  (cross-process/replica) needs `HttpLedger` + the enterprise service.
- Test: `two_concurrent_executions_share_the_in_process_aggregate` runs two
  `execute_graph` calls through one executor and asserts one refusal, one marker,
  and `committed + outstanding <= hard_limit`.

## #1054 — Drop is best-effort, not a guarantee
- `Drop` failure is logged (`tracing::warn!`) instead of `let _ =`.
- Docs: Drop is local cleanup; explicit awaited `commit`/`release` is the
  correctness path; durable lease + reconciliation (now `Ambiguous`) is recovery.
- RAII is a safeguard against forgotten cleanup paths, not durable recovery.

## Validation
ci (incl. e2e), architecture, canonical, docs, panic surface.
