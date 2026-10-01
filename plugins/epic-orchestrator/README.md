# epic-orchestrator

Drive a GitHub, Jira, or Linear epic to merged PRs. Builds the sub-issue
dependency graph, launches herdr worktrees with workers (delivery-flow from
brainstorm through PR and babysit), merges green PRs, and cleans up until the
epic is complete.

Works on **Claude Code**, **Codex**, **Cursor Agent**, and **OpenCode**, both
as the orchestrator and as workers. One run can mix hosts
(`--hosts claude:2,codex:2`): Jev scores each issue's complexity and a routing
table picks the model and effort per host.

## Install

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

Requires `delivery-flow` and its `toolu`, `toolu-review`, `pr-babysit`, and `brainstorm` dependencies, in every worker host. Runtime: `bun`, `gh`, and `herdr`
(with `HERDR_ENV=1` inside a herdr pane).

Optional: `jev` with `TYPESAFE_API_KEY` (complexity routing; without it tiers
come from a heuristic), the `jira` plugin (Jira epics), and `LINEAR_API_KEY`
(Linear epics: a personal key `lin_api_…` is sent bare, an OAuth token
`lin_oauth_…` as `Bearer`; `LINEAR_API_URL` overrides the endpoint).

### Claude Code

```text
/plugin install epic-orchestrator@toolu
```

### Codex

```bash
npx @toolu/plugins install delivery-flow epic-orchestrator --host codex
```

### Cursor Agent

Install the toolu marketplace plugin the same way as Claude Code (Cursor loads
the Claude-shaped plugin tree). Then invoke the `epic-orchestrator` skill or
the `/epic-orchestrator:epic` command.

### OpenCode

```bash
opencode plugin add @toolu/opencode
```

Enable the workflow plugins in `<project>/.opencode/toolu/plugins.json`:

```json
{ "version": 1, "enabled": ["delivery-flow", "epic-orchestrator"] }
```

Wire OpenCode to the generated surface under
`tools/toolu-opencode/generated/` (shipped in `@toolu/opencode`); see
[docs/opencode.md](../../docs/opencode.md). Set `TOOLU_PLUGIN_ROOT` to the
installed `plugins/epic-orchestrator` directory when running scripts from the
generated skill.

## What it provides

- **Skill `epic-orchestrator` and command `epic`** — orchestrate an epic (or
  `status` / `stop`). Workers run the toolu delivery chain then babysit; the
  orchestrator merges and advances the graph.
- **Bun CLIs** under `scripts/` — `epic-graph.ts`, `route.ts`,
  `launch-issue.ts`, `epic-watch.ts`, `merge-gate.ts`, `checkpoint.ts`,
  `epic-close.ts`, `finish-issue.ts`, `report.ts`. Tracker adapters live
  in `scripts/trackers/`.
- **Codex SessionStart** — warns when `delivery-flow` or its dependencies are missing.

## State directory

Override with `EPIC_STATE_HOME`. Otherwise:

| Host | Default |
|------|---------|
| Claude Code / Cursor Agent | `~/.claude/epics/<owner>-<repo>-<n>/` |
| Codex | `$CODEX_HOME/toolu/epics/…` (default `~/.codex/toolu/epics/…`) |
| OpenCode | `$TOOLU_OPENCODE_HOME/toolu/epics/…` (or `$OPENCODE_HOME/…`) |

## Trackers

| Tracker | Epic reference | Children | Blockers | Done |
|---------|----------------|----------|----------|------|
| GitHub | URL, `owner/repo#N`, `#N` | sub-issues, else task-list links | dependency API + "blocked by" text | close |
| Jira | browse URL, `jira:KEY-1`, `KEY-1` | `parent = KEY-1` (`"Epic Link"` on Server/DC) | "is blocked by" / "depends on" links | transition to Done |
| Linear | issue or project URL, `linear:ENG-1`, `ENG-1` | sub-issues or project issues | `blocks` relations | completed state |

Jira and Linear items map to a GitHub repo through a `repo:owner/name` label,
a `Repo: owner/name` description line, or `--repo` (default: current repo).

## Hosts and routing

| Host | Unattended flags | Model / effort | Resume |
|------|------------------|----------------|--------|
| Claude Code | `--dangerously-skip-permissions` | `--model` / `--effort` | `--continue` |
| Codex | `--dangerously-bypass-approvals-and-sandbox` | `--model` / `-c model_reasoning_effort=` | `resume --last` |
| Cursor Agent | `--yolo --trust --approve-mcps` | model id carries effort | `--continue` |
| OpenCode | `--auto` | `--model provider/model#variant` | `--continue` |

`--safe` keeps approval prompts on. Default tiers (`trivial`, `standard`,
`complex`, `critical`) map to Claude `sonnet` low → `opus` xhigh, Codex
`gpt-6-sol` low → xhigh, Cursor `composer-2.5` → `gpt-5.6-sol-xhigh`, and
OpenCode's configured model. Override per host and tier in `routing.json`
under the state root, or point `EPIC_ROUTING_FILE` at one.

## Merging

The merge gate verifies the PR (rebased on base, checks green, zero
unresolved threads, babysit cleared) and merges pinned to the verified head.
When only checks are pending it arms GitHub auto-merge
(`gh pr merge <n> --auto --squash --delete-branch --match-head-commit <sha>`),
and disarms it whenever the PR needs a rebase or fix. Repos with auto-merge
disabled fall back to the watcher's periodic recheck.

## Guardrails

- **Progress:** workers commit and push each phase; the watcher snapshots
  every active worktree to `refs/epic-wip/<key>` every 15 minutes and when an
  agent disappears; teardown snapshots first; relaunches resume the host's
  last session; one watcher per epic.
- **Rate limits:** retries with backoff that honor `Retry-After` and reset
  headers (GitHub, Jira, Linear); at most 4 concurrent graph fetches; no new
  launches below `EPIC_GH_CORE_FLOOR` (1000) / `EPIC_GH_GRAPHQL_FLOOR` (500);
  hosts that hit a provider usage limit cool down (`EPIC_HOST_COOLDOWN_MIN`,
  60) and their issues move to another host.
