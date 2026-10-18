---
guardian_issue:
  id: "ISSUE-RUN-COMMAND-POLICY-SEES-WRONG-STRING"
  title: "run_command: the allowlist checks a prefix of a compound shell string, and check_bash is fed the JSON intent instead of the command"
  epic: "execution boundary"
  component: "RunCommandTool"
  module: "tools"
  status: planned
  priority: critical
  dependencies: []

  intent: |
    Make the command policy see the command. Two independent defects make the `run_command`
    policy decidable in the caller's favour:

    1. `is_command_allowed` matches a **prefix** of the whole string, which is then handed to
       `/bin/sh -c`. A command beginning with an allowed token can append another operation
       using a shell separator. The policy approves a prefix; the shell executes a compound.
    2. The engine dispatch path calls `check_bash(tool_intent, None)` where `tool_intent` is the
       **serialized JSON intent**, not the command. `BashClassifier::classify` splits on
       `|`/`;`/`&&` and takes `tokens[0]`, so it sees a fragment of JSON and never classifies
       the destructive command inside it.

    Together: the allowlist can be satisfied by a prefix while the classifier is looking at a
    different string than the shell will execute.

  in_scope:
    - "Refuse shell control operators in the accepted command, or execute without a shell"
    - "Pass the extracted command string to check_bash, not the intent JSON"
    - "Make classification cover the whole command, not only the first token"
    - "Adversarial tests: ; && || | redirection subshell command-substitution backticks newline"
    - "A test asserting the classifier receives the same bytes the shell will execute"

  out_of_scope:
    - "OS-level isolation of the child process (separate issue)"
    - "Implementing a full shell grammar — the preferred fix is not using a shell"

  affected_layers:
    domain:
      - "None"
    application:
      - "Modified: RunCommandTool — command acceptance and execution"
      - "Modified: execute_tool — pass the command, not the intent"
      - "Modified: BashClassifier — whole-command classification"
    infrastructure:
      - "Modified: process spawn (argv instead of sh -c where possible)"
    api:
      - "None (input shape unchanged)"

  canonical_references:
    - adr: ".pi/architecture/decisions/ADR-011-approval-binding.md"
    - evidence: "engine/src/tools/application/run_command_tool.rs:69 — prefix-only is_command_allowed"
    - evidence: "engine/src/tools/application/run_command_tool.rs:125 — tokio::process::Command::new(\"/bin/sh\").arg(\"-c\")"
    - evidence: "engine/src/execution_engine/application/service_impl/tools.rs:58 — enforcer.check_bash(tool_intent, None)"
    - evidence: "engine/src/execution_engine/application/service_impl/tools.rs:17 — tool_intent = &node.intent (a JSON string)"
    - evidence: "engine/src/permission/domain/bash_classifier.rs:314-337 — splits on | ; && and takes tokens[0]"

  acceptance_criteria:
    - "A command containing a shell control operator is refused, or is executed as argv without a shell"
    - "check_bash receives the command text the process will run"
    - "A destructive command in the second position is refused"
    - "Adversarial tests cover each control operator"
    - "Existing allowlist behaviour for single simple commands is unchanged"

  validators:
    - ci
    - tests
    - security

  implementation_notes: |
    - Do not sanitize by splitting on separators; quoting and nesting defeat it. Either forbid
      shell metacharacters outright (simple and checkable) or execute argv directly and drop the
      shell (better, but changes the tool contract for pipes and redirection).
    - The classifier receiving the wrong string is the more dangerous half: the control appears
      present in the dispatch path while classifying JSON. Pin it with a test that the bytes the
      classifier sees equal the bytes the shell gets.
    - The allowlist match requires `prefix + space` or `prefix + '/'`, so it does not accept a
      longer token with the same stem. Keep that and add the metacharacter rule.

  file_changes:
    - "modify: engine/src/tools/application/run_command_tool.rs"
    - "modify: engine/src/execution_engine/application/service_impl/tools.rs"
    - "modify: engine/src/permission/domain/bash_classifier.rs"
    - "create: engine/tests/unit/tools/run_command_policy_test.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-RUN-COMMAND-POLICY-SEES-WRONG-STRING

## Two defects, one seam

**1. Prefix allowlist over a compound string** — `run_command_tool.rs:69` accepts a command
when it equals a prefix, or starts with `prefix + " "` / `prefix + "/"`. Line 125 then runs it
via `/bin/sh -c`. A command that begins with an allowed token and appends another operation
with a shell separator passes the allowlist and is executed by the shell as a compound.

**2. The classifier is fed the wrong string** — `tools.rs:17` takes `tool_intent = &node.intent`
(a JSON string) and `tools.rs:58` passes it to `check_bash`. `BashClassifier::classify` splits
on `|`, `;`, `&&` and takes `tokens[0]`, so for an intent blob that token is a fragment of JSON
such as `{"tool":"run_command","command":"rm`. The destructive intent inside is never classified.

## Suggested fix

1. **Refuse shell control operators** in the accepted command (`;` `&&` `||` `|` `` ` `` `$(` `>`
   `<`, newline), or execute argv directly and remove the shell. Do not split-and-check.
2. **Pass the command** to `check_bash`, extracted from the intent.
3. **Pin the invariant in a test**: the bytes the classifier sees are the bytes the shell
   receives.
