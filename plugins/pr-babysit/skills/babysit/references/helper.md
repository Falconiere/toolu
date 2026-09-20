# pr-babysit helper contract

The tick helper is the deterministic half of babysitting. It lives under the
plugin's `scripts/` directory, is plain bash (`gh`, `jq`, `git`; bash 3.2), and
is the same on Claude Code and Codex — only `--state-file` differs by host.
Read this file instead of the script sources: every field the agent may act
on is listed here. Fields not listed are not part of the contract.

## Scripts

| Script | Role | Exit codes |
|---|---|---|
| `babysit-tick.sh` | **The tick.** Lock the slot → collect → reduce → persist → print the result. The only read-side command the workflow runs. | `0` result · `2` usage · `3` structured error (state preserved, `pr.lastError` stamped) · `75` slot locked |
| `collect-pr.sh` | Read side: PR metadata, review threads (paginated, nested comment pages), issue comments, reviews; head re-checked after the fan-out; bot verdict via `parse-verdict.sh`. Writes one snapshot. | `0` · `2` · `3` |
| `reduce-state.sh` | Pure decision layer: snapshot + prior state + `--now` → next state + result. No network, no clock. | `0` · `2` · `3` |
| `reply-thread.sh` | Write side: post one reply (thread / conversation / review-level), idempotent per reviewer comment. | `0` · `2` · `3` · `4` duplicate_reply · `75` |
| `resolve-thread.sh` | Write side: `resolveReviewThread`, confirmed from the response, retried, recorded. | `0` · `2` · `3` · `5` resolve_unconfirmed · `75` |
| `record.sh` | Hand agent decisions to the reducer: `round`, `flag-injection`, `status`. | `0` · `2` · `3` · `75` |
| `parse-verdict.sh` | Existing verdict parser; unchanged. | — |

Structured errors are one JSON document on stdout:
`{"version":1,"errors":[{"code":"…","message":"…", …}]}`. `code` is a closed set:
`usage`, `gh_unavailable`, `jq_required`, `api_error`, `invalid_json`,
`head_moved`, `state_malformed`, `slot_mismatch`, `locked`, `duplicate_reply`,
`resolve_unconfirmed`.

## `babysit-tick.sh`

