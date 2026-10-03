# Brainstorm — OpenCode: pr-babysit native fixer dispatch and lifecycle (#357)

**Date:** 2026-10-03   **Mode:** Delivery, Full path (cross-cutting, security, external writes)

## Capsule

- **Outcome:** On pinned OpenCode (`opencode-ai@1.18.34`), `/pr-babysit-babysit-ff6e5a3d` runs the shared strict-clearance workflow with an OpenCode controller, routes Fix items with `--host opencode`, and can dispatch an OpenCode fixer that edits, tests, commits and reports in an isolated worktree. Cancellation and fixer failures leave accurate state; the fixer cannot push or call `gh`.
- **Material defaults / non-goals:** Claude Code and Codex controllers and herdr fixers are unchanged. No plugin-side scheduler. No change to the tick, reply, resolve or record helpers' GitHub behavior.
- **Repository evidence:** the generated skill is the Codex text verbatim (`get_goal`, `--host codex`, a `../../hooks/dist` path that does not exist in `generated/`); `babysit-route-fix.js` rejects `--host opencode`; `hostKind` has no `opencode`; the npm package stages `hooks/dist/*.js` but not `skills/babysit/references/fixer-brief.md`, which `babysit-dispatch-fix.js start` reads; the capability matrix leaves "tick scheduling" open for OP-23.
- **Risk:** live behavior of `--agent` defined through `OPENCODE_CONFIG_CONTENT`, and bash permission pattern matching for `git -C <dir> push`, are only known after the pinned-host scenario runs. A controller turn that runs for hours grows context until OpenCode compacts it.
- **Handoff:** spec.

## Axes

### Continuation (controller)

OpenCode's plugin API has no cron or goal primitive (`docs/opencode-host-contract.md`, capability-matrix note owned by OP-23).

| Option | Verdict |
| --- | --- |
| A. In-turn bounded loop: run a cycle, `sleep` at most `backoff.waitSeconds` (≤ 60 s) in bash, run the next tick, until a stop; re-invoking the command resumes from the slot state | **Chosen.** Same shape as Codex's native bounded wait. Uses only bash and the shipped helpers. |
| B. toolu adapter watches `session.idle` and re-prompts through the SDK client | Rejected: puts pr-babysit-specific scheduling in the core adapter (against recorded decision 5a002d41: leaf behavior stays in the leaf's own startup bundle), needs session ownership, timers and an unprobed prompt path. |
| C. External cron/launchd loop | Rejected: not a usable native workflow. |

Jev (choice, with the adapter-rule evidence): A 1.00, B 0.00, C 0.00. Without that evidence A 0.51 / B 0.47, so the adapter rule decided it.

### Fixer transport

| Option | Verdict |
| --- | --- |
| A. herdr pane, `herdr agent start --kind opencode -- --auto --model …` | Rejected: herdr types args into a pane shell, so no environment can scope the fixer's permissions; not runnable without a herdr server, so no hermetic real-host evidence. |
| B. Detached `opencode run --format json --auto --agent pr-babysit-fixer --model … --variant …` subprocess in the slot worktree; the agent is defined in `OPENCODE_CONFIG_CONTENT` with `task` and remote-write bash patterns denied | **Chosen.** Real exit, log file and report; `--auto` approves asks but keeps explicit denies; runs against the scripted loopback provider. |

Jev: B 1.00.

Agent-scoped rules (`agent.pr-babysit-fixer.permission`) are layered over the user's own rules instead of replacing them, so a user `bash: "ask"` or `bash: "deny"` is not widened. `OPENCODE_PERMISSION` was rejected for that reason: the pinned binary deep-merges it into `config.permission`, so a string-valued user rule would be replaced by an object.

### Worktree

Herdr worktrees stay for plans that include a Claude Code, Codex or Cursor group. A plan whose groups are all `opencode` needs no pane, so the dispatcher creates a native `git worktree` next to the state file (`<state>.worktree`). On OpenCode that is under `<repo>/.opencode/tmp/pr-babysit/`, inside the project, so the controller's bash verification raises no `external_directory` prompt; Bun's test runner, ripgrep and TypeScript globs skip dot directories (checked: `bun test` ignores `.opencode/tmp/**`).

### Packaging

The npm package must ship `plugins/pr-babysit/skills/babysit/references/fixer-brief.md`, or `start` fails on npm installs.

## Open risks

- `--agent` from `OPENCODE_CONFIG_CONTENT` and the bash deny patterns are proven only by the pinned-host scenario; if either fails there, the design changes before merge.
- A long controller turn: mitigated by OpenCode auto-compaction and by resume-from-state on re-invocation.
