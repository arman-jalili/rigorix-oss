---
name: debug
description: Systematic debugging with reproduction-first approach.
---

# Debug Skill

Systematic debugging using: observe → hypothesize → verify → fix.

## Protocol

### Step 1: Observe
1. Read the full error message and stack trace
2. Identify the failing component and function
3. Note the exact conditions (input, state, environment)
4. **Do NOT start fixing yet** — this is investigation only

### Step 2: Reproduce
1. Create a minimal reproduction of the issue
2. Run the reproduction to confirm the error
3. Document the reproduction command and output

### Step 3: Hypothesize
1. Form a theory about the root cause based on evidence
2. State the hypothesis explicitly
3. Identify what would confirm or refute it

### Step 4: Verify
1. Test the hypothesis (add logging, isolate the function, etc.)
2. Confirm or refute the hypothesis
3. If refuted, return to Step 3 with new information

### Step 5: Fix
1. Implement the fix targeting the root cause
2. Run the reproduction to verify the fix
3. Run full test suite to ensure no regressions

## Commands

```bash
# Run specific test to reproduce
cargo test <test_name> -- --nocapture
bun test <file> --test-name-pattern='<pattern>'
pytest <file> -k <pattern> -v

# Add logging for investigation
# Use the project's existing logging framework

# Run full suite after fix
[full test command]
```

## Rules

- NEVER fix without understanding the root cause
- NEVER make random changes hoping something works
- ALWAYS reproduce the issue before fixing
- ALWAYS verify the fix works with the reproduction
- ALWAYS run the full test suite after fixing
