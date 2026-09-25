# delivery-flow

One delivery skill for Claude Code and Codex. It runs brainstorm, spec, spec review, plan, plan review, execution with real-data tests, PR delivery, and `pr-babysit`. The phase procedures are private references inside the plugin.

Install `delivery-flow@toolu` from the toolu marketplace. Claude Code installs its `toolu`, `toolu-review`, and `pr-babysit` dependencies automatically. For Codex, use `npx @toolu/plugins install delivery-flow --host codex` to install the dependency set, or add those three plugins explicitly with `codex plugin add`. Invoke `/delivery-flow:delivery-flow` in Claude Code or `$delivery-flow:delivery-flow` in Codex. Invocation authorizes commit, push, PR creation, and babysit after all checks pass.

Every phase runs even for small fixes; concise artifacts are enough when the task is small. Failed reviews and checks stop the flow at that phase. Resume there after fixing the finding, and refresh downstream evidence. Delivery stops with a named blocker if GitHub auth, a non-default branch, an installed dependency, or a required gate is unavailable.
