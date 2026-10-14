# pr-babysit

Babysit a PR for the current branch until every review thread, the review-bot
verdict, and CI are clear. `toolu babysit wait` checks the PR every 180 s
without a model turn and returns only when there is work: Claude runs it as a
background command, and Codex runs it inside an explicit durable goal. Fixes
run as Claude Code, Codex or Cursor Agent sessions in a herdr worktree, each
at the model and effort Jev picks for its complexity.

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
fixes run in-session as before. Jev routing needs `TYPESAFE_API_KEY`; without
it a task/severity heuristic picks the tier.

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
