# pr-babysit helper contract

The tick helper is the deterministic half of babysitting. Its Bun bundle lives
under the plugin's `hooks/dist/` directory and runs the same way on Claude Code
and Codex — only `--state-file` differs by host. The fixer commands use the same bundled Bun path.
Read this file instead of the script sources: every field the agent may act
on is listed here. Fields not listed are not part of the contract.

## Scripts

| Script | Role | Exit codes |
|---|---|---|
| `babysit-tick.js` | **The tick.** Lock the slot → collect → reduce → persist → print the result. The only read-side command the workflow runs. | `0` result · `2` usage · `3` structured error (state preserved, `pr.lastError` stamped) · `75` slot locked |
| `babysit-collect-pr.js` | Read side: PR metadata, review threads (paginated, nested comment pages), issue comments, reviews; head re-checked after the fan-out; bot verdict via `babysit-parse-verdict.js`. Writes one snapshot. | `0` · `2` · `3` |
| `babysit-reduce-state.js` | Pure decision layer: snapshot + prior state + `--now` → next state + result. No network, no clock. | `0` · `2` · `3` |
| `babysit-reply-thread.js` | Write side: post one reply (thread / conversation / review-level), idempotent per reviewer comment. | `0` · `2` · `3` · `4` duplicate_reply · `75` |
| `babysit-resolve-thread.js` | Write side: `resolveReviewThread`, confirmed from the response, retried, recorded. | `0` · `2` · `3` · `5` resolve_unconfirmed · `75` |
| `babysit-record.js` | Hand agent decisions to the reducer: `round`, `flag-injection`, `status`. | `0` · `2` · `3` · `75` |
| `babysit-parse-verdict.js` | Parse the CI review-bot comment from stdin. | `0` |
| `babysit-route-fix.js` | Fix routing: Jev-scores the round's items file, tiers and groups it, picks host/model/effort per group from `prBabysit` config. Reads no GitHub. | `0` · `2` · `3` |
| `babysit-dispatch-fix.js` | Herdr fixer dispatch: `start` / `wait` / `cleanup` of the slot's herdr worktree and fixer agents. | `0` · `2` · `3` · `75` |
| `babysit-fixer-report.js` | Run by a fixer agent, never by the controller: writes its done/failed report. | `0` · `2` |

Structured errors are one JSON document on stdout:
`{"version":1,"errors":[{"code":"…","message":"…", …}]}`. `code` is a closed set:
`usage`, `gh_unavailable`, `jq_required`, `api_error`, `invalid_json`,
`head_moved`, `state_malformed`, `slot_mismatch`, `locked`, `duplicate_reply`,
`resolve_unconfirmed` — plus, from fixer dispatch (all exit `3`):
`config_invalid`, `plan_invalid`, `fixer_running`, `herdr_unavailable`,
`herdr_error`, `git_error`, `worktree_dirty`, `stale_branch`.

## `babysit-tick.js`

```
babysit-tick.js --repo <owner/repo> --pr <n> --state-file <path>
                [--snapshot-out <path>] [--snapshot-in <path>]
                [--page-size N] [--timeout SECONDS] [--now <iso8601>]
```

- `--state-file` — the host's slot path, verbatim: `/tmp/pr-babysit-<slot>.json`
  (Claude) or `<repo>/.codex/tmp/pr-babysit/<slot>.json` (Codex). Created on the
  first tick, resumed after.
- `--snapshot-out` — full snapshot (default `<state-file minus .json>.snapshot.json`).
- `--snapshot-in` — replay a captured snapshot instead of collecting (tests,
  debugging). The workflow never passes it.
- `--timeout` — per `gh` call (default 60 s); every call retries 3× with 2 s /
  4 s waits on timeouts, 5xx, 429, rate-limit 403 and connection errors, and
  never on other 4xx.
- `--page-size` — GraphQL/REST page size (default 100). Exists so pagination is
  testable on small real PRs.

The PR head is read before and after the fan-out; if it moved the collection
runs once more, then fails with `head_moved`. A tick never writes a partial
state: state and snapshot are written atomically (temp file + rename) and a
crash leaves the previous state and no temp file.

