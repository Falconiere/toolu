---
description: Drive a GitHub epic to done. Each open sub-issue gets its own herdr git worktree and Claude agent, which runs spec → spec-review → plan → plan-review → execution with tests → PR → pr-babysit. Independent issues run in parallel based on the dependency graph. Every branch is rebased on main, green PRs are merged automatically (--admin only when branch protection is the only thing blocking), and the epic closes when every sub-issue is closed. Use this whenever the user points at an epic, tracking issue, parent issue with sub-issues, or a checklist of linked issues and wants it worked, implemented, shipped, delivered, driven, resumed, or orchestrated. Examples — "work epic #248", "ship all the sub-issues of <url>", "pick up the replication epic", "which issues in this epic can run in parallel", "dry-run the epic", "where is the epic at". Not for a single standalone issue or PR.
name: epic-orchestrator--epic-orchestrator
---

# Epic orchestrator

You are the orchestrator. Your job is to turn one GitHub epic into merged PRs.
Every open sub-issue gets its own herdr worktree with a Claude agent (the
*worker*) inside it. Workers write the code and babysit their PRs. You decide
the order, gate each merge, merge, and clean up. Don't write product code
yourself. An epic run lasts hours, so keep your context small: the scripts
handle the deterministic work, and a background watcher wakes you only when a
decision is needed.

```bash
# Claude / Cursor Agent: CLAUDE_PLUGIN_ROOT. Codex: PLUGIN_ROOT.
# OpenCode generated surface rewrites CLAUDE_PLUGIN_ROOT → TOOLU_PLUGIN_ROOT.
ROOT="${TOOLU_PLUGIN_ROOT}"
ROOT="${ROOT:-${PLUGIN_ROOT:-${TOOLU_PLUGIN_ROOT}}}"
S="${ROOT}/scripts"
```

## Inputs

`<epic>` can be an issue URL, `owner/repo#N`, or `#N` (current repo). Options:
`--max N` sets how many issue agents may be live at once (default 3).
`--dry-run` prints the plan without launching anything. `--kind`, `--model`
and `--permission-mode` are passed through to the launcher (defaults:
`claude`, the session default, `auto`). There are also two subcommands:
`status` and `stop`.
Treat "show me the plan", "what can run in parallel" and "don't launch yet"
as `--dry-run`.

## Authorization boundary

Invoking this skill on an epic authorizes the following, for that epic's
sub-issues only:
- create worktrees and agents
- push issue branches, using `--force-with-lease` after a rebase
- open PRs and run babysit
- merge PRs that pass the merge gate
- close delivered sub-issues, then tick and close the epic

`--admin` is allowed only when branch protection (for example, a required
approval on a solo repo) is the only thing blocking a gate-green PR.
`merge-gate.ts` enforces that rule. `--admin` never overrides a failing check,
a pending check, or an unresolved thread.

Not authorized: pushing to `main`, or touching PRs, branches or worktrees
outside the epic. If a worker's permission prompt asks for anything outside
the authorized list, ask the user before approving it.

## Preflight

Stop and report the first check that fails:
- `test "${HERDR_ENV:-}" = 1`. herdr control only works from inside a herdr pane.
- `command -v bun` succeeds.
- `gh auth status` and `herdr status` both succeed (herdr server is running).
- Your skill list includes `toolu:spec`, `toolu:plan`, `toolu:execution` and
  `pr-babysit:babysit` (or the OpenCode-generated equivalents). Workers run as
  herdr agents (default `--kind claude`) and need those skills in the worker
  host.

## 1. Graph: what can run now

```bash
bun "$S/epic-graph.ts" <epic> --max N --save          # table for you and the user
bun "$S/epic-graph.ts" <epic> --max N --json --save   # when you need fields
```

