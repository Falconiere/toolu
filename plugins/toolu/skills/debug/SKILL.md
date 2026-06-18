---
name: debug
description: "Use when something is broken and you need the root cause — a failing or flaky test, a crash, a stack trace, wrong output, a regression, a Sentry issue. Drives a scientific-debug loop: reproduce → observe evidence → falsifiable hypothesis → instrument/bisect → isolate the ROOT cause (not the symptom) → fix → regression test → record. Tells: \"why does X fail\", \"it crashes\", \"this is broken\", \"flaky test\", \"debug this\", \"root cause\", \"it worked before\", \"track down this bug\", \"stack trace\". A non-chain break-glass loop in the toolu workflow — any phase can drop into it and return."
---

# Debug

The break-glass loop of the toolu workflow. The 8-phase chain builds; this is what you reach for when something *broke*. It is not a chain step — any phase (most often `execution` or `test`) drops into it and returns. Its discipline is the session protocol made concrete: **evidence before claims; the same approach failed twice → stop and change the hypothesis, don't retry harder.**

**Trigger phrases:** why does X fail, it crashes, this is broken, flaky test, debug this, find the root cause, it worked before, track down this bug, what's wrong with, stack trace.

## The one rule that matters

**Find the root cause, then fix the root cause.** Patching the symptom — silencing the error, adding a retry, special-casing the failing input — is the single most expensive mistake in debugging, because the bug survives and the next occurrence is harder to see. Every step below exists to push you toward the cause and away from the symptom.

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

Three language-agnostic collectors turn raw failure output into a compact, capped summary so Observe doesn't flood context. Each reads stdin or `--file <path>`, takes `--json`, and degrades to a capped raw passthrough on input it doesn't recognize. Caps are env-overridable (`DEBUG_MAX_*`).

- `plugins/toolu/scripts/debug-testfail.sh` — failing-test transcript → failed test names, error/assertion lines, code `file:line` locations. `bun test out.txt | debug-testfail.sh` or `cargo test 2>&1 | debug-testfail.sh`.
- `plugins/toolu/scripts/debug-stack.sh` — stack trace / backtrace → app frames first, framework/runtime frames collapsed.
- `plugins/toolu/scripts/debug-log.sh` — large log → deduped error/warn lines + a tail, hard-capped in lines and bytes.

Use them to *seed* the investigation; they observe, they don't diagnose.

## Sentry adapter (opt-in, best-effort)

When the bug originates from a Sentry issue and the Sentry MCP is authenticated, you can pull the event to seed Reproduce + Observe:

1. The Sentry MCP's fetch tools only appear **after** OAuth — discover them at runtime with `ToolSearch` (e.g. query `+Sentry issue event`); do not assume tool names. If only `mcp__claude_ai_Sentry__authenticate` is present, the user hasn't connected it.
2. Fetch the issue/event the user names (URL or short-id), extract the exception + stack + breadcrumbs, pipe the stack through `debug-stack.sh`, then proceed from Observe.
3. **If Sentry is unavailable, unauthed, or exposes no fetch tool:** say so in one line ("Sentry unavailable, proceeding manually") and continue — ask the user to paste the stack/error. The loop never depends on Sentry.

## Return to the chain

A debug session ends by handing back: the fix re-enters `test` (write/confirm the regression test) and then `execution-review`. Don't call the work done from inside the loop — a fix without a regression test and a green gate isn't done.