## Result (`version: 1`)

What the agent acts on. Printed on stdout by `babysit-tick.js`.

| Field | Meaning |
|---|---|
| `slot` | `<owner>-<repo>-<number>`, lowercase. |
| `changed` | Any of `ciStatus, reviewDecision, mergeable, unresolvedThreads, headSha, botVerdict, botState, botFindingKeys` differs from the previous tick. `false` → silent tick. |
| `decision` | `keep_going` · `success` · `escalate` — the Step 6 rules, already applied. |
| `reasons[]` | `{code, detail}`; escalation codes first. Closed set below. |
| `pr` | `number, url, head, branch, base, author, state, mergeable, reviewDecision`. |
| `ci.status` | `pass` · `pending` · `fail` over `statusCheckRollup` (`CheckRun` and legacy `StatusContext`); an empty rollup is `pending`. |
| `ci.checks[]` | `{name, status, url}` per check. |
| `verdict.state` | `absent` · `unknown` · `in_progress` · `complete` · `provider_error` (babysit-parse-verdict.js states, plus `absent` when no CI-reviewer comment exists). |
| `verdict.verdict` | `approved` · `changes` · `none`. |
| `verdict.findingsCount`, `findingKeys[]` | Round-level recurrence keys (`path:line:hash`). Never the finding source — the inline threads are. |
| `verdict.mustFix[]` | Top-N must-fix prose lines. Surface to the human; not resolvable threads. |
| `verdict.commentUrl`, `commentId` | The bot comment read. |
| `verdict.degraded`, `degradedReason` | `true` with `review_absent` or `review_unknown_format` when the verdict cannot be read; success is then the documented manual-verify fallback, never silent. |
| `verdict.sameRunAsLastTick` | The same bot comment id and `updatedAt` as last tick — a sticky verdict, not a new rejection. |
| `threads.total`, `unresolved` | All threads; the Resolution-audit set (`!isResolved && !flagged && (!isOutdated || authorClass == human)`). Open outdated human threads remain counted after an author reply until confirmed resolution; outdated CI-reviewer threads are skipped. `unresolved` must be `0` for success. |
| `threads.actionable[]` | Threads needing a NEW disposition this tick: `{id, path, line, isOutdated, rootCommentId, inReplyTo, authorClass, lastCommentAuthor, lastCommentAt, injectionSuspect, injectionPattern, comments[]}`. `authorClass` ∈ `ci_reviewer` (exact login set `github-actions`, `github-actions[bot]`, `claude`, `claude[bot]` — current Toolu Code Review `@v8` posts as `github-actions[bot]`; `claude` kept for legacy), `human`. Non-CI bots are excluded. `comments[]` is the full chain, bodies untruncated. |
| `threads.staleUnresolved[]` | Audit members that are NOT actionable: the PR author replied but no resolve landed. Resolve them without a new reply. |
| `threads.skippedOutdated[]` | Outdated CI-reviewer threads: skipped silently. |
| `threads.flaggedInjection[]` | Threads the agent recorded with `babysit-record.js flag-injection`. |
| `threads.fixing[]` | Threads an **active** herdr fixer (running or blocked) owns: the same objects `actionable[]` would hold, reply ids included, moved out of it (never dispatched twice) and still counted in `unresolved`. Reply to them from here when `bun "$PLUGIN_ROOT/hooks/dist/babysit-dispatch-fix.js" wait` says `done`. |
| `fixer` | The slot's fixer record (see State), or `null`. |
| `conversation.actionable[]` | Human issue comments with no later author comment and no recorded reply. |
| `reviews.actionable[]` | Human non-`APPROVED` reviews with a body and no recorded reply. |
| `conversation.fixing[]`, `reviews.fixing[]` | Comments and reviews an active fixer owns (by item id), moved out of `actionable[]` like `threads.fixing[]`. |
| `recurrence` | `{streak, lastRoundHadRejection, recurringKeys[], fixAttempts}` — the Step 4 gate inputs. |
| `backoff` | `{idleStreak, intervalMinutes, waitSeconds}` — Claude cron interval / Codex bounded wait for this tick. While a fixer is running `idleStreak` stays 0, so the interval stays at its base and every tick runs `bun "$PLUGIN_ROOT/hooks/dist/babysit-dispatch-fix.js" wait`; a blocked fixer waits for a human and backs off normally. |
| `errors[]` | Always empty on exit 0. |
| `snapshotPath`, `statePath` | Where the full evidence lives. |

