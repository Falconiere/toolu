# brainstorm

Think a change through before building: evidence-backed triage, alternatives, trade-offs, and a recommended approach, without editing code. Standalone, and phase 1 of delivery-flow. No dependencies.

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

```
/plugin install brainstorm@toolu
```

Standalone, no dependencies. For Codex: `codex plugin add brainstorm@toolu`, or `npx @toolu/plugins install brainstorm --host codex`.

## What it provides

- `/brainstorm:brainstorm` (Claude Code) or `$brainstorm:brainstorm` (Codex). It triages the request as Minimal, Compact, or Full, grounds the decision in repository evidence, sets defaults, and asks at most one structured question when a goal-defining or hard-to-reverse fork remains.
- A capsule in chat every time: Outcome, Material defaults/non-goal, Repository evidence, Risk, and Handoff. On the Full path, or when you ask, it also writes `docs/toolu/brainstorms/<YYYY-MM-DD>-<slug>.md`.
- Standalone use never edits code or starts delivery. `delivery-flow` depends on this plugin and runs it as phase 1, then hands off to its spec phase.
