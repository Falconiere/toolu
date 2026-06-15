---
name: execution-review
description: "Use after implementing, to review the work BEFORE calling it done — does it match the plan, is every error handled (never suppressed), are tests real-data, is the gate green. Tells: \"review the execution\", \"review what I built\", \"is this done\", \"audit the implementation\". Pairs with `execution`; runs before test. For deep bug-hunting, defer to /code-review."
---

# Execution Review

The last gate before work is called done. It checks that what got built matches what was planned and holds the house conventions — with a hard focus on error handling, the thing most likely to be silently wrong. This is a conventions-and-completeness audit; for deep correctness bug-hunting use `/code-review`, and point at it when the change warrants it.

Review the diff (the executed work) against the checklist. For each item ask *would I be comfortable shipping this?*

## Checklist

### Error handling (the priority)
- **Every fallible path is handled** — async/`await`, I/O, parsing, `Result`-returning calls. Errors are propagated (`?`, rethrow), matched, or converted — never swallowed.
- **Nothing is suppressed** — no `@ts-ignore` / `@ts-expect-error` / `eslint-disable` / `biome-ignore` / `#[allow]` / `#[expect]` papering over a real problem. Fixed in code, per the standing rule. A suppression comment is a finding, not a style choice.
- **No silent swallow** — no empty catch, no `catch { return null }`, no `.catch(() => {})`, no `.unwrap()`/`.expect()`/`panic!` on a fallible path in `src/`. Errors carry a message and a type.

### Matches intent
- **Implements the plan** — the change does what the reviewed plan said, no silent additions or omissions. Scope creep is a finding.
- **No dead ends** — no TODO/stub left where the plan expected working code.

### Conventions
- **Tests** — real-world data only (no mocks), colocated (`__tests__/` TS, `tests/` Rust), and they actually exercise the new behavior including its failure paths.
- **Structure** — one responsibility per file, named after its export, under the size ceiling; no leftover duplication of existing helpers.
- **Docs** — public/exported symbols carry a concise doc; nothing verbose.
- **Docs in sync** — user-facing docs match the shipped behavior: README, `docs/` guides, `SKILL.md` trigger text, and release notes reflect any changed interface/CLI/command/config. A stale doc is a finding.
- **Gate is green** — the quality gate passes; the change didn't land by working around it.
- **Ledger is fresh-green** — for ledger-tracked work, `bash plugins/toolu/hooks/lib/plan-ledger.sh status` reports every step fresh-green (status==green against the current diff_sha) before done; any red/pending/stale step is a blocker.

## Output

One line per finding, severity-tagged, location + problem + fix — no praise:

```
path:line: 🔴 blocker: <problem>. <fix>.
path:line: 🟡 should-fix: <problem>. <fix>.
path:line: 🔵 consider: <minor>. <fix>.
```

End with a verdict:
- **Approved** → ready for the final `test` pass / merge.
- **Needs changes** → list the blockers; back to `execution`.

Be adversarial about error handling specifically — that's where "looks done" and "is done" diverge. If you find nothing there, say you looked and it held.
