# toolu-review

Project-tuned pre-push code review mirroring the CI review bot's checklist (correctness, security, perf, test coverage, doc accuracy), writing the toolu `push-review` state so the gate passes.

## Install

```
/plugin install toolu-review@toolu
```

Standalone, no dependencies.

## What it provides

- **`toolu-review:review` skill** — reviews the branch diff against what this repo's CI review bot (the `claude[bot]` verdict comment) flags, so the bot's verdict is clean on the first push instead of bouncing low/nit findings back as rework. It also records a clean `push-review` state, satisfying toolu's `push-review` gate.

Explicit — it does not auto-fire on edits. Run it before pushing a feature branch, or when `pr-babysit` needs a reviewer.