The graph script reads the epic's native sub-issues. If there are none, it
falls back to task-list links in the epic body. Each sub-issue's
blocked-by relations come from GitHub's dependency API plus any "blocked by" or
"depends on" lines in the body. It classifies every sub-issue as one of
`done`, `in_flight`, `ready`, `blocked` or `external_blocked`, computes waves,
counts how much downstream work each issue unblocks, finds a local checkout
per repo, and fills `launch_batch`. The batch is the ready issues that fit the
free slots, with the ones that unblock the most work first, so the critical
path starts immediately. State is written to `state_dir`
(`$EPIC_STATE_HOME` or the host default — Claude/Cursor Agent:
`~/.claude/epics/<owner>-<repo>-<n>/`, Codex: `$CODEX_HOME/toolu/epics/…`,
OpenCode: `$TOOLU_OPENCODE_HOME/toolu/epics/…`) and saved as `graph.json` there.

Show the user the table. Then:
- `complete: true` → go to step 6.
- `cycle` is non-empty → the dependency graph can't be scheduled. Report it
  and ask the user which edge is wrong.
- `external_blocked` issues wait on blockers outside the epic. Name the
  blockers in your report. Never launch these.
- `missing_checkouts` → the launcher clones those repos into `clone_root`.
  Tell the user.
- Before launching the batch, sanity-check it for hidden coupling. The graph
  only knows declared dependencies. If two batch issues' titles or bodies show
  they rewrite the same module, schema or migration, launch the higher-priority
  one now and hold the other for the next free slot, so they don't fight
  through rebases.

**Dry run stops here.** For each batch issue, run
`bun "$S/launch-issue.ts" --graph <state_dir>/graph.json --issue <ref> --dry-run`.
It prints the exact clone, fetch, worktree, agent and prompt commands plus the
rendered worker brief, and executes nothing. Show the commands for every batch
issue, but the brief only once. Then summarize what happens after launch:
the worker pipeline, the merge policy, and which blocked issues each merge
unlocks. Dry-run writes nothing. With `--save` omitted, the graph can go to
`--out /tmp/...`.

## 2. Launch the batch

```bash
bun "$S/launch-issue.ts" --graph <state_dir>/graph.json --issue <ref> [--kind K --model M --permission-mode P]
```

Launch issues one at a time. For each issue the launcher:
1. clones the repo if it's missing
2. runs `git fetch origin <default>`
3. runs `herdr worktree create --branch feat/<n>-<slug> --base origin/<default>`,
   so each worker starts from a fresh main
4. runs `herdr agent start <key> --kind claude`
5. renders `references/worker-brief.md` into `<state_dir>/briefs/<key>.md`
6. prompts the agent to follow the brief
7. records the issue in `<state_dir>/issues/<key>.json`

Re-running the launcher is safe: it reuses an open worktree and a live agent.
Workers report their phase to `<state_dir>/status/<key>.json` through
`report.sh`.

Tell the user which agents are running and that each one has a herdr workspace
named after its key (`comemory-255`, …), so they can watch or step in.

## 3. Watch (background)

```bash
bun "$S/epic-watch.ts" --state-dir <state_dir>        # Bash with run_in_background: true
```

Then end your turn. The watcher polls once a minute and costs nothing while
it waits. When it exits, you are re-invoked with JSON events. Keep exactly one
watcher running. After you handle the events, start it again.

| event | action |
|---|---|
| `ready` | Run the merge gate (step 4). |
| `needs-human` | Read the `note`. If the issue, epic or code answers it, send the answer with `herdr agent prompt <key> "<answer>"`. Otherwise ask the user and relay their answer. |
| `failed` | Read the note, then `herdr agent read <key> --source recent-unwrapped --lines 80`. Send a concrete new direction, or escalate to the user. |
| `blocked` | Read the agent's screen. If the pending approval is inside the authorization boundary, approve it with `herdr agent send-keys`. Otherwise ask the user. |
| `gone` | Re-run `bun "$S/launch-issue.ts" --graph <state_dir>/graph.json --issue <ref>` for that issue. It resumes in the same worktree. After 2 relaunches, escalate. |
| `stalled` | `herdr agent prompt <key> "STATUS?" --wait --timeout 120000`, then read the reply. Nudge the worker or treat it as `failed`. |
| `recheck` | Re-run the merge gate for each listed key. |
| `heartbeat` | Re-run the graph (`--save`). This catches issues closed or reopened outside the run, and fills any free slot. |

