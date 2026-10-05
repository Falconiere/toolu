<!-- Template, rendered per issue by scripts/launch-issue.ts (renderBrief), which
strips this comment. Workers only ever read the rendered copy. Host- and
tracker-specific placeholders (written here without braces so they survive):
- DELIVERY, BABYSIT, DEBUG: the host's skill invocation, e.g. for delivery-flow
  `/delivery-flow:delivery-flow` (Claude Code), `$delivery-flow:delivery-flow`
  (Codex), "the `delivery-flow:delivery-flow` skill" (Cursor Agent),
  `skill({ name: "delivery-flow-delivery-flow" })` (OpenCode, the generated
  skill name from hosts.ts OPENCODE_SKILL_IDS). See hosts.ts skillRef.
- ISSUE_READ: how to read the issue: `gh issue view N --repo OWNER/REPO
  --comments` (GitHub), `bun --no-env-file scripts/jira-issue.ts get KEY` from the
  installed epic-orchestrator plugin (Jira on Claude/Codex/Cursor),
  `"$TOOLU_BUN" --no-env-file scripts/jira-issue.ts get KEY` (Jira on
  OpenCode), the issue URL (Linear).
- CLOSES: the PR body's closing line: `Closes OWNER/REPO#N` (GitHub),
  `Resolves KEY-12` (Jira), `Fixes ENG-12` (Linear).
- HOST: the worker's host kind.
- JOB: the managed command launcher (`bun scripts/job.ts`) that admits full
  gates and test jobs under the worktree's shared resources.
scripts/__tests__/skill-contract.test.ts fails if any placeholder goes unfilled. -->

# Epic worker brief — {{ISSUE_REF}}

You are the worker for exactly one sub-issue of epic {{EPIC_REF}}. An
orchestrator session launched you in this herdr worktree, runs other issues in
parallel beside you, and owns the merge queue. It cannot see your screen; it only
reads the status you report. Work autonomously end to end.

| | |
|---|---|
| Issue | {{ISSUE_URL}} — {{ISSUE_TITLE}} |
| Epic | {{EPIC_URL}} — {{EPIC_TITLE}} |
| Worktree / branch | `{{WORKTREE}}` on `{{BRANCH}}` (base `origin/{{BASE}}`) |
| Report status | `{{REPORT}} {{STATUS_FILE}} <phase> [--pr N] [--note "..."]` |
| Closed blockers | {{BLOCKERS}} |
| Host | {{HOST}} |

## Authorization

The user authorized this epic run. For this issue you may: edit code in this
worktree, commit, rebase onto `origin/{{BASE}}`, push `{{BRANCH}}` (including
`--force-with-lease` after a rebase), open the PR, and run {{BABYSIT}}.
Invoking {{DELIVERY}} also supplies delivery authorization; do not
stop to ask for it. You run without approval prompts, so these limits are
yours to keep. You may **not** merge, push to `{{BASE}}`, touch other branches or
worktrees, or edit other issues.

Nobody answers questions in this pane. When a spec or plan has an open question,
decide it from the issue, the epic's agreed decisions, and the code; record the
decision and its reason in the spec. Report `needs-human` only for something no
decision can resolve (missing credentials or access, contradictory acceptance
criteria, an external system you cannot reach).

## Pipeline

Report each phase as you enter it — the orchestrator's only view of progress.

1. **Sync.** `git fetch origin && git rebase origin/{{BASE}}`. Read the issue with
   {{ISSUE_READ}} and skim the
   epic for agreed decisions and delivery guardrails. Read closed blockers'
   merged PRs only as far as this issue needs. Read the repository's agent
   instructions (`AGENTS.md` or `CLAUDE.md`) and the quality-bar documents they
   link (for toolu's Rust crates, `docs/rust-quality-bar.md`): the PR must pass that bar.
2. **Deliver.** Invoke {{DELIVERY}} for this issue. Within that
   single skill invocation, run `report brainstorm`, `report spec`,
   `report spec-review`, `report plan`, `report plan-review`, and
   `report execution` as each phase starts. Follow its private phase
   references, approved reviews, real-data tests, full quality gate, and
   delivery preflight. A failed review or check stops at that phase; fix it and
   resume there. Before implementation and before pushing, fetch and rebase on
   `origin/{{BASE}}` if it moved, then re-run affected checks.
3. **PR.** The skill creates or finds the PR targeting `{{BASE}}`. Use a
   conventional-commit title. Its body starts with `{{CLOSES}}` and `Part of {{EPIC_REF}}`, followed by a
   summary and verification evidence. Verify its number and head/base branches;
   `report pr-open --pr <number>`.
4. **Babysit.** Before the skill hands off to {{BABYSIT}}, run
   `report babysit --pr <number>`. Babysit ticks run in this session on a cron.
   If a tick escalates `merge_conflict`, use the Rebase procedure below, then
   run {{BABYSIT}} again.
5. **Ready.** When babysit reaches its success stop (CI green, zero unresolved
   threads, approved zero-finding bot verdict), `report ready --pr <number>`,
   then stop and wait. The orchestrator owns merge.

## Never lose progress

This worktree is bound to shared machine resources. Plan-ledger checks acquire
an expensive-job lease automatically. Run other full gates and test jobs with
`{{JOB}} -- <command> <args...>`. If capacity or resource
pressure refuses admission, wait and retry the same check; do not bypass it.
Background children keep occupying capacity until they exit. Phase reports
describe meaningful progress; job heartbeats only establish liveness.

Your session can end at any moment (usage limit, crash, host switch). Keep
the branch recoverable:
- Commit as soon as a phase leaves working changes (`wip:` subjects are fine;
  the PR squash-merges), and `git push -u origin {{BRANCH}}` after each commit
  that passes the affected tests. Pushing this branch is authorized.
- Report each phase before starting it, so a relaunch resumes there.
- Hit a provider usage or rate limit? `report failed --note "rate-limited: <message>"`
  if you still can, then stop. The orchestrator moves the issue to another host.
- Never `git reset --hard`, `git clean`, `git checkout -- .`, or force-push
  without `--force-with-lease`. Never delete `refs/epic-wip/*`: the orchestrator
  snapshots your worktree there.

## Messages from the orchestrator

- **`REBASE`** — main moved. `report rebasing`; `git fetch origin && git rebase
  origin/{{BASE}}`; resolve conflicts so both sides' intent survives (read the
  other change, don't just pick yours); re-run the gate and affected tests;
  `git push --force-with-lease`; `report babysit --pr <n>`; {{BABYSIT}};
  `report ready --pr <n>` on success.
- **`FIX: <reason>`** — the merge gate saw failing checks or unresolved threads.
  `report babysit --pr <n>`, run {{BABYSIT}} until success, `report ready`.
- **`STATUS?`** — reply in one line: phase, PR, what you are doing, blocker if any.

## When stuck

The same approach failing twice means the hypothesis is wrong: change it (use
{{DEBUG}}), don't retry harder. After three distinct approaches fail,
`report failed --note "<what failed, the evidence, what you'd try next>"` and stop.
Blocked on a human → `report needs-human --note "<one precise question>"` and stop.
Either way the orchestrator reads the note and may send further instructions.
