# pr-babysit in Rust — Design

**Date:** 2026-10-11   **Status:** Approved   **Author:** epic worker (#433)   **Topic:** port pr-babysit to `toolu babysit`, fixed 180 s checks, LLM only on work

## Problem

pr-babysit is ≈4k lines of Bun bundles. Every tick is an LLM turn: a Claude
cron or a Codex/OpenCode loop wakes the agent, the agent runs `babysit-tick.js`
and usually learns nothing changed. That costs tokens and wall time on every
PR of every epic worker. The epic engine (#447/#460) already calls a
`BabysitTick` trait every few minutes, but `toolu_pr_babysit::Tick` returns
`NotPorted`, so workers still poll by hand. Bun bundles also block #425/#440
(retire Bun from hooks).

## Non-Goals

1. No change to clearance policy: strict clearance, slot isolation,
   injection-suspect handling, recurrence and fix-attempt caps, and the
   "never re-fetch what the result reports" rule are kept as written.
2. No engine watch for PRs outside a registered epic (#447 non-goal 3):
   `toolu babysit wait` ticks in its own process.
3. No adaptive backoff. The result's `backoff` and the state's `idleStreak`,
   `currentInterval`, `waitSeconds` and `cronName` are dropped.
4. No new third-party dependency (SHA-1 for finding keys is in-crate).
5. No change to fixer routing tiers, agent flags, the fixer brief's rules or
   the OpenCode fixer sandbox (env, denies, `GIT_ALLOW_PROTOCOL=file`).
6. Cursor is not a babysit controller host (it stays a fixer host only).

## Architecture

`crates/pr-babysit` (package `toolu-pr-babysit`, namespace `babysit`) becomes
the implementation; `plugins/pr-babysit/hooks/src` and `hooks/dist` are deleted.

- **Collect** (`collect/`): `toolu_github::Client` instead of `gh`. One GraphQL
  query reads the PR fields and the head commit's `statusCheckRollup.contexts`
  (paginated), review threads and overflowing thread comments are paginated
  GraphQL as today, issue comments and reviews are paginated REST
  (`per_page=<page-size>&page=<k>`, stop on a short page). The head is read
  before and after the fan-out; a move recollects once, then `head_moved`.
  The snapshot keeps version 1 and its shape, so `reduce` and the goldens are
  unchanged. Verbs use `Config::one_shot` (`PB_GH_ATTEMPTS`, `PB_GH_BACKOFF`,
  `PB_GH_TIMEOUT`; `--timeout` overrides the per-call deadline); the engine's
  `Tick` uses `Config::scheduled`. GraphQL points are summed into the tick's
  `GraphQlUsage`.
- **Reduce / verdict / record / writes**: line-for-line ports of `reduce.ts`,
  `verdict.ts`, `babysit-record.ts`, `writes.ts`, minus backoff. The SHA-1
  for `path:line:<8 hex>` keys is a small in-crate function checked against
  FIPS 180 vectors and the existing `parse-verdict.golden.json`.
- **Schedule**: the state gains `schedule.nextCheckAt` (Unix seconds). A tick
  run by `wait` or the engine sets it to the first whole 180 s slot after now,
  anchored on the first check (`anchor + k·180`). Ad-hoc `toolu babysit tick`
  calls do not move it.
- **Attention**: every result gains `attention`, computed by the reducer from
  the same data: the work an agent must do now, a fingerprint of that work, and
  whether it differs from the previous tick's (`state.attention.fingerprint`).
- **Wait** (`wait.rs`): loop { if due → tick; stop on `success`, `escalate`
  or `attention.needed`; sleep until min(next slot, deadline) }; at the
  deadline print `{"version":1,"state":"waiting",…}`. Generic over a `Clock`
  and a `Check` so the 6-hour test runs on a virtual clock against the real
  loopback GitHub fixture.
- **Engine link**: `TickDecision` gains `Act`, and `TickReport` gains
  `prompt: Option<String>`. `toolu_pr_babysit::Tick` returns `Success` on
  success, `Act` with prompt text when `attention.new`, else `KeepGoing`.
  `epic-orchestrator`'s `babysit::next` maps `Act` to `Next::Prompt(text)`;
  `github_engine::note_babysit` journals `kind: "prompt", name: "babysit"`
  with the text and emits a new `Step::Prompt { key, text }`, which
  `server.rs` sends to the worker through herdr `agent.prompt` (or the
  `prompt` script action, body `{"key","text"}`, when scripts are
  configured). The stall nudge's `STATUS?` moves onto the same step, so one
  path sends every prompt. The engine's PR watch already runs every fixed
  `watch::CHECK_MS` (180 s). De-duplication lives in the tick's state file,
  so an engine restart never re-prompts the same work.
- **Route-fix**: Jev scoring calls `toolu_jev_client::Jev::ask` in process;
  `--jev-answers-in` replays a captured answer map; no subprocess.
- **Dispatch-fix**: port of `fixer-dispatch.ts`/`fixer-process.ts`. herdr,
  git and ps run through `toolu_runtime::process::run`; the detached
  `opencode run` fixer through a new `toolu_runtime::process::spawn_logged`
  (own process group, stdin null, stdout+stderr appended to the log, returns the
  pid); a new `group::signal_pid` signals one pid. The fixer brief template stays
  `plugins/pr-babysit/skills/babysit/references/fixer-brief.md`, embedded with
  `include_str!`; its report lines become `toolu babysit fixer-report …`.
- **Hooks**: two native SessionStart entries (`check-toolu`, the Codex core
  dependency notice through `toolu_runtime::startup::codex_dependency_notice`,
  and `check-binary`, the native `toolu` advisory) dispatched in
  `crates/cli/src/hook/plugins.rs`; `hooks.json` from `cargo xtask print-hook`.
- **Tests and fixtures**: goldens move to `fixtures/pr-babysit/`; the TS
  suites under `hooks/src/__tests__` and `scripts/__tests__` become Rust tests
  in `crates/pr-babysit/src/**/tests/`; recorded `gh` replies become loopback
  HTTPS routes (`toolu-http-test-support`). The herdr smoke
  (`tooling/src/pr-babysit-herdr-smoke.ts`) and the OpenCode babysit scenarios
  call `toolu babysit …`.

Reused: `toolu_github`, `toolu_jev_client`, `toolu_runtime::{process, atomic,
startup, env, cli}`, `toolu_state::time::{iso_seconds, parse_iso}`.

## Interfaces / Schema

```
toolu babysit tick   --repo <o/r> --pr <n> --state-file <p> [--snapshot-out <p>] [--snapshot-in <p>] [--page-size N] [--timeout S] [--now <iso>]
toolu babysit wait   --repo <o/r> --pr <n> --state-file <p> [--max-seconds N] [--page-size N] [--timeout S]
toolu babysit collect      --repo <o/r> --pr <n> [--out <p>] [--page-size N] [--timeout S]
toolu babysit reduce-state --snapshot <p> [--state <p>] --now <iso> [--state-out <p>] [--result-out <p>] [--state-path <p>] [--snapshot-path <p>]   (pure: prints {state,result})
toolu babysit parse-verdict                                                     (comment body on stdin)
toolu babysit record flag-injection --state-file <p> --thread <id>
toolu babysit record round  --state-file <p> --had-rejection true|false [--fix-pushed]
toolu babysit record status --state-file <p> --status complete|escalated|cancelled
toolu babysit reply   --state-file <p> --kind thread|conversation|review [--thread <id> --root-comment <n> --in-reply-to <n> | --comment-id <n> | --review-id <n>] --body-file <p>
toolu babysit resolve --state-file <p> --thread <id>
toolu babysit route-fix --items <p> --host claude|codex|opencode [--state-file <p>] [--raise <id>]... [--no-jev] [--jev-answers-in <p>] [--now <iso>]
toolu babysit dispatch-fix start   --state-file <p> --plan <p> --items <p> --repo-root <d> --branch <b> --base <b> [--dry-run]
toolu babysit dispatch-fix wait    --state-file <p> [--timeout-seconds N]
toolu babysit dispatch-fix cleanup --state-file <p> [--dry-run]
toolu babysit fixer-report <report-file> done|failed [--note <text>]
toolu babysit stop --state-file <p>        (dispatch-fix cleanup, then record status cancelled)
toolu babysit hook check-toolu|check-binary --event SessionStart --plugin-root <d>
```

`reply --body-file` is any caller-chosen path; the workflow writes
`<state stem>.reply-<n>.md` with a counter, never a thread-id-derived name
(two thread ids may differ only by case on case-insensitive APFS).

State-file paths (one slot per repository/PR, `<slot>` = `<owner>-<repo>-<n>`
lowercased): Claude `/tmp/pr-babysit-<slot>.json`; Codex
`<repo>/.codex/tmp/pr-babysit/<slot>.json`; OpenCode
`<repo>/.opencode/tmp/pr-babysit/<slot>.json`; epic engine
`<epic root>/babysit/<owner>__<repo>-<n>.json`. The snapshot is
`<stem>.snapshot.json` and the last result `<stem>.result.json`.

`wait --max-seconds` default by `--host`/detected host: `claude` 1800 (a
background Bash call), `codex` 55, `opencode` 55, any other 55.

Result (version 1) = today's contract minus `backoff`, plus:

```json
"attention": {"needed": true, "new": true, "codes": ["threads_actionable"],
              "fingerprint": "<40 hex>|null"},
"nextCheckAt": "2026-10-11T12:03:00Z|null",
"resultPath": "/tmp/pr-babysit-o-r-1.result.json"
```

`attention.codes` (closed, in this order): `escalate`, `ci_failed`,
`threads_actionable`, `threads_stale_unresolved`, `conversation_actionable`,
`reviews_actionable`, `review_changes` (a new `changes` verdict run),
`provider_error` (a new provider-error run), `fixer_done` (the slot's fixer
record is `done`/`failed`, waiting for its round). `needed` = codes non-empty;
`fingerprint` hashes the escalation codes, head SHA, item ids (thread
`id@inReplyTo`, conversation/review ids) and the bot run id; `new` =
`needed` and the fingerprint differs from the previous tick's. A fixer that
is running or blocked is never attention (the agent is in
`dispatch-fix wait`), and `wait` returns at once with that tick's result when a
fixer is active so the agent goes back to `dispatch-fix wait`.

Waiting document: `{"version":1,"state":"waiting","slot":…,"nextCheckAt":…,
"statePath":…,"resultPath":…}`.

State (version 2) drops `cronName`, `idleStreak`, `currentInterval`,
`waitSeconds` (ignored when read), adds `schedule: {anchor, nextCheckAt}` and
`attention: {fingerprint}`.

Exit codes (`toolu_protocol::exit`): `0` result; `64` usage; `69`
`gh_unavailable` (no token) or `herdr_unavailable`; `75` `locked`,
`api_error`, `head_moved`, `resolve_unconfirmed`; `1` every other structured
error (`duplicate_reply` included). The error document stays
`{"version":1,"errors":[{"code","message",…}]}` on stdout, and the tick still
stamps `pr.lastError`.

Engine (`toolu_engine::babysit`): `TickDecision::{KeepGoing, Success, Escalate,
Act}`; `TickReport { decision, result, graphql, prompt: Option<String> }`.
Prompt text (no reviewer prose, only ids and locations):

```
pr-babysit: <o/r>#<n> needs you (<codes>). Tick result: <resultPath>.
- thread <id> <path>:<line> (<authorClass>)        (≤ 10 items, then "… and N more")
Act with toolu babysit reply|resolve|record --state-file <statePath>; push fixes; do not re-fetch.
```

## Failure modes and edge cases

- **No token / `gh` absent**: `gh_unavailable`, exit 69, state untouched except
  `pr.lastError`. Engine: `LinkError::Failed` → attention item (unchanged path).
- **Rate limit**: verbs retry per `PB_GH_*`, then `api_error` (class
  `rate_limited`), exit 75. Engine: `LinkError::RateLimited` with the reset.
- **GraphQL `errors[]`, 5xx, decode error**: `api_error` with `source`,
  `class`, `lastMessage`; never a partial snapshot or state.
- **Head moves twice**: `head_moved`, exit 75, no state write.
- **Lock held**: `locked` with `pid`/`since`, exit 75; stale locks (dead pid,
  > `PB_LOCK_STALE_SECONDS`, incomplete > 1 s) are broken as today.
- **Malformed or foreign state**: `state_malformed` / `slot_mismatch`, exit 1,
  file untouched. Old states carrying the dropped backoff fields load.
- **Empty rollup**: `ci.status = pending`. **Closed/merged PR**: escalate.
- **`wait` with a due tick that fails**: prints the error document and exits
  with its code (no silent retry); a locked slot (another controller) is exit
  75 at once. `--max-seconds 0` runs at most one due tick and returns.
- **Clock skew / sleep overshoot**: the next slot is recomputed from the anchor,
  so a late wake runs one check, never a burst of missed ones.
- **Engine restart**: the fingerprint in the state file prevents a second prompt
  for the same work; a deleted state file re-prompts once.
- **Detached OpenCode fixer**: survives the tick and the CLI process (own
  process group, null stdin); `cleanup` TERMs then KILLs the group after 10 s;
  a reused pid (start time mismatch) is never signalled.
- **Concurrent reply**: the slot lock serialises writes; `duplicate_reply`
  exit 1 posts nothing.

## Acceptance criteria

- **AC-1:** The pr-babysit test suites (reduce, verdict, collect, tick, record,
  writes, route, dispatch native and OpenCode, check hooks) pass as Rust tests
  against `toolu babysit` using the recorded goldens, and
  `tooling/src/pr-babysit-herdr-smoke.ts` drives `toolu babysit` (tick,
  route-fix, dispatch-fix start/wait/cleanup) through a live herdr server.
- **AC-2:** A scripted epic whose PR fixture is green with an approved,
  zero-finding verdict reaches `ready` with zero `prompt` journal records between
  the `babysit` phase and the merge-queue record.
- **AC-3:** A scripted epic whose PR fixture has one bot finding thread produces
  exactly one `prompt`/`babysit` journal record whose text names that thread and
  path:line across repeated ticks; after the fixture's next verdict (approved,
  thread resolved) the epic reaches `ready` with no further prompt.
- **AC-4:** `wait` returns `{"state":"waiting"}` after exactly the per-host
  default (Claude 1800 s, Codex 55 s, OpenCode 55 s) on a virtual clock for a
  pending PR fixture; the binary with `--max-seconds 1` prints the waiting
  document.
- **AC-5:** With a pending fixture whose verdict turns approved and green at a
  virtual time t, `wait` returns the success result at the first check ≥ t,
  which is ≤ t + 180 s.
- **AC-6:** On a 6-hour virtual clock against a red-CI PR fixture, the epic
  engine's babysit ticks run exactly 180 s apart (121 ticks, every gap 180 s,
  read from the fixture's GraphQL request log) and the worker gets exactly one
  prompt for that failure. On a 6-hour virtual clock against a pending-CI
  fixture, `wait --max-seconds 21600` checks exactly 121 times, 180 s apart,
  then returns the waiting document.
- **AC-7:** A live babysit of a real GitHub PR with `toolu babysit wait` built
  from this branch clears CI, threads and the bot verdict (sandbox: this
  issue's own PR in `Falconiere/toolu`; see Open Questions).
- **AC-8:** A detached OpenCode fixer started by `dispatch-fix start` is still
  running after the starting process exits, and `dispatch-fix cleanup` reaps
  its whole process group (no surviving pid).
- **AC-9:** `plugins/pr-babysit/hooks/src` and `hooks/dist` no longer exist;
  every skill, command, agent and workflow under `plugins/pr-babysit` runs
  `toolu babysit …` and names no `scripts/`, `hooks/dist/`, Bun bundle or
  `bun`; the workflow creates no cron.
- **AC-10:** `cargo xtask check-markdown-cli` passes with the pr-babysit
  Markdown, and the full `cargo xtask gate` passes.

## Acceptance evidence

| AC | Real input | Expected | Boundary / failure | Check |
|---|---|---|---|---|
| AC-1 | `fixtures/pr-babysit/{reduce-state.golden.json, parse-verdict.golden.json, collect-pr-115-p2.golden.json, snapshots/*, states/*, jev/fix-tiers.json, opencode/fixer-r1g1.log}`; recorded GitHub replies served over loopback TLS | each golden reproduced; herdr smoke prints `PASS` | locked slot, head_moved, duplicate_reply, resolve_unconfirmed, injection suspect, plan_invalid, worktree_dirty | `cargo test -p toolu-pr-babysit`; `bun run tooling/src/pr-babysit-herdr-smoke.ts` |
| AC-2 | epic fixture: green rollup, approved bot comment, resolved threads | issue phase `ready`, journal `github-babysit-ready`, 0 `prompt` | engine restart mid-way | `cargo test -p toolu-epic-orchestrator babysit_` |
| AC-3 | same fixture with one unresolved CI-reviewer thread, then switched to approved | exactly 1 `prompt` naming the thread id and `path:line`; then `ready` | 5 ticks with the same thread → still 1 | `cargo test -p toolu-epic-orchestrator babysit_` |
| AC-4 | pending-CI fixture, virtual clock per host | waiting doc at 1800/55/55 s | `--max-seconds 0` | `cargo test -p toolu-pr-babysit wait`; binary test with `--max-seconds 1` |
| AC-5 | fixture switching to green + approved at t | success at first slot ≥ t, ≤ t+180 | t exactly on a slot boundary | `cargo test -p toolu-pr-babysit wait` |
| AC-6 | red-CI fixture driven by the engine with `world.now_ms` advanced; pending-CI fixture driven by `wait` on a virtual clock | 121 ticks, every gap 180 s; 1 `prompt`/`babysit` record; waiting document after 121 checks | late wake (sleep overshoot) runs one check, not a burst | `cargo test -p toolu-epic-orchestrator babysit_`; `cargo test -p toolu-pr-babysit wait` |
| AC-7 | this PR on GitHub | `wait` result `decision: success` recorded in the PR body | rate limit | manual run, output in the PR |
| AC-8 | a real `sh -c` stand-in for `opencode` on `PATH` writing a log and sleeping | pid alive after start returns; dead after cleanup | reused pid not signalled | `cargo test -p toolu-pr-babysit fixer_opencode` |
| AC-9 | repository tree | paths absent; rg finds no `hooks/dist`, `bun`, `scripts/` in `plugins/pr-babysit` | — | `cargo xtask check-markdown-cli`; a `toolu-pr-babysit` test scanning the plugin Markdown |
| AC-10 | repository | exit 0 | — | `cargo xtask gate --base origin/main` |

## Documentation impact

`plugins/pr-babysit/{README.md, workflows/babysit.md, skills/babysit/SKILL.md,
skills/babysit/references/{helper.md, fixer-brief.md}, commands/babysit.md}`,
`plugins/pr-babysit/hooks/hooks.json`, `docs/cli/` (generated),
`docs/pr-babysit/README.md`, `docs/config.md` (`prBabysit` unchanged, helper
names), `docs/opencode.md` (generated section), `AGENTS.md` key-files rows for
pr-babysit and `toolu-engine`'s babysit trait, the OpenCode generator
(`tools/toolu-opencode/scripts/lib/opencode-port-pr-babysit.ts`) and its
acceptance scenarios, `CHANGELOG` through the `feat` commit.

## Open Questions

1. **Sandbox repository for AC-7** — owner: this worker; non-blocking. The
   brief authorises only this branch and its PR, not creating repositories, so
   the live run is this issue's own PR babysat with `toolu babysit wait` built
   from the branch. Decision recorded; the orchestrator may repeat it on a
   sandbox repository.
2. **Per-host wait defaults** — owner: this worker; non-blocking. 1800 s for
   Claude (background Bash has no timeout; a half-hour return keeps the session
   responsive), 55 s for Codex and OpenCode (inside today's 45–60 s bound).

## Spec review (2026-10-11)

Jev coverage check of each issue acceptance item against AC-1…AC-10: every
item scored 0.87–0.94 (covered).

- Interfaces: 🟡 should-fix: `reduce-state` flags differed from the shipped helper. Fixed: `--snapshot --state --now --state-out --result-out --state-path --snapshot-path`.
- AC-6: 🟡 should-fix: red CI makes `wait` return at once, so a 6-hour red-CI run is only meaningful in the engine. Fixed: engine run for red CI, `wait` run for pending CI.
- Architecture: 🔵 consider: AC-8 uses an `opencode` stand-in executable as the TS suite did; the real host is covered by the OpenCode acceptance scenarios, which keep running against `toolu babysit`.
- Failure modes: no blocker; locked, head_moved, malformed state and rate limits all state their exit and state effect.

**Status:** Approved

## Implementation decisions (2026-10-11)

Decided from the issue, the epic and the code; nobody answers questions in the
worker pane, so each carries its reason.

- **REST pagination** follows `Link: rel="next"` and passes the URL through
  unchanged, which the GitHub client accepts only under its API URL (so GitHub
  Enterprise `/api/v3` keeps its prefix). GraphQL and REST reads stop with
  `invalid_json` after 100 pages, so a reply that never ends cannot hang a tick.
- **Head read** is one REST `pulls/<n>` call, and a null `reviewDecision` is
  `""`, as the TypeScript collector produced.
- **Collection is sequential.** One controller per slot holds the lock, and the
  engine's GitHub budget already paces calls; parallel reads would add a
  thread pool to save little.
- **Config paths** come from `toolu_runtime::host::Roots`, the owner every native
  hook uses; Jev runs in process through `toolu-jev-client`; the report command
  a fixer runs is `toolu babysit fixer-report`, located through `current_exe`.
- **`GitHubTick`** is the public, configurable tick (config, environment, page
  size) so the engine's tests drive the real tick against loopback GitHub;
  `Tick` keeps the engine's `Config::scheduled` default.
- **Decision mapping.** `success` is Success. Otherwise only new attention is
  reported: `escalate` becomes Escalate (an attention item for a human, and the
  prompt goes to the worker), anything else Act (the prompt only). Attention
  that is not new is KeepGoing, so an escalation is raised once, not every
  180 s.
- **The engine's clock is the tick's `now`**, so the 6-hour virtual-clock run
  (AC-6) schedules from engine time.
- **Attention newness is the stored work set** (`attention.work` in the state):
  attention is new when an item is not in the previous tick's work. A one-tick
  code such as `review_changes` dropping out therefore does not prompt again
  (AC-3). A state written before this field compares fingerprints.
- **Prompt delivery.** The babysit prompt and the `STATUS?` nudge share
  `Step::Prompt`; the `prompt` effect script now reads `{"key","text"}` instead
  of `{}`. A babysit prompt that herdr or the script cannot deliver (or that is
  deferred for lack of both) raises a `babysit` attention item, because the
  tick has already recorded the work as seen; the nudge keeps its own fallback
  (a stall that survives it is an attention item).
- **`rate_limited`** carries `remaining` beside `retryAfter` and `resetAt`.
- **Fail closed on the process table.** When `ps` fails, stopping or waiting on
  an OpenCode fixer is `process_error`, never "gone", so cleanup does not
  remove the worktree of a fixer that may still run.
- **Detached fixers** get their own process group and null stdin, not a new
  session: `setsid` needs `unsafe` `pre_exec`, which the workspace forbids. The
  fixer has no terminal input, so job-control stops do not apply to it.
- **Inherited limits kept as in TypeScript**: SIGKILL goes to the pids taken
  before SIGTERM (a pid reused within those 10 s could be hit). The stale-lock
  reclaim now removes the directory only if it is still the one it judged
  stale, which narrows TypeScript's race but does not close it.
- **Inputs TypeScript accepted** stay accepted: `dispatch: null`, a whole-number
  `round` such as `2.0` or `null`, and an empty model or effort (unset).
  `prBabysit.jev: false` turns Jev off as `--no-jev` does, and a replayed
  answers file wins over both. `--base`/`--branch` starting with `-` are
  refused before any `git` call.