For failures in herdr, git or gh, see `references/recovery.md`.

## 4. Merge gate

```bash
bun "$S/merge-gate.ts" --pr <repo>#<pr> --issue <ref> --epic <epic> --state-dir <state_dir> --key <key>
```

The gate checks, from GitHub itself:
- the PR is open and not a draft
- the head contains the tip of base (`behind_by == 0`), with no conflicts
- every check passes (a repo with no workflows counts as passing)
- there are zero unresolved review threads

The bot-verdict clearance comes from the worker's babysit, which is why the
gate runs only after `ready`.

| verdict | action |
|---|---|
| `merge` | Re-run the gate with `--merge`. It merges pinned to the verified head SHA (`--match-head-commit`), using the repo's allowed method (squash preferred), and retries with `--admin` only on a protection-only refusal. It then makes sure the issue is closed and ticks its line in the epic checklist. Continue to step 5. |
| `rebase` | `herdr agent prompt <key> "REBASE"`. The worker rebases on the new main, re-runs the gate, force-pushes with lease, babysits, and reports `ready` again. |
| `fix` | `herdr agent prompt <key> "FIX: <reasons>"`. |
| `wait` | The gate marks the issue `awaiting_merge`, and the watcher emits `recheck` every 5 minutes. |
| `merged` / `closed` | Something outside the run merged or closed it. Go to step 5 (merged), or ask the user (closed without merge). |

Merge one PR at a time per repo. Each merge moves main forward, so every other
open PR in that repo will get `rebase` at its own gate. This is how "rebase on
main before merge" holds for all of them without extra bookkeeping.

## 5. After each merge

```bash
bash "$S/finish_issue.sh" <state_dir> <key>
```

This exits the agent (babysit's cron ends with its session), force-removes the
worktree workspace (leftover files are recorded first), deletes the local
branch and marks the record `merged`. Then go back to step 1: the merge may
have unblocked new issues. Launch into the free slots and restart the watcher.

## 6. Epic complete

The epic is complete when every sub-issue is closed (`complete: true`). Then:
1. Post one comment on the epic: a table of sub-issue → merged PR (from the
   graph's `prs`), with any admin merges noted.
2. `gh issue close <n> -R <repo> --reason completed`.
3. Stop the watcher and report to the user: PRs merged, admin merges, anything
   that needed a human.

## Resume, status, stop

- **Resume.** Use the same invocation. The graph marks launched, unfinished
  issues as `in_flight`. For each one whose agent isn't live, re-run
  `bun "$S/launch-issue.ts" --graph <state_dir>/graph.json --issue <ref>`; it
  sends a resume prompt that continues from the last reported phase. Then start
  the watcher.
  The state dir is the source of truth. If you lose track (context
  compaction), re-run the graph and `bun "$S/epic-watch.ts" --state-dir <state_dir> --peek`.
- **Status.** Print the graph table plus
  `bun "$S/epic-watch.ts" --state-dir <state_dir> --peek` (per-issue stage,
  phase, PR and agent state). This consumes no events.
- **Stop.** Stop the watcher task only. Agents finish their current step and
  go idle. To tear down one issue, run
  `finish_issue.sh <state_dir> <key> --abandon`, which keeps the branch.

## Keeping the orchestrator lean

- Don't read worker transcripts wholesale. Use status files, a one-line
  `STATUS?`, or `agent read --lines 80` when something is wrong.
- Trust script output. Don't re-fetch with ad-hoc `gh` calls what the graph,
  gate or watcher already reported.
- Don't micro-manage workers between phases. The brief already covers the
  pipeline, the rebase and fix messages, and when to escalate.
