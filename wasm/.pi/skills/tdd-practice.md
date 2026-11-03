---
name: tdd-practice
description: TDD practice guide for Guardian implementation agents. Use when TDD mode is enabled on an epic — follows Red→Green→Refactor with pre-generated failing tests from architecture contracts.
---

# TDD Practice — Red → Green → Refactor

> This skill applies when TDD mode is enabled on the current epic (`--tdd`).
> Pre-generated failing test files exist in the project's test directory
> (language-specific: `tests/unit/` for TypeScript, `src/test/java/` for Java,
> `tests/` for Python/Go/Rust).
> These tests were generated deterministically from architecture contracts — no LLM involvement.

## Core Principle

Tests are **living artifacts**. They evolve with the code:

1. They start as minimal failing tests generated from architecture contracts
2. As you implement, you expand them to cover real behavior
3. When you refactor, you update both code and tests
4. They remain as regression protection for the lifetime of the component

Do NOT treat the generated tests as rigid specs you must match exactly. Treat them as a starting point. If the implementation reveals a better API than the generated test assumes, update the test.

## Workflow

### 1. Red — See the test fail

Run the project's test runner on the generated test file:

```bash
# TypeScript: bun test tests/unit/<module>/<component>/<component>.test.ts
# Java:       mvn test -Dtest=<Component>Test
# Python:     pytest tests/<module>/<component>/test_<component>.py
# Go:         go test ./tests/<module>/<component>/
# Rust:       cargo test --test <component>
```

Confirm the test fails before writing any implementation code. This validates that:
- The test infrastructure works
- The test is actually testing something (not a false positive)
- You understand what "not implemented" looks like

### 2. Green — Make the test pass

Write the minimal implementation to make the failing test pass.

**Guidelines:**
- Write only enough code to pass the test
- Do not over-engineer — YAGNI
- Use the test's TODO comments as a guide for what the API should look like
- The test file is in the project's test directory (`tests/unit/`, `src/test/java/`, or `tests/`), implementation files go in `src/`
- Import the implementation module into the test file

If the generated test assumes an API shape that doesn't fit, **update the test first**, then implement.

Example — given a generated test:

```typescript
it("should be defined", () => {
  // TODO: uncomment when MyComponent is implemented
  // const instance = new MyComponent();
  // expect(instance).toBeDefined();
});
```

**Step 1:** Uncomment and update to match the actual API:

```typescript
import { MyComponent } from "../../../src/module/my-component";

it("should be defined", () => {
  const instance = new MyComponent();
  expect(instance).toBeDefined();
});
```

**Step 2:** Create the minimal implementation:

```typescript
export class MyComponent {
  // Minimal implementation — add methods as tests require them
}
```

**Step 3:** Run the test — it should pass.

### 3. Refactor — Improve without breaking tests

Once the test passes, improve the code quality:

- Extract duplication
- Add error handling
- Follow Clean Architecture / DDD patterns
- Run tests again to confirm nothing broke

## Handling Generated Tests

The generated tests are **suggestions, not constraints**. You should modify them when:

- **API mismatch**: The real component has a different constructor signature or method name — update the test
- **Missing edge cases**: Add new `it()` blocks for edge cases the generator didn't anticipate
- **Wrong test type**: The generator created an instantiation test but what you need is a behavior test — rewrite it
- **Layer evolution**: The generator guessed a domain test but the component only needs application-layer behavior — adjust

**Do NOT** delete test files. If a test is genuinely wrong, fix it. If a component doesn't need tests (rare), leave the file with a comment explaining why.

## Test Structure Conventions

- One `describe` block per component: `describe("ComponentName", () => { ... })`
- Descriptive `it` names explaining the behavior: `it("should reject invalid input")`
- Use `beforeEach` for shared setup
- No test interdependencies — each `it` runs independently
- Mock external dependencies (APIs, databases) at boundaries

## File Locations

| Artifact | TypeScript | Java | Python | Go | Rust |
|----------|------------|------|--------|-----|------|
| Tests | `tests/unit/.../component.test.ts` | `src/test/java/.../ComponentTest.java` | `tests/.../test_component.py` | `tests/.../component_test.go` | `tests/.../mod.rs` |
| Implementation | `src/<module>/` | `src/main/java/.../` | `src/<module>/` | `src/<module>/` | `src/<module>/` |
| TDD guide | `.pi/skills/tdd-practice.md` | same | same | same | same |

## Anti-Patterns

- ❌ Writing all tests upfront before any implementation code (only the generated skeleton exists)
- ❌ Making tests pass without understanding what they test
- ❌ Deleting generated tests because they "look hard to implement"
- ❌ Adding production code that has no corresponding test
- ❌ Skipping the "Red" step — you need to see the test fail to know it can pass
- ❌ Over-mocking — prefer real objects when feasible
- ❌ Treating generated tests as immutable contracts