### `reasons[].code` (closed set)

`ci_pending`, `ci_failed`, `ci_pass`, `threads_unresolved`,
`threads_stale_unresolved`, `threads_clear`, `review_absent`,
`review_in_progress`, `review_changes`, `review_approved`,
`review_unknown_format`, `provider_error`, `provider_error_repeated`,
`manual_verify`, `pr_closed`, `pr_merged`, `merge_conflict`,
`mergeable_unknown`, `fix_attempts_exhausted`, `recurrence_after_rejection`,
`recurrence_streak`, `unchanged`, `fixer_running`.

### Decision rules

- `success` — `pr.state == OPEN`, `ci.status == pass`, `threads.unresolved == 0`,
  `mergeable != UNKNOWN`, no fixer active (running or blocked), and the verdict is
  `complete`/`approved`/zero findings — or `degraded` (reasons then include
  `manual_verify`).
- `escalate` — PR merged/closed, `mergeable == CONFLICTING`, `fixAttempts ≥ 5`,
  finding keys recurring after a Won't-fix round, a recurrence streak of 2, or
  `provider_error` twice on the same head.
- `keep_going` — everything else, including pending CI, an in-progress review,
  an unchanged tick, and `mergeable == UNKNOWN`.

Recurrence only advances on a NEW verdict run (`sameRunAsLastTick: false`) and
only against the keys the agent rotated with `babysit-record.js round`.

## State (`version: 2`)

Persisted at `--state-file`; owned by the helper. The agent reads it only
through the result, and writes it only through `babysit-record.js`, `babysit-reply-thread.js`
and `babysit-resolve-thread.js`.

| Field | Meaning |
|---|---|
| `slot`, `repo`, `number`, `cronName` | Slot identity; a state for another PR is refused (`slot_mismatch`). |
| `lastUpdate`, `totalTicks`, `idleStreak`, `currentInterval`, `waitSeconds` | Tick bookkeeping and backoff. |
| `status` | `active` · `complete` · `escalated` · `cancelled` — set only by `babysit-record.js status`. |
| `worktree` | Codex's exact worktree path for this slot, or `null`. |
| `pr.key`, `ciStatus`, `reviewDecision`, `mergeable`, `unresolvedThreads`, `headSha` | Last observed values (change detection). |
| `pr.fixAttempts` | Bumped by `babysit-record.js round --fix-pushed`, cap 5. |
| `pr.botVerdict`, `botState`, `botCommentId`, `botCommentUpdatedAt`, `botFindingKeys` | Last verdict and its run identity. |
| `pr.lastRoundFindingKeys`, `lastRoundHadRejection`, `recurrenceStreak` | Step 4 gate memory; rotated by `babysit-record.js round`. |
| `pr.unresolvedAfterClearance` | Audit count at the last tick; non-zero on a completed tick is a bug. |
| `pr.lastError` | `{code, message, at}` of the last failed tick, or `null`. |
| `actions.replied` | `key → {commentId, url, at, headSha, kind}`; keys `thread:<id>@<inReplyTo>`, `conversation:<id>`, `review:<id>`. |
| `actions.resolved` | `threadId → {confirmed, at, attempts, headSha}`. |
| `actions.flagged` | `threadId → {reason, at}`. |
| `lastGoodSnapshot` | Path of the last snapshot that produced a result. |
| `fixer` | Written by `babysit-dispatch-fix.js`: `{round, status (running·done·failed·blocked), reason, startedAt, finishedAt, current, unattended, context, itemsFile, items[], groups[{seq, tier, host, model, effort, items[], agent, status (pending·launching·running·done·failed·blocked), reason, error?, brief, report, startedAt, finishedAt, head}]}`, or `null`. `babysit-record.js round` clears a done or failed record; a running or blocked one has a live agent and is kept. |
| `herdrWorktree` | `{path, workspaceId, paneId, branch: "pr-babysit/<slot>", prBranch, repoRoot, base}` of the slot's herdr worktree, or `null`. |
| `hostCooldowns` | `{<host>: {until, reason: "host_limited"}}` — a host that hit a provider usage limit is skipped by `babysit-route-fix.js` for 60 min. |

