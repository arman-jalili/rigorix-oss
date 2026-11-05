# Architect operating rules

Two roles, and mixing them is what produced the Phase C mess.

**The architect** decides, specifies and audits. The architect writes the module definition, files
the issues, states what is true, and verifies what came back. **The worker** implements what an
issue describes, and must be able to do so without making a decision.

When the architect also implements, three things happen, all of them observed:

- the specification gets written *after* the code, so it describes whatever the code became — the
  `gate` module's doc certified that its whole vocabulary belonged in `application/dto.rs`, and
  the module shipped with one DDD layer (#1406);
- verification starts being about the author's reasoning rather than the command, so five
  consecutive red runs each came from something the architect never executed;
- issues are written from confidence instead of from the tree, so the worker invents the parts
  that were not verified — a fourth `PreconditionOutcome`, a `GatedAction` accessor, an
  "obvious" adapter signature.

## Before filing anything

1. **Read the code the issue is about, and cite it.** Every code-bearing statement carries
   `file:line`, verified in the commit the worker will start from. If a signature cannot be
   verified, the issue is not ready. State the rule in the issue: *if a signature is wrong at
   your commit, file the delta — do not invent an accessor, and do not add a helper to make the
   compiler accept a guess.*
2. **Decide the end state.** No options, no recommendations, no "decision needed". If the issue
   cannot be implemented without a choice, the architect has not finished.
3. **State what has already landed**, with the commit it landed in. An issue that describes a
   half-done change as untouched makes the worker redo it or fight it.
4. **Give the acceptance as commands and outputs**, not as prose. "Verified: `bash <script>`
   prints X" beats "the slice must be clean". Where a guard is involved, name the seed that
   proves it fires.
5. **Name the prerequisite, and make it a separate issue.** An issue whose exit depends on
   another open issue never closes, and a finished issue that stays open is indistinguishable
   from unfinished work (#1386 carried the `default = ["engine"]` flip as its last line).

## Before building a bounded context

The definition precedes the directory. In order:

1. the module doc in the module-template's shape — Overview, `## DDD Layers` naming exactly the
   directories that will exist, Components by Layer, Data Flow, Degradation Strategy,
   Dependencies in both directions, Acceptance Criteria, Testing Requirements;
2. one ADR if a decision is being made, amending the existing ones where it belongs rather than
   sitting beside them;
3. the issue chain, each with verified signatures;
4. the code.

A layer that is deliberately absent is declared **in the doc, with a reason** — that is different
from an exemption that lets any module omit any layer.

## A seed is a hypothesis about a line, not a wish

Every seed in an issue asserts what a mutation will do. That assertion is about code, so it has to
be read off the code before it is written down — from the line that computes the value, not from the
name of the property.

The failure this rule exists for (#1414, revision 1): a seed required the decision digest to be
*unchanged* when a declared instant changed. `gate/domain/decision.rs:64` hashes `(verdict,
findings)`; `compose.rs:81` puts the whole finding into `evidence`; `service_impl.rs:177` sets
`checked_at` from the observation. A correct implementation therefore *had* to fail the seed. The
worker could not satisfy it and was forbidden to invent, so it stopped and asked — the issue became
the blocker, and the architect became the pipeline's bottleneck.

Before writing a seed, name the computing line and say what it reads. Two failure modes disappear
with that one habit: a seed that cannot fire, and a seed whose required outcome is backwards.

## Before saying "done"

1. **Run the command, not the reasoning.** The CI job's own command, from the directory the job
   runs it in (`bash .pi/scripts/local-ci.sh --stage=<stage>`, `bash .github/scripts/<script>`).
2. **Reproduce the environment, not just the text.** `CARGO_TERM_COLOR=always` broke a log parser
   on every crate while the same command was green locally; `mktemp -t name` is BSD-only and died
   on GNU before running a single doctest. If a script parses tool output, its seed constructs the
   output the tool actually emits.
3. **A gate that has never run is not a gate.** Check that the stage you rely on executes: seven
   stages were skipped for the whole of Phase C behind one lint edge, so "main is green apart from
   X" meant "the tests never ran" (#1411).
4. **Fix the cause, do not relocate the problem.** `--no-tests=warn` for an empty selection,
   a size tolerance for paths in the artefact, a skip list for a failing validator — each moves
   the symptom and keeps the cause.
5. **Never widen or narrow a check to make a run green** without saying so in the commit message
   and filing the residue.

## Worktree discipline

- Do not stage a file another agent is editing. `git diff <file>` first; stage hunks. A whole-file
  `git add` swept an unfinished `ci.yml` step into a commit, and main failed with exit 127 for a
  script that was never committed.
- Do not commit onto whatever branch happens to be checked out. Push explicitly
  (`git push origin HEAD:main`) and verify with `git cat-file -e origin/main:<path>`.
- Leave the other agent's files alone, and say which ones they are.

## The recurring defect shape

A word that outruns its evidence: `verified`, `derived`, `adopted`, `matched`, `anchored`,
`already current`, `portable`, `byte-identical`, `no tests to run`. When a change introduces or
uses one of these, the change carries the proof — the command, its output, and the seed that
makes it fail.
