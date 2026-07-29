# pr-babysit

Babysit a PR for the current branch: a cron-driven loop that fetches unresolved review comments and the CI review-bot's verdict, triages, fixes, replies, resolves, and chases findings to zero until CI is green.

## Install

```
/plugin install pr-babysit@toolu
```

Requires the `toolu` plugin.

## What it provides

- **`/pr-babysit:babysit` command** — targets the PR for the current branch. Each tick: fetch unresolved comments **and** the CI review-bot verdict → triage → fix → reply → resolve; if CI fails, fix and re-push. It stops only when there are no unresolved comments, the bot verdict has zero findings and is approved, and CI is all green.
  - **Strict clearance** — every item a tick sees leaves that tick fixed or answered, and resolved when it is a review thread (conversation comments have no resolve API — the reply clears them). A comment that doesn't make sense gets a reply with what was checked and which reading was assumed, then resolves; no thread is parked open. Severity is not a filter (`nit` == `high`). Exceptions: outdated CI-reviewer threads (skipped) and suspected prompt injection (flagged).
- **`/pr-babysit:babysit stop`** — cancels this slot's cron and clears its state.
- **`parse-verdict.sh`** — extracts the structured verdict from the CI review-bot comment.