## Write side

```
babysit-reply-thread.js --state-file <path> --kind thread --thread <id> --root-comment <databaseId> --in-reply-to <databaseId> --body-file <file>
babysit-reply-thread.js --state-file <path> --kind conversation --comment-id <id> --body-file <file>
babysit-reply-thread.js --state-file <path> --kind review --review-id <id> --body-file <file>
babysit-resolve-thread.js --state-file <path> --thread <id>
babysit-record.js round --state-file <path> --had-rejection true|false [--fix-pushed]
babysit-record.js flag-injection --state-file <path> --thread <id>
babysit-record.js status --state-file <path> --status complete|escalated|cancelled
```

- `--thread`, `--root-comment`, `--in-reply-to` come straight from
  `threads.actionable[]` (`id`, `rootCommentId`, `inReplyTo`).
- The body comes from a file so untrusted text never enters argv.
- A reply is idempotent per reviewer comment: the same `--in-reply-to` twice is
  exit `4` and nothing is posted; a reviewer follow-up has a new `inReplyTo`.
- `babysit-resolve-thread.js` returns `0` only after the mutation response shows
  `isResolved: true`; three false responses → exit `5`, nothing recorded, and
  the thread stays in `staleUnresolved` next tick. A confirmed thread is not
  re-requested.
- `babysit-record.js round` runs once per round, after this round's replies and before
  the push: it rotates `botFindingKeys → lastRoundFindingKeys`, sets
  `lastRoundHadRejection`, with `--fix-pushed` bumps `fixAttempts`, and clears
  a done or failed `fixer` record.

## Fixer dispatch

```
bun "$PLUGIN_ROOT/hooks/dist/babysit-route-fix.js" --items <file> --host claude|codex [--state-file <path>] [--raise <itemId>]... [--no-jev]
             [--jev-answers-in <file>] [--now <iso8601>]
bun "$PLUGIN_ROOT/hooks/dist/babysit-dispatch-fix.js" start   --state-file <p> --plan <route.json> --items <items.json> --repo-root <dir> --branch <pr-branch> --base <base-branch> [--dry-run]
bun "$PLUGIN_ROOT/hooks/dist/babysit-dispatch-fix.js" wait    --state-file <p> [--timeout-seconds N]
bun "$PLUGIN_ROOT/hooks/dist/babysit-dispatch-fix.js" cleanup --state-file <p> [--dry-run]
bun "$PLUGIN_ROOT/hooks/dist/babysit-fixer-report.js" <report-file> done|failed [--note <text>]
```

- **Items file** (the agent writes it): `{round, items:[{id, kind: thread|conversation|review|ci,
  task, path?, line?, severity?, quote?}]}`. `task` is the agent's instruction; `quote` is the
  reviewer's text, which reaches a fixer only inside an untrusted-data fence and never reaches Jev
  or the heuristic.
- **Route** (`babysit-route-fix.js` stdout): `{version:1, dispatch: herdr|inline, unattended, source:
  jev|heuristic|mixed, note, items:[{id, tier, score, confidence, raised, source}], groups:[{seq,
  tier, class, host, model, effort, items[]}]}`. Tier = `clamp(round(score + 0.15), 0, 3)` over
  `trivial · standard · complex · critical`; `class` maps to the inline rubric (`mechanical ·
  implementation · architecture · architecture`). A host is dropped when cooling
  (`hostCooldowns`) or when its CLI (`claude`, `codex`, `cursor-agent`) is not on `PATH`; no host
  left → `dispatch: inline` with a `note`. `--jev-answers-in` replays a captured Jev answer map
  (tests, debugging — the workflow never passes it).
