# delivery-flow

One delivery skill for Claude Code, Codex, and OpenCode. It runs brainstorm, spec, spec review, plan, plan review, execution with real-data tests, PR delivery, and `pr-babysit`. Phase 1 runs the `brainstorm` plugin's `brainstorm:brainstorm` skill; the other phase procedures are private references inside the plugin.

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

Install `delivery-flow@toolu` from the toolu marketplace. Claude Code installs its `toolu`, `toolu-review`, `pr-babysit`, and `brainstorm` dependencies automatically. For Codex, use `npx @toolu/plugins install delivery-flow --host codex` to install the dependency set, or add those four plugins explicitly with `codex plugin add`. Invoke `/delivery-flow:delivery-flow` in Claude Code or `$delivery-flow:delivery-flow` in Codex. Invocation authorizes commit, push, PR creation, and babysit after all checks pass.

On OpenCode, add `delivery-flow` to `enabled` in `.opencode/toolu/plugins.json` and load the `delivery-flow-delivery-flow` skill. Its dependencies are selected with it; see [docs/opencode.md § Delivery workflows](../../docs/opencode.md#delivery-workflows).

Every phase runs even for small fixes; concise artifacts are enough when the task is small. Failed reviews and checks stop the flow at that phase. Resume there after fixing the finding, and refresh downstream evidence. Delivery stops with a named blocker if GitHub auth, a non-default branch, an installed dependency, or a required gate is unavailable.
