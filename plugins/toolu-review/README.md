# toolu-review

Project-tuned pre-push code review mirroring this repo's CI Toolu Code Review action (`falconiere/toolu-ghactions/code-review@v8`, with Jev assessment enabled), writing the toolu `push-review` state so the gate passes.

## Install

**Prerequisite:** the native `toolu` binary. See [docs/install.md](../../docs/install.md).

```
/plugin install toolu-review@toolu
```

Standalone, no dependencies.

## What it provides

- **`toolu-review:review` skill** — reviews the branch diff against what the CI Toolu Code Review action (`github-actions[bot]` verdict comment, optionally App-branded) flags, so the first-push verdict is clean instead of bouncing low/nit findings back as rework. It also records a clean `push-review` state, satisfying toolu's `push-review` gate.

Run `toolu review write-state --findings-count 0` after reviewing and committing the branch diff. On OpenCode the skill is `toolu-review-review`; the command records state in `<project>/.opencode/tmp/push-review/<branch>.json`. See [docs/opencode.md § Core workflows](../../docs/opencode.md#core-workflows).

Explicit — it does not auto-fire on edits. Run it before pushing a feature branch, or when `pr-babysit` needs a reviewer.
