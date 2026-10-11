# pr-babysit

Babysit a PR for the current branch until every review thread, the review-bot
verdict, and CI are clear. `toolu babysit wait` checks the PR every 180 s
without a model turn and returns only when there is work: Claude runs it as a
background command, Codex inside an explicit durable goal, OpenCode in the
invoking turn. Fixes run as Claude Code, Codex or Cursor Agent
sessions in a herdr worktree, or as `opencode run` fixers, each at the model
and effort Jev picks for its complexity.

## Install

**Prerequisite:** the native `toolu` binary. See [docs/install.md](../../docs/install.md).

```text
/plugin install pr-babysit@toolu
```

```bash
codex plugin add toolu@toolu
codex plugin add pr-babysit@toolu
```

Requires the `toolu` plugin. Multi-host fixes need [herdr](https://herdr.dev)
and the host CLIs you list (`claude`, `codex`, `cursor-agent`); without herdr,
fixes run in-session as before. OpenCode fixers need only the `opencode` CLI. Jev
routing needs `TYPESAFE_API_KEY` and the `jev` plugin; without it a task/severity
heuristic picks the tier.

OpenCode: add `pr-babysit` to `.opencode/toolu/plugins.json` (see [OpenCode install](../../docs/opencode.md)); run the `pr-babysit-babysit-ff6e5a3d` command or load the `pr-babysit-babysit-73c340c6` skill.

- **Controller.** OpenCode has no goal, so the invoking turn is the controller: it runs `toolu babysit wait` (at most 55 s per call) again and again until the Success or Escalation stop. If the turn ends early, invoking the command again resumes from `<repo>/.opencode/tmp/pr-babysit/<slot>.json`. `toolu` reads no project `.env`, so a stray `GH_TOKEN` there never reaches GitHub.
- **Fixers.** `--host opencode` routing reads `prBabysit` from your OpenCode config (`routing.opencode` rows are `provider/model` plus an OpenCode `--variant`; with none, your default model). An OpenCode fixer is a detached `opencode run` as the `pr-babysit-fixer` agent in a native worktree beside the state file: it may edit, test and commit. Its agent denies `task` and the plain `git push` and `gh` forms on top of your own permission rules, and whatever form a command takes it has no GitHub token or `gh` login and git may use only local (`file`) remotes, so https and ssh pushes are refused. herdr is needed only for Claude Code, Codex or Cursor fixers. Inline fixes go to `task` with `toolu-quick-task`, `toolu-implementer` or `toolu-architect`.
- **Cancel.** `stop` ends a running fixer's whole process group, removes the clean worktrees and marks the state `cancelled`; the state file stays as the record.

## Authorization and delivery handoff

The no-argument interface is unchanged. A user can invoke babysit directly, or
`execution` can invoke it after a verified execution handoff; that handoff is
sufficient authorization to create the durable goal and begin delivery. Before
an automatic handoff, stop before delivery and report the exact missing
prerequisite: GitHub authentication is unavailable, the current branch is the
repository default branch, or the optional `pr-babysit` plugin is unavailable.

## What it provides

- **Claude `/pr-babysit:babysit` and Codex `$pr-babysit:babysit`** — target the PR for the current branch. Each cycle fetches unresolved comments **and** the CI review-bot verdict → triages → fixes → replies → resolves; failed CI is fixed and re-pushed. Success requires zero unresolved comments, an approved zero-finding verdict, and all-green CI.
  - **Strict clearance** — every item a tick sees leaves that tick fixed or answered, and resolved when it is a review thread (conversation comments have no resolve API — the reply clears them). A comment that doesn't make sense gets a reply with what was checked and which reading was assumed, then resolves; no thread is parked open. Severity is not a filter (`nit` == `high`). Exceptions: outdated CI-reviewer threads (skipped) and suspected prompt injection (flagged). A reply alone is never clearance — a resolve is confirmed by its mutation response, retried on failure, and re-checked every tick via a resolution audit that's independent of who last commented, so a resolve that silently fails can never go permanently unnoticed.
- **`stop` / `cancel`** — Claude stops only the matching slot's `wait` and runs `toolu babysit stop`. Codex safely removes only the matching clean worktree, marks its native repo state cancelled, and leaves goal cancellation to the user/system goal control.
- **Codex durability** — one goal per repository/PR, state below `<repo>/.codex/tmp/pr-babysit/`, `wait` calls bounded to 55 seconds, and no false completion while CI is merely pending.
- **Tick and wait (`toolu babysit`, every host)** — `toolu babysit tick` runs one check: lock the slot, collect the PR (paginated threads, comments, reviews, CI rollup, bot verdict), reduce it against the slot state, persist atomically, and print a result with a `decision` (`keep_going` / `success` / `escalate`), reasons, `attention` (what needs the agent, and whether it is new), and the precomputed actionable / stale-unresolved thread lists. `toolu babysit wait` runs those checks every 180 s on a fixed schedule and returns only on success, escalation, attention or an active fixer. `toolu babysit collect` (network) and `toolu babysit reduce-state` (pure) are its halves, so every rule is tested against captured real PRs. The agent trusts the result and keeps only the judgment: triage, fixes, reply wording.
- **Epic workers** — inside an epic, the epic engine runs the same tick every 180 s and prompts the worker only when there is new work, so a worker never polls.
- **Multi-host fixers** — after triage, `toolu babysit route-fix` scores each Fix item with Jev (the same complexity tiers as epic-orchestrator), groups items by tier and routes each group to a host, model and effort from the `prBabysit` block of `toolu.config.json` ([docs/config.md](../../docs/config.md)): pool (`hosts`), per-tier preference (`prefer`), per-host table (`routing`), and `unattended`. `toolu babysit dispatch-fix` runs the groups one at a time as herdr agents in the slot's herdr worktree (`pr-babysit/<slot>`); a fixer only edits, tests and commits, and the controller verifies, pushes, replies and resolves. A host that hits a usage limit cools down for an hour; re-routing the failed group sends it to another host.
- **Write side** — `toolu babysit reply` (idempotent per reviewer comment), `toolu babysit resolve` (confirmed from the mutation response, retried), `toolu babysit record` (round outcome, injection skip, terminal status), `toolu babysit stop` (cleanup and cancel). Contract: `skills/babysit/references/helper.md`.
- **`toolu babysit parse-verdict`** — extracts the structured verdict from the CI review-bot comment.
