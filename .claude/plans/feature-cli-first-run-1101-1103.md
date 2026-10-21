# Batch — CLI first-run: init, key, doctor (#1101, #1102, #1103)

**Branch:** `feature/cli-first-run-1101-1103`

## Why these three are one batch
Same component (`cli/src/cli_boundary/`), same failure: **a fresh repo learns nothing, and every gate
is silently unarmed**. #1103's own framing — "every other usability finding in this set compounds
into this one question" — makes that the spine.

| Issue | Defect | Fix |
|---|---|---|
| #1103 | no way to ask "is enforcement armed here?" | `rigorix doctor`: probe each seam through the *same* constructor the factory uses, print armed/unarmed/misconfigured |
| #1101 | `init` scaffolds no seam config, names no env var, and early-returns so older repos never hear it | scaffold gains a commented seam block; the advisory runs **even when `.rigorix/` exists** |
| #1102 | `key` prints a UUID that nothing consumes, labelled "API key" | produce the key the readers actually consume (`RIGORIX_HMAC_KEY`), and say where it goes |

## Order
1. **#1103** — the seam inventory. `init`'s advisory reuses it, so it lands first.
2. **#1101** — advisory + commented scaffold, using the inventory.
3. **#1102** — `key` honesty.

## Design notes
- The probe calls the same `*Setup::from_env` the composition root calls. A doctor that reimplements
  the check would be a second source of truth and could disagree with the thing it reports on.
- `Misconfigured` (a seam that is present but cannot be honoured, e.g. #1070's
  `RIGORIX_APPROVAL_BINDING=1` without a key) exits non-zero. `Unarmed` does not: an unarmed gate is
  a valid default, it is just not the same thing as an armed one, which is the whole point.
- No secret is ever printed — the signing row reports presence and length only.

## Validation
`cargo test -p rigorix-cli`, clippy `-D warnings`, CI 5/0, architecture/canonical/security, docs.
Falsifier: a temp repo with `RIGORIX_APPROVAL_BINDING=1` and no key must report `misconfigured` and
exit 1; the same repo without the env var must report `unarmed` and exit 0.
