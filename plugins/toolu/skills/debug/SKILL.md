---
name: debug
description: Find and fix the root cause of a crash, failing/flaky test, wrong result or regression. Use for broken behavior; not feature design, routine review or external research.
---

# Debug

This break-glass loop is not a chain step: any phase can enter it and return. Gather evidence before claims; if the same approach fails twice, change the hypothesis.

Use the mandatory [Jev workflow](../../workflows/semantic-judgments.md) after
observations to compare hypotheses and prioritize experiments; allow insufficient evidence.
Reproduction/tests establish causes.

## The one rule that matters

**Find and fix the root cause.** Silencing errors, adding retries, or special-casing an input can leave the defect intact.

## The loop

1. **Reproduce reliably.** Get a deterministic failing case before changing anything. If you cannot reproduce it, that *is* the first problem — narrow conditions (input, env, timing, order) until it fails on demand. A bug you can't reproduce, you can't prove you fixed.
2. **Observe — gather evidence, do not guess.** Read the actual failure: test output, stack trace, logs, runtime state. Pipe raw signal through the evidence helpers (below) so it lands compact, not as a wall of text. Let the evidence narrow the search; never start from a hunch about the cause.
3. **Hypothesize — one falsifiable claim.** State a single hypothesis precise enough to be *wrong*: "the token-expiry check uses `<` where it needs `<=`, so tokens expiring this exact second pass." Vague hypotheses ("something with auth") can't be tested.
4. **Instrument & test the hypothesis.** Prove or kill it: add a targeted log/assert, bisect (`git bisect`, or halve the input/code path), or inspect the exact value. One change at a time — change two things and you learn nothing from the result.
5. **Isolate the root cause.** Trace from symptom to the actual defect. Confirm it explains *all* the observed evidence, not just the loudest symptom. If your fix wouldn't explain every data point from step 2, you haven't found the cause yet.
6. **Fix at the root, verify red → green.** Apply the minimal fix at the cause. Re-run the step-1 reproduction: it must go from failing to passing. No green reproduction, no fix.
7. **Regression test (real data, no mocks).** Add a test that fails before the fix and passes after, in the toolu layout — see the `test` skill (TS `__tests__/`, Rust `tests/`). This is what stops the bug coming back.
8. **Record.** Save the bug to comemory so the next encounter is a hit, not a re-investigation:
   `comemory.sh save "<symptom>" "Root cause: <cause>. Fix: <what changed + path>." --kind bug`

**Failed twice? Stop.** If two distinct attempts on the same hypothesis both failed, the hypothesis is wrong — return to step 2, gather more evidence, form a new one. Retrying a third time is how you burn a session.

## Evidence helpers (the Observe step)

Four language-agnostic native commands turn raw failure output into a compact, capped summary so Observe doesn't flood context. Each reads stdin or `--file <path>`; put the global `--json` before `debug` for structured output. Caps are env-overridable (`DEBUG_MAX_*`).

- `toolu debug io` — count input bytes and lines before summarizing a large capture.
- `toolu debug testfail` — failing-test transcript → failed test names, error/assertion lines, code `file:line` locations. For example, `cargo test 2>&1 | toolu debug testfail`.
- `toolu debug stack` — stack trace / backtrace → app frames first, framework/runtime frames collapsed.
- `toolu debug log` — large log → deduped error/warn lines + a tail, hard-capped in lines and bytes.

Use them to *seed* the investigation; they observe, they don't diagnose.

## Sentry adapter (opt-in, best-effort)

When the bug originates from a Sentry issue and the Sentry MCP is authenticated, you can pull the event to seed Reproduce + Observe:

1. The Sentry MCP's fetch tools only appear **after** OAuth — discover them at runtime with `ToolSearch` (e.g. query `+Sentry issue event`); do not assume tool names. If only `mcp__claude_ai_Sentry__authenticate` is present, the user hasn't connected it.
2. Fetch the issue/event the user names (URL or short-id), extract the exception + stack + breadcrumbs, pipe the stack through `toolu debug stack`, then proceed from Observe.
3. **If Sentry is unavailable, unauthed, or exposes no fetch tool:** say so in one line ("Sentry unavailable, proceeding manually") and continue — ask the user to paste the stack/error. The loop never depends on Sentry.

## Return to the chain

A debug session ends by handing back: the fix re-enters `execution`, uses `test` to write and confirm the regression evidence, then completes execution's local readiness checks and authorized PR handoff. Don't call the work done from inside the loop — a fix without a regression test and a green gate isn't done.
