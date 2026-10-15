# Assets

Images and recordings referenced from the top-level `README.md`.

## Required: a visual of a refusal

The README's **Watch it stop** section needs a **GIF or short video of an actual
refusal** — the action is attempted, the gate refuses, the record is written. A
paragraph describing a refusal is not a visual; this issue (#1012) is explicit that
the README must show one.

**Status: not yet recorded.** The README intentionally does **not** reference an
image here yet. A broken `<img>` is worse than no image, and a synthetic
"terminal-style" animation would be a fabrication dressed as a capture.

### What to record

The [payments-demo](https://github.com/arman-jalili/payments-demo) already produces
the scenario:

```bash
git clone https://github.com/arman-jalili/payments-demo
cd payments-demo
npm install && node .rigorix/run-demo.mjs
```

Capture the moment the plan drifts to `src/auth.ts` and the step marked
`requires_approval = true` stops execution. Keep it under ~15 seconds and readable
at README width (~800px wide is plenty).

### How to attach it

1. Save as `docs/assets/refusal.gif` (or `.mp4` — prefer GIF: it autoplays inline
   in a README, `.mp4` does not).
2. Replace the `<!-- Visual: ... -->` comment in `README.md` with:

   ```html
   <img src="docs/assets/refusal.gif" alt="Rigorix refusing a step that drifts into a file the plan was not cleared to touch" width="800">
   ```

3. Re-derive nothing else — the surrounding prose is already accurate.

### Why this file exists rather than the image

Recorded here so the requirement is discoverable and the deferred acceptance
criterion is not silently lost. `#1012` AC3 remains **open** until the file lands.
