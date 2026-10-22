# Issue packets

A packet is the specification for one component of an epic: in scope, out of
scope, layers, canonical references, acceptance criteria, validators.

## Closing a packet

A packet moves from `status: planned` to `status: done` only when its acceptance
criteria are met **in code**, not when the epic merges. The same edit adds a
`closed_reason:` naming the implementation and the PR (or commit) that landed it:

```yaml
  status: done
  closed_reason: "Implemented in #1143 — application/service_impl.rs; begin derives the intent key before recording"
```

Rules:

- Do not bulk-flip: check each packet's criteria against the source.
- Do not delete a packet: it is the record of what was specified, including what
  was deliberately left out.
- If a packet is superseded, say so in `closed_reason` and name the replacement.
- A `status: planned` packet for merged work makes the ledger lie in the other
  direction; the same rule applies to module docs and wiring claims.
