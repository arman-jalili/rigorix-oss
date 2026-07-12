# Validation Checklists

> **Purpose:** Machine-parseable checklists for all validators. Used for caching — passed items are skipped on retry.
> **Generic:** Replace check items with your project's validation rules.

---

## Architecture Validation

### Pre-Implementation (Plan Review)
- [ ] Design follows existing module organization
- [ ] Error handling approach defined
- [ ] No duplicate types (grep verified)
- [ ] Dependencies identified and available
- [ ] Module structure planned

### Post-Implementation (Code Review)
- [ ] Wiring: callers exist (not dead code)
- [ ] No duplicate type definitions
- [ ] Module declared AND imported
- [ ] Tools/components registered in registry
- [ ] Errors integrated into parent error type
- [ ] E2E tests pass
- [ ] Architecture contract tests pass

## Security Validation

### All Code Changes
- [ ] No hardcoded secrets
- [ ] No command injection (no `Command::new` + `format!`)
- [ ] Path traversal prevention (canonicalize + starts_with)
- [ ] Input validation on all external inputs
- [ ] Secrets not logged
- [ ] Risk levels properly assigned (Safe/Medium/Dangerous)

## Operations Validation

### Performance
- [ ] No O(N²) where O(N) expected
- [ ] Proper data structures used
- [ ] Memory within bounds

### Observability
- [ ] `#[instrument]` on public functions
- [ ] Proper span fields
- [ ] Events for state changes
- [ ] No secrets in logs

### Reliability
- [ ] CancellationToken passed to all async ops
- [ ] Proper cleanup on cancellation
- [ ] Atomic writes (write-rename pattern)
- [ ] Resource cleanup (Drop implementations)

## Test Validation

### Coverage
- [ ] All tests pass
- [ ] Coverage ≥ 80%
- [ ] All new code has tests
- [ ] Tests follow AAA pattern (Arrange-Act-Assert)

### Quality
- [ ] No flaky tests
- [ ] Integration tests pass
- [ ] E2E tests pass
- [ ] Architecture contract tests pass

## Integration Validation

### Component Interaction
- [ ] Component interfaces match design
- [ ] No circular dependencies
- [ ] End-to-end flows work
- [ ] Error propagation across boundaries

## CI/MR Validation

### Build & Test
- [ ] Build succeeds
- [ ] All tests pass
- [ ] Lint passes
- [ ] Format check passes
- [ ] Security audit passes

### Merge Readiness
- [ ] Required approvals present
- [ ] CI checks all green
- [ ] No merge conflicts
- [ ] PR description complete
- [ ] Issues linked
