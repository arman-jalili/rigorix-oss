---
guardian_issue:
  id: "ISSUE-KEY-COMMAND-EMITS-UNCONSUMED-UUID"
  title: "cli: `rigorix key` prints a random UUID that nothing consumes — 26 readers of RIGORIX_HMAC_KEY, zero of this value"
  epic: "docs hygiene"
  component: "CLI"
  module: "cli-boundary"
  status: planned
  priority: medium
  dependencies:
    - "#1068 — the HMAC key's custody and environment exposure"

  intent: |
    `rigorix key` generates a UUID and prints it as an API key. Nothing reads it. It is not the
    envelope HMAC key, not the approval-binding HMAC key, not the anchor signing key. A user who runs
    it reasonably concludes they now hold a credential the engine will accept, and they do not.

  in_scope:
    - "Decide what the command is for: emit real key material in the format the readers expect, or remove/rename it"
    - "If it emits key material, state custody explicitly (who holds it, where it must not be written)"
    - "If it is a placeholder, say so in the output rather than printing a bare credential-shaped value"

  out_of_scope:
    - "Key rotation (separate concern)"
    - "Changing how RIGORIX_HMAC_KEY is read"

  affected_layers:
    domain:
      - "None"
    application:
      - "cli_boundary/dispatch.rs — cmd_key"
    infrastructure:
      - "None"
    api:
      - "None"

  canonical_references:
    - evidence: "cli/src/cli_boundary/dispatch.rs:126-134 — cmd_key builds serde_json json!({'key': Uuid::new_v4(), 'label': label})"
    - evidence: "no reader of that value exists; RIGORIX_HMAC_KEY has 26 reader sites and is a different secret"
    - evidence: "the output is shaped like a credential (a key field and a label), which is what makes it misleading"

  acceptance_criteria:
    - "The command either produces material the engine accepts, or it does not present itself as doing so"
    - "A test asserts the emitted value is accepted by the seam that consumes it (if it emits a real key)"
    - "The help text states the difference between this key and RIGORIX_HMAC_KEY"

  validators:
    - tests
    - security

  implementation_notes: |
    A UUID v4 is 122 bits of randomness and looks like a secret; that is exactly why printing it under
    the name "key" is harmful. Nothing about the value is wrong as a random string — the problem is
    the claim the command makes by naming it a key and returning `Generated API key`.

    Note the asymmetry that makes this worth filing: 26 call sites read `RIGORIX_HMAC_KEY`. If a user
    needs a key today, the thing they must set is one none of the scaffolding mentions and this
    command does not produce.

  file_changes:
    - "modify: cli/src/cli_boundary/dispatch.rs"

  pipeline_steps:
    - implement
    - validate
    - create-mr
    - merge
---

# ISSUE-KEY-COMMAND-EMITS-UNCONSUMED-UUID

```rust
fn cmd_key(label: Option<String>) -> DispatchResult {
    let key = uuid::Uuid::new_v4();
    let data = serde_json::json!({ "key": key.to_string(), "label": label_str });
    DispatchResult::success_with_data(format!("Generated API key ({label_str})"), data)
}
```

## The asymmetry

| value | readers |
|---|---|
| the UUID printed by `rigorix key` | **0** |
| `RIGORIX_HMAC_KEY` / `AUDIT_ENVELOPE_HMAC_SECRET` | **26** |

So the command named "key" produces the one key nothing consumes, while the keys that gate every
envelope are unnamed in `init` and absent from `key`.

## Why it is a real defect and not cosmetic

The output says `Generated API key (default)` and returns a field called `key`. Someone who runs it
will store that value and believe they have configured authentication. They have configured nothing,
and the failure will appear later as an unverifiable envelope rather than as a wrong key — which is
the harder failure to diagnose.

Either make it produce the key the 26 readers want, or stop calling a UUID a key.
