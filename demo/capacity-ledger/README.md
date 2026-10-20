# Capacity ledger demo (ADR-018)

One command, no Docker, no network:

```bash
demo/capacity-ledger/run.sh
```

It writes a throwaway `.rigorix/capacity.toml` and drives the real engine types —
`CapacitySetup::from_env` → `CapacityLedgerService` in act 1, and the same setup
armed into `ParallelExecutionFactoryImpl` → `execute_graph` in act 2. Nothing is
mocked, and every number in the transcript is read back from the ledger after the
operation that changed it.

| Act | Shows | Why it matters |
|---|---|---|
| 1 | reserve → retry → refusal → commit / release, with exposure after each step | a retry is not a second hold, a refusal reserves nothing, and `committed`/`outstanding` are reported separately (#1053, #1090) |
| 2 | two executions contending for a cap of 4 with 3-unit steps, through the real executor | exactly one dispatches; the other is refused with `capacity_denied` **before** its tool runs — its marker file never appears |

The marker file in act 2 is the actual proof. A counter that runs the tool and
reports afterwards cannot produce an absence.

## Recorded output

```text
Rigorix capacity ledger — ADR-018
committed █ | outstanding ▒ | free ·

Declaration: .rigorix/capacity.toml (in a temp dir, written by this demo)
  resource payments-budget: hard_limit 5, window 3600s
  step "pay": amount 3, lease 60s
  armed: true

── Act 1 — the contract ──

1. execution A reserves 3
   ▒▒▒··   committed 0, outstanding 3, available 2 of 5

2. A retries with the same idempotency key
   ▒▒▒··   committed 0, outstanding 3, available 2 of 5
   same reservation: true — a retry is not a second hold

3. execution B reserves 3
   refused: insufficient capacity for resource 'payments-budget': requested 3, available 2
   ▒▒▒··   committed 0, outstanding 3, available 2 of 5
   unchanged — a refusal reserves nothing

4. B reserves what is left: 2
   ▒▒▒▒▒   committed 0, outstanding 5, available 0 of 5

5. A commits 3 — its work happened
   ███▒▒   committed 3, outstanding 2, available 0 of 5
   committed + outstanding = 3 + 2 = 5 (never one number)

6. B releases 2 — its work did not happen
   ███··   committed 3, outstanding 0, available 2 of 5

── Act 2 — the seam, through the real executor ──
  hard_limit 4, step amount 3, two executions launched together
  dispatched: 1, refused before dispatch: 1
  tool markers on disk: 1 of 2 (the refused step never ran its tool)
  ███·   committed 3, outstanding 0, available 1 of 4
  refusal: capacity_denied (the step's tool was never called)

ok — the cap held, and the refused step's tool never ran.
```

## What this demo does *not* prove

The in-process adapter gives **single-process atomicity**: every execution in this
process that shares the instance contends on one aggregate. It is not
cross-session protection — two processes, two replicas or two hosts are outside
it. That requires the HTTP adapter against a real contested store
(`HttpLedger`, ADR-018), and no deployment may claim the second while running the
first. See the [capacity-ledger module spec §Guarantee
ceiling](../../engine/.pi/architecture/modules/capacity-ledger.md).

It also does not show *observability*: exposure is readable in-process (as act 1
does) and over the HTTP adapter's route, but there is no OSS subcommand or server
route that answers "why was my step refused" from outside the process. See the
[capacity-ledger runbook §Observing
refusals](../../engine/docs/runbook-capacity-ledger.md).
