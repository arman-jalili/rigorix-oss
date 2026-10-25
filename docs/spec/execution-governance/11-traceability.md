# 11 — Traceability

**Status: informative.** Requirement → implementation → test → demonstration.

A requirement without a trace is an aspiration. This table is how the specification
stays honest.

| Requirement | Implementation anchor | Verification |
|---|---|---|
| EG-001 plan before execution | planning pipeline, DAG engine | integration tests; all demos |
| EG-002 sealed acyclic plan | dag-engine (two-phase, Kahn, cycle detection) | unit + integration tests |
| EG-003 no model in decision path | dispatch path; no LLM in gates | code review; ADR-007 |
| EG-004 reject unevaluable plan | fail-closed evaluation in sequence/precondition modules | negative tests |
| EG-010 approval required | approval module; `requires_approval` | migration/conference demos |
| EG-011 intent-bound, single-use, time-bound | ADR-011 canonical intent + HMAC | `approval_binding_tests` |
| EG-012 re-derive at dispatch, halt on mismatch | dispatch loop `HaltIntent` | approval-binding tests |
| EG-013 attested approver identity | ADR-012 device flow | identity epic tests |
| EG-014 fail closed on missing key | #1070 `Err(MissingKey)` | `approval_binding_env_tests` |
| EG-020 precondition at dispatch | ADR-017 `DispatchGate` | authority-demo; engine tests |
| EG-021 fail closed on ambiguity | ADR-017 failure taxonomy | failure-mode tests |
| EG-022 no caching across the gap | per-dispatch evaluation | engine tests |
| EG-023 no short-circuit between preconditions | ADR-017 R4 non-inheritance | R4 tests |
| EG-024 check and authority digests | signed findings | authority-demo output |
| EG-025 check outside agent write scope | `--isolated` mode; out-of-repo check | authority-demo boundary docs |
| EG-026 companion step must succeed | #1069 `evaluate_companion_prerequisite` | companion-prerequisite tests |
| EG-030 plan-time sequence evaluation | ADR-013 plan validation | sequence-policy tests |
| EG-031 prefix gate | dispatch loop prefix evaluation | engine tests |
| EG-032 cross-run over signed history | ADR-013 R7 | conference-demo Session A′ |
| EG-033 promote or refuse only | ADR-013 actions | config validation tests |
| EG-034 parameter cross-reference | ADR-014 JSON-pointer predicates | matcher tests |
| EG-035 canonical effect keys | ADR-014 | payouts-demo |
| EG-040 reserve-then-authorize | ADR-018 service + guard | capacity contract suite |
| EG-041 release on drop | `ReservationGuard` | guard tests |
| EG-042 refuse on unknown capacity | `LedgerError::Unavailable` | HTTP adapter tests |
| EG-043 no cross-session claim in-process | ADR-018 claim ceiling | docs review |
| EG-050 one gate for all routes | #1074 `gate.rs` | gate parity tests |
| EG-051 refuse when unevaluable | all gate modules | negative tests |
| EG-052 control posture reporting | `rigorix doctor` (#1103/#1105) | doctor tests; new seams must be added |
| EG-053 reject compound shell | #1067 | `run_command` tests |
| EG-054 no secret inheritance | #1068 cleared env | subprocess tests |
| EG-060 decision record with inputs | envelope findings | audit tests; demos |
| EG-061 degraded marking | `evidence_degraded` | audit tests |
| EG-062 chain | ADR-016 `prev_hash`/`sequence` | chain tests |
| EG-063 external anchor | Ed25519 anchor (enterprise) | anchor tests; SDK verify |
| EG-064 verifier states its level | SDK verifier surface | verifier tests |
| EG-065 offline verification | `rigorix-verifier` (5 languages) | published packages |
| EG-066 refusal reconstructable without transcript | signed finding | authority-demo |
| EG-070 all interfaces through the gate | server/MCP/CLI/actions | parity tests |
| EG-071 no direct-execute surface | catalog subset rule | catalog tests |
| EG-072 control plane not agent-writable | permission R5 | permission tests |
| EG-073 scope checked before planning | permission enforcer | permission tests |
| EG-080–082 claim discipline | this document set | review |
| EG-090 write-ahead before dispatch | ADR-019 `EffectCommitmentService::begin` | effect-commitment tests |
| EG-091 four-state machine, non-terminal Ambiguous | `EffectState` | state-machine tests |
| EG-092 dependents fenced (F1) | fence module | fence tests |
| EG-093 retry refused (F2) | reconciliation policy | retry tests |
| EG-094 intent-derived key | `IntentKeyDerivation` | key tests |
| EG-095 key exported to provider | `EFFECT_IDEMPOTENCY_ENV` | provider tests |
| EG-096 resolver authority (F3/F4) | `ResolverIdentity`, closed errors | resolution tests |
| EG-097 durability must name the store | ADR-019; doctor derived ceiling | docs review |
| EG-098 settlement requires receipt (L3) | provider receipt reader | receipt tests |
| Effect-scope evidence wired | #1106; plan declares scope (#1175) | R5 oracle tests |

## Known gaps

| Requirement | Status | Note |
|---|---|---|
| EG-052 control posture reporting | implemented, and fragile | every new control seam must be added to `rigorix doctor` or it becomes invisible to the discipline that closed EG-052 |
| EG-043 distributed claim ceiling | enforced in docs only | needs a machine-checkable mode assertion |
| EG-063 anchor as separate party | deployment property | cannot be enforced by code; must be documented per deployment |

Gaps are expected. A traceability table with no gaps is usually a table nobody
verified.
