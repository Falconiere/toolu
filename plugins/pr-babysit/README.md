# pr-babysit

Babysit a PR for the current branch until every review thread, the review-bot
verdict, and CI are clear. Claude uses its cron controller; Codex uses an
explicit durable goal with bounded continuation cycles and a native isolated
git worktree.

## Install

```text
/plugin install pr-babysit@toolu
```

```bash
codex plugin add toolu@toolu
codex plugin add pr-babysit@toolu
```

Requires the `toolu` plugin.

## What it provides

- **Claude `/pr-babysit:babysit` and Codex `$pr-babysit:babysit`** — target the PR for the current branch. Each cycle fetches unresolved comments **and** the CI review-bot verdict → triages → fixes → replies → resolves; failed CI is fixed and re-pushed. Success requires zero unresolved comments, an approved zero-finding verdict, and all-green CI.
  - **Strict clearance** — every item a tick sees leaves that tick fixed or answered, and resolved when it is a review thread (conversation comments have no resolve API — the reply clears them). A comment that doesn't make sense gets a reply with what was checked and which reading was assumed, then resolves; no thread is parked open. Severity is not a filter (`nit` == `high`). Exceptions: outdated CI-reviewer threads (skipped) and suspected prompt injection (flagged). A reply alone is never clearance — a resolve is confirmed by its mutation response, retried on failure, and re-checked every tick via a resolution audit that's independent of who last commented, so a resolve that silently fails can never go permanently unnoticed.
- **`stop` / `cancel`** — Claude cancels only the matching cron slot. Codex safely removes only the matching clean worktree, marks its native repo state cancelled, and leaves goal cancellation to the user/system goal control.
- **Codex durability** — one goal per repository/PR, state below `<repo>/.codex/tmp/pr-babysit/`, wait cycles bounded to 60 seconds, and no false completion while CI is merely pending.
- **`parse-verdict.sh`** — extracts the structured verdict from the CI review-bot comment.