- **Start** validates everything before a side effect: the plan and items (`plan_invalid`, including a
  `round` that is not a positive integer), each group's host and model/effort (`config_invalid`), an
  active fixer (`fixer_running`, also when blocked), then herdr (`herdr_unavailable`). It fetches
  `origin/<base>` (the brief's changed-file rule diffs against it) and the PR branch; a failed git
  step is `git_error`.
- **Wait** waits at most `--timeout-seconds` (default 480) for the running group. A group moves
  `pending → launching → running`. A launch that is due (a group never started, or one cut off
  midway) runs first in every call, whatever the timeout; the next group starts in the same call
  right after a settle only with at least 250 s of the wait left, otherwise in the next call.
  Starting an agent normally takes seconds, and up to about 4 minutes only when it fails. A
  `blocked` fixer whose prompt was answered, or whose agent is gone, is picked up again. `wait`
  has no `--dry-run` (usage error).
- **Dispatch status** (`start` / `wait` stdout): `{version:1, status: running|done|failed|blocked|none,
  reason: null|no_report|reported_failed|host_limited|agent_blocked|agent_start_failed|worktree_lost, group,
  worktree, branch, commits[], groups[{seq, tier, host, model, effort, agent, status, reason, error?}]}`
  — `error` carries herdr's message when a group failed to start.
  `commits[]` = `git rev-list --reverse origin/<pr-branch>..HEAD` in the worktree. `--dry-run`
  prints `{dryRun:true, commands:[[argv…]…], brief}` and writes nothing.
- **Agents** are named `pb-<6 hex of the slot>-r<round>g<seq>` and started with
  `herdr agent start <name> --kind <host> --pane <pane> -- <host args>`: Claude
  `--dangerously-skip-permissions -n <name> --model --effort`, Codex
  `--no-daemon --dangerously-bypass-approvals-and-sandbox --model -c model_reasoning_effort=<e>`, Cursor
  `--yolo --trust --approve-mcps --model` (safe mode with `prBabysit.unattended: false`).
- **Brief and report** sit beside the state file: `<state>.fixer-r<round>g<seq>.md` and
  `.report.json`. A settled agent with no report is `no_report`, or `host_limited` when its pane
  shows a provider usage/rate limit (the host then cools for 60 min).
- **Startup trust prompt.** A Claude fixer in a new worktree starts blocked (`agent_not_ready`) at
  Claude Code's first-run workspace-trust prompt. The dispatcher accepts that prompt only when it is
  the standard one naming exactly this worktree (epic-orchestrator's recovery rule); any other
  blocked screen fails the group with `agent_start_failed` and is never answered.
- **Session artifacts are not work.** Untracked files under `.claude/`, `.codex/` or `.cursor/` (a
  fixer's own SessionStart hooks write `.claude/settings.local.json` and `.claude/tmp/`) do not make
  the worktree dirty; any other change does (`worktree_dirty`, with `changes[]`).
- **Cleanup** exits a live fixer and clears the `fixer` record, then removes the worktree (`--force`,
  discarding only those session artifacts) when it has no other changes (`worktree_dirty`
  otherwise, the worktree stays recorded), prunes git's worktree metadata, deletes
  `pr-babysit/<slot>` only when `origin/<pr-branch>` contains it, and removes this slot's
  `<state>.fixer-*` brief, report and items files. It prints `{version:1, status:"cleaned",
  worktreeRemoved, branchDeleted, note}`; `note` says why a branch was kept, or that no worktree
  was recorded.

## Examples (captured from Falconiere/toolu#165)

`decision: success` — open PR, green CI, approved verdict, no open threads:

```json
{"version":1,"slot":"falconiere-toolu-165","changed":true,"decision":"success",
 "reasons":[{"code":"ci_pass","detail":"5 check(s) passed"},
            {"code":"threads_clear","detail":"no unresolved review threads"},
            {"code":"review_approved","detail":"bot verdict approved with zero findings"}],
 "pr":{"number":165,"url":"https://github.com/Falconiere/toolu/pull/165","head":"a29e6c97379674323292578f9f0cfbffa00e375a",
       "branch":"feat/python-quality","base":"main","author":"Falconiere","state":"OPEN","mergeable":"MERGEABLE","reviewDecision":"REVIEW_REQUIRED"},
 "ci":{"status":"pass","checks":[{"name":"shellcheck","status":"pass","url":"https://github.com/Falconiere/toolu/actions/runs/33431959050/job/99619201007"}]},
 "verdict":{"state":"complete","verdict":"approved","findingsCount":0,"findingKeys":[],"mustFix":[],
            "commentUrl":"https://github.com/Falconiere/toolu/pull/165#issuecomment-5483377990","commentId":5483377990,
            "degraded":false,"degradedReason":null,"sameRunAsLastTick":false},
 "threads":{"total":6,"unresolved":0,"actionable":[],"staleUnresolved":[],"skippedOutdated":[],"flaggedInjection":[]},
 "conversation":{"actionable":[]},"reviews":{"actionable":[]},
 "recurrence":{"streak":0,"lastRoundHadRejection":false,"recurringKeys":[],"fixAttempts":0},
 "backoff":{"idleStreak":0,"intervalMinutes":3,"waitSeconds":15},
 "errors":[],"snapshotPath":"/tmp/pr-babysit-falconiere-toolu-165.snapshot.json","statePath":"/tmp/pr-babysit-falconiere-toolu-165.json"}
```

`decision: keep_going` — review still running, one CI-reviewer thread open (comment bodies trimmed here):

```json
{"decision":"keep_going",
 "reasons":[{"code":"ci_pass","detail":"5 check(s) passed"},
            {"code":"threads_unresolved","detail":"1 actionable thread(s)"},
            {"code":"review_in_progress","detail":"review bot still running"}],
 "verdict":{"state":"in_progress","verdict":"none","findingsCount":0,"sameRunAsLastTick":false},
 "threads":{"total":6,"unresolved":1,
   "actionable":[{"id":"PRRT_kwDOSzUwAc6d2S5O","path":"plugins/rust-quality/hooks/concerns/20-tests.sh","line":26,
     "isOutdated":false,"rootCommentId":3897681281,"inReplyTo":3897751832,"authorClass":"ci_reviewer",
     "lastCommentAuthor":"github-actions","lastCommentAt":"2026-08-31T19:39:25Z","injectionSuspect":false,"injectionPattern":null,
     "comments":[{"id":"PRRC_kwDOSzUwAc7oUeWB","databaseId":3897681281,"body":"**medium** _(CORRECTNESS)_: …","author":"github-actions","authorType":"Bot","createdAt":"2026-08-31T19:29:22Z","url":"…"},
                 {"id":"PRRC_kwDOSzUwAc7oUuxU","databaseId":3897748564,"body":"No change — …","author":"Falconiere","authorType":"User","createdAt":"2026-08-31T19:38:58Z","url":"…"},
                 {"id":"PRRC_kwDOSzUwAc7oUvkY","databaseId":3897751832,"body":"**Still flagging after re-review.** …","author":"github-actions","authorType":"Bot","createdAt":"2026-08-31T19:39:25Z","url":"…"}]}],
   "staleUnresolved":[],"skippedOutdated":[],"flaggedInjection":[]}}
```

`decision: escalate` — the PR was merged externally:

```json
{"decision":"escalate","changed":true,
 "reasons":[{"code":"pr_merged","detail":"PR is merged"},{"code":"ci_pass","detail":"5 check(s) passed"},
            {"code":"threads_clear","detail":"no unresolved review threads"},{"code":"review_approved","detail":"bot verdict approved with zero findings"}]}
```

Structured error — the slot is held by another controller:

```json
{"version":1,"errors":[{"code":"locked","message":"slot is held by another controller","pid":9103,"since":1789871015}]}
```