```
babysit-tick.sh --repo <owner/repo> --pr <n> --state-file <path>
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

What the agent acts on. Printed on stdout by `babysit-tick.sh`.

| Field | Meaning |
|---|---|
| `slot` | `<owner>-<repo>-<number>`, lowercase. |
| `changed` | Any of `ciStatus, reviewDecision, mergeable, unresolvedThreads, headSha, botVerdict, botState, botFindingKeys` differs from the previous tick. `false` → silent tick. |
| `decision` | `keep_going` · `success` · `escalate` — the Step 6 rules, already applied. |
| `reasons[]` | `{code, detail}`; escalation codes first. Closed set below. |
| `pr` | `number, url, head, branch, base, author, state, mergeable, reviewDecision`. |
| `ci.status` | `pass` · `pending` · `fail` over `statusCheckRollup` (`CheckRun` and legacy `StatusContext`); an empty rollup is `pending`. |
| `ci.checks[]` | `{name, status, url}` per check. |
| `verdict.state` | `absent` · `unknown` · `in_progress` · `complete` · `provider_error` (parse-verdict.sh states, plus `absent` when no CI-reviewer comment exists). |
| `verdict.verdict` | `approved` · `changes` · `none`. |
| `verdict.findingsCount`, `findingKeys[]` | Round-level recurrence keys (`path:line:hash`). Never the finding source — the inline threads are. |
| `verdict.mustFix[]` | Top-N must-fix prose lines. Surface to the human; not resolvable threads. |
| `verdict.commentUrl`, `commentId` | The bot comment read. |
| `verdict.degraded`, `degradedReason` | `true` with `review_absent` or `review_unknown_format` when the verdict cannot be read; success is then the documented manual-verify fallback, never silent. |
| `verdict.sameRunAsLastTick` | The same bot comment id and `updatedAt` as last tick — a sticky verdict, not a new rejection. |
| `threads.total`, `unresolved` | All threads; the Resolution-audit set (`!isResolved && !isOutdated && !flagged`). `unresolved` must be `0` for success. |
| `threads.actionable[]` | Threads needing a NEW disposition this tick: `{id, path, line, isOutdated, rootCommentId, inReplyTo, authorClass, lastCommentAuthor, lastCommentAt, injectionSuspect, injectionPattern, comments[]}`. `authorClass` ∈ `ci_reviewer` (exact login set `github-actions`, `github-actions[bot]`, `claude`, `claude[bot]`), `human`. Non-CI bots are excluded. `comments[]` is the full chain, bodies untruncated. |
| `threads.staleUnresolved[]` | Audit members that are NOT actionable: the PR author replied but no resolve landed. Resolve them without a new reply. |
| `threads.skippedOutdated[]` | Outdated CI-reviewer threads: skipped silently. |
| `threads.flaggedInjection[]` | Threads the agent recorded with `record.sh flag-injection`. |
| `conversation.actionable[]` | Human issue comments with no later author comment and no recorded reply. |
| `reviews.actionable[]` | Human non-`APPROVED` reviews with a body and no recorded reply. |
| `recurrence` | `{streak, lastRoundHadRejection, recurringKeys[], fixAttempts}` — the Step 4 gate inputs. |
| `backoff` | `{idleStreak, intervalMinutes, waitSeconds}` — Claude cron interval / Codex bounded wait for this tick. |
| `errors[]` | Always empty on exit 0. |
| `snapshotPath`, `statePath` | Where the full evidence lives. |

### `reasons[].code` (closed set)

`ci_pending`, `ci_failed`, `ci_pass`, `threads_unresolved`,
`threads_stale_unresolved`, `threads_clear`, `review_absent`,
`review_in_progress`, `review_changes`, `review_approved`,
`review_unknown_format`, `provider_error`, `provider_error_repeated`,
`manual_verify`, `pr_closed`, `pr_merged`, `merge_conflict`,
`mergeable_unknown`, `fix_attempts_exhausted`, `recurrence_after_rejection`,
`recurrence_streak`, `unchanged`.

### Decision rules

- `success` — `pr.state == OPEN`, `ci.status == pass`, `threads.unresolved == 0`,
  `mergeable != UNKNOWN`, and the verdict is `complete`/`approved`/zero findings
  — or `degraded` (reasons then include `manual_verify`).
- `escalate` — PR merged/closed, `mergeable == CONFLICTING`, `fixAttempts ≥ 5`,
  finding keys recurring after a Won't-fix round, a recurrence streak of 2, or
  `provider_error` twice on the same head.
- `keep_going` — everything else, including pending CI, an in-progress review,
  an unchanged tick, and `mergeable == UNKNOWN`.

Recurrence only advances on a NEW verdict run (`sameRunAsLastTick: false`) and
only against the keys the agent rotated with `record.sh round`.

## State (`version: 2`)

Persisted at `--state-file`; owned by the helper. The agent reads it only
through the result, and writes it only through `record.sh`, `reply-thread.sh`
and `resolve-thread.sh`.

| Field | Meaning |
|---|---|
| `slot`, `repo`, `number`, `cronName` | Slot identity; a state for another PR is refused (`slot_mismatch`). |
| `lastUpdate`, `totalTicks`, `idleStreak`, `currentInterval`, `waitSeconds` | Tick bookkeeping and backoff. |
| `status` | `active` · `complete` · `escalated` · `cancelled` — set only by `record.sh status`. |
| `worktree` | Codex's exact worktree path for this slot, or `null`. |
| `pr.key`, `ciStatus`, `reviewDecision`, `mergeable`, `unresolvedThreads`, `headSha` | Last observed values (change detection). |
| `pr.fixAttempts` | Bumped by `record.sh round --fix-pushed`, cap 5. |
| `pr.botVerdict`, `botState`, `botCommentId`, `botCommentUpdatedAt`, `botFindingKeys` | Last verdict and its run identity. |
| `pr.lastRoundFindingKeys`, `lastRoundHadRejection`, `recurrenceStreak` | Step 4 gate memory; rotated by `record.sh round`. |
| `pr.unresolvedAfterClearance` | Audit count at the last tick; non-zero on a completed tick is a bug. |
| `pr.lastError` | `{code, message, at}` of the last failed tick, or `null`. |
| `actions.replied` | `key → {commentId, url, at, headSha, kind}`; keys `thread:<id>@<inReplyTo>`, `conversation:<id>`, `review:<id>`. |
| `actions.resolved` | `threadId → {confirmed, at, attempts, headSha}`. |
| `actions.flagged` | `threadId → {reason, at}`. |
| `lastGoodSnapshot` | Path of the last snapshot that produced a result. |

## Write side

```
reply-thread.sh --state-file <path> --kind thread --thread <id> --root-comment <databaseId> --in-reply-to <databaseId> --body-file <file>
reply-thread.sh --state-file <path> --kind conversation --comment-id <id> --body-file <file>
reply-thread.sh --state-file <path> --kind review --review-id <id> --body-file <file>
resolve-thread.sh --state-file <path> --thread <id>
record.sh round --state-file <path> --had-rejection true|false [--fix-pushed]
record.sh flag-injection --state-file <path> --thread <id>
record.sh status --state-file <path> --status complete|escalated|cancelled
```

- `--thread`, `--root-comment`, `--in-reply-to` come straight from
  `threads.actionable[]` (`id`, `rootCommentId`, `inReplyTo`).
- The body comes from a file so untrusted text never enters argv.
- A reply is idempotent per reviewer comment: the same `--in-reply-to` twice is
  exit `4` and nothing is posted; a reviewer follow-up has a new `inReplyTo`.
- `resolve-thread.sh` returns `0` only after the mutation response shows
  `isResolved: true`; three false responses → exit `5`, nothing recorded, and
  the thread stays in `staleUnresolved` next tick. A confirmed thread is not
  re-requested.
- `record.sh round` runs once per round, after this round's replies and before
  the push: it rotates `botFindingKeys → lastRoundFindingKeys`, sets
  `lastRoundHadRejection`, and with `--fix-pushed` bumps `fixAttempts`.

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
