# Epic: Muse as a worker host — Design

**Date:** 2026-10-23   **Status:** Approved   **Author:** Claude (epic worker, #603)   **Topic:** `toolu epic` launches, resumes, meters and tears down Muse Code workers

## Problem

toolu's plugins run natively on Muse Code (#539) and herdr 0.9.5 starts it
(`herdr agent start --kind muse`), but the epic engine rejects the host:

```text
$ toolu epic route --graph graph.json --hosts muse:1 --dry-run
toolu epic route: unknown host muse; use one of claude, codex, cursor
```

`HostKind` (`crates/epic-orchestrator/src/hosts.rs`) has no Muse variant, so
there are no launch arguments, routing defaults, session capture, token
metrics, status capacity row, limit host name or lifecycle evidence for it.
Epic #591 requires all four hosts to run workers.

## Non-Goals

1. pr-babysit fixer hosts (#604) and the epic-orchestrator skill's preflight
   and waiting (#605).
2. Approving Muse plugins (`muse plugins approve`): `toolu plugins install
   --host muse --approve` already does that (#539); the worker host must
   already have it.
3. Reading Muse's `session-index.db` (SQLite): the session directory layout is
   enough and toolu links no SQLite driver.
4. A default host pool change: Muse runs only where `--hosts`, `--kind` or a
   saved route names it.

## Architecture

Add `HostKind::Muse` (`name() == "muse"`, herdr's kind and the binary name, so
no alias) to `HOST_KINDS` after Cursor, and let the compiler drive every `match` to handle it.

**Live facts** (Muse Code 1.4.12, herdr 0.9.5, recorded in the probe evidence):

- `muse --yolo` disables approval and sandbox and trusts the workspace;
  `--approval-mode on-request --trust-workspace` keeps approvals on and loads
  workspace skills without saving trust.
- `--model <id>` and `--reasoning-effort <tier>` route model and effort.
- `muse resume <uuid> …flags` restores the exact conversation; `muse resume
  --last` restores the workspace's most recent one.
- A session is written at start to
  `$XDG_DATA_HOME/muse/sessions/<yyyy>/<mm>/<dd>/<uuid>/session.jsonl`
  (default `~/.local/share/muse`). Among its first records is one with
  `payload_type == "runtime.session.metadata"`, `stream.id == <uuid>` and
  `payload.record.workspace_root == <cwd>`. A session that never got a turn is
  deleted at exit.
- Token use is in `goal_usage_attribution` run events
  (`payload.event.record`), one per `usage_id`, with `quantity.reported`,
  `input_tokens`, `output_tokens`, `cached_tokens`, `reasoning_tokens`;
  subagents log theirs in `<uuid>/subagent/<child>/session.jsonl`, never in the
  parent file.
- `esc` cancels the running turn and its foreground tool; managed background
  bash sessions survive it. `/exit` exits at once when idle; with live
  background work Muse shows "Local work is still active … 1. Exit anyway /
  2. Stay" and needs `enter`, after which Muse and its background processes are
  gone.
- Plugin skills are named `plugin:<plugin>:<skill>` (`muse skills list`).
- Throttle text in the 1.4.12 binary: `usage limit reached`, `rate limited`,
  `model limit reached, try again later`, `You've used your <plan> quota
  (resets at <t>)`; the warning `At this pace your <plan> quota runs out in
  about <n> (resets at <t>)` is not a hit.
- Right after a resume Muse may start its own turn (it saw its background task
  cancelled) and drop a prompt typed meanwhile (`message not sent — the runtime
  is busy with other work · draft restored`).

**Changes:**

| Area | Change |
| --- | --- |
| `hosts.rs` | approvals `--yolo` / `--approval-mode on-request --trust-workspace`; effort `--reasoning-effort <e>`; resume lead `resume <id>` or `resume --last` (as Codex); `skill_ref` → ``the `plugin:<p>:<s>` skill``; error text lists `muse` |
| `route/table.rs` | Muse tiers `muse-spark-1.3`/low, `muse-wonder-1.0`/medium, `muse-wonder-1.0`/high, `muse-wonder-full-1.0`/xhigh (effort rises; model steps up as Claude's does); `routing.json` `hosts.muse` overrides |
| `launch/muse_session.rs` (new) | capture: scan the day directories of `since − 1 day`, `since`, `since + 1 day` (UTC; covers any local-time folder naming) for `<uuid>/session.jsonl` written since `since`, read ≤ 64 KiB / 32 records, accept the metadata record whose `stream.id` equals the directory name and whose `workspace_root` equals the worktree; exactly one match or none |
| `launch/session.rs` | dispatch `HostKind::Muse` to the new module |
| `launch/lifecycle.rs` | Muse exit: send `enter` right after `/exit` (confirms "Exit anyway" when background work is live; lands harmlessly at the pane shell when Muse already exited, verified live both ways); a failed `enter` because the agent is gone is ignored and the 30 s wait decides |
| `metrics/muse_tokens.rs` (new) | find `<uuid>` under `sessions/` (depth 3), sum the `reported` records of the main and subagent logs, deduplicated by `usage_id`; source `muse session log` |
| `metrics/tokens.rs` | route `"muse"` rows there |
| `limit.rs`, `probe_evidence.rs` | add `muse` |
| `status_resources.rs` | list hosts from `HOST_KINDS` instead of a copied literal |
| `hosts.rs` `HOST_LIMIT` | add Muse's own limit lines `model limit reached` and `you've used your … quota` (its `usage limit reached` and `rate limited` already match) |
| `docs/toolu/evidence/epic-hosts-v1.json` | add the live Muse probe record |

Trade-off: scanning three day folders instead of the SQLite index keeps toolu
dependency-free at the cost of a bounded directory walk (≤ 256 sessions, as
today's other hosts).

## Interfaces / Schema

- `HostKind::Muse`, `HOST_KINDS: [HostKind; 4]`.
- `host_kind` error: `unknown host <n>; use one of claude, codex, cursor, muse`.
- `agent_args(Muse, …)` examples:
  - unattended, fresh: `["--yolo", "--model", "muse-wonder-1.0", "--reasoning-effort", "medium"]`
  - attended: `["--approval-mode", "on-request", "--trust-workspace", …]`
  - resume captured: `["resume", "<uuid>", "--yolo", …]`; resume without id: `["resume", "--last", "--yolo", …]`
- `launch::muse_session::capture(worktree: &str, since: u64, env: &Env) -> Result<Option<String>, String>`;
  data root `XDG_DATA_HOME/muse` (non-empty) else `$HOME/.local/share/muse`.
- `metrics::muse_tokens::muse(session: &str, env: &Env) -> Result<(&'static str, Value), &'static str>`,
  tokens `{"input_tokens", "output_tokens", "cached_tokens", "reasoning_tokens"}`.
- Evidence: `hosts.muse` with the existing host schema; `HOSTS` keys
  `claude, codex, cursor, muse`.
- `toolu epic status` resources: `hosts.items` gains `{"host": "muse", …}`.

## Failure modes and edge cases

- Unknown host names still fail with the listed hosts; `opencode` stays unknown.
- Muse data root absent or a day folder missing → no candidate, `Ok(None)`;
  an unreadable directory (not `NotFound`) → `Err`, as other hosts.
- More than 256 session directories in the scanned days → `Ok(None)` (no
  guess), as other hosts.
- Two fresh sessions for one worktree → `Ok(None)` (ambiguous); a session for
  another `workspace_root`, an older session, a non-UUID directory, a file
  without the metadata record or with a `stream.id` other than its directory
  → ignored.
- A session vanishing mid-scan → skipped.
- Tokens: no captured id → `unavailable` "no session id was captured at
  launch"; no directory → "no session log for the session"; unreadable log →
  "the session log cannot be read"; zero reported records → tokens all zero
  (Muse logged none). Repeated `usage_id` counted once.
- Exit: if `enter` fails because the agent already left, the exit request
  still succeeds and the 30 s wait decides; if Muse stays (the key was lost),
  the wait ends `cleanup-incomplete` and the lease is kept, as other hosts.
- Shell safety: Muse arguments pass the existing safe-argument check; a model
  with a space is refused.

## Acceptance criteria

- **AC-1:** `toolu epic route --hosts muse:1 --dry-run` on a real graph routes
  issues to `muse` with the tier's Muse model and effort, and `routing.json`
  `hosts.muse` overrides them.
- **AC-2:** `agent_args` produce the captured Muse argv for unattended,
  attended, model/effort, resume-by-id and resume-last combinations, and
  `skill_ref(Muse, "delivery-flow", "delivery-flow")` is
  ``the `plugin:delivery-flow:delivery-flow` skill``.
- **AC-3:** Given a real Muse 1.4.12 session directory layout (trimmed real
  records), `capture_session(Muse, …)` returns exactly that session's UUID and
  rejects older, foreign-workspace, mismatched-id and ambiguous sessions;
  `require_captured_session` accepts only the exact id.
- **AC-4:** Given a real Muse session log with a subagent log, the token row for
  a `muse` worker sums the reported records of both, deduplicated, with source
  `muse session log`; a missing log is `unavailable`.
- **AC-5:** `toolu epic status` resource hosts, the provider-limit host list and
  `toolu epic probe --check-evidence` include `muse`; the committed evidence
  holds the live Muse record (start, tool task, resume, background work,
  cancel/replace, teardown) and validates.
- **AC-7:** Muse's `model limit reached, try again later` and `You've used your
  weekly quota (resets at 5pm)` lines count as provider limits; `At this pace
  your weekly quota runs out in about 2 hours (resets at 5pm)` (a warning) does not.
- **AC-6:** Shutting down a started Muse agent on the scripted herdr socket
  sends `esc`, `/exit`, then `enter`, and ends `exited` although the agent is
  already gone when `enter` is sent; Claude's sequence is unchanged.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
| --- | --- | --- | --- | --- |
| AC-1 | `fixtures/epic/epic248-graph.json`, temp `routing.json` | route JSON host `muse`, models per tier; override applied | unknown host error lists muse | `cargo test -p toolu-cli --test epic_graph` and `--test epic_launch` (new Muse cases), `cargo test -p toolu-epic-orchestrator table` |
| AC-2 | `fixtures/epic/hosts.json` new Muse `args`/`skill`/`alias` cases from the live argv | exact argv | unsafe model refused | `cargo test -p toolu-epic-orchestrator hosts` |
| AC-3 | `fixtures/epic/muse/sessions/…` trimmed from the probe session `01a16abb-7eef-7340-8cf2-2e0378dcf66c` | the UUID | foreign / old / mismatched / two sessions | `cargo test -p toolu-epic-orchestrator muse_session` |
| AC-4 | same fixture plus a real subagent log, issue record `e.json` | summed reported tokens | missing session → unavailable | `cargo test -p toolu-epic-orchestrator tokens` |
| AC-5 | committed `epic-hosts-v1.json`; a temp resource policy with a `muse` cap; a `rate-limited` failed report from a `muse` worker | `probe-evidence: valid`; status `hosts.items[3].host == "muse"` with its cap; the limit cools `muse` | evidence without `hosts.muse` → `expected keys claude, codex, cursor, muse` | `cargo test -p toolu-cli --test epic_probe`, `cargo test -p toolu-epic-orchestrator probe_evidence status_resources limit` |
| AC-7 | `fixtures/epic/hosts.json` `limit` cases with the Muse strings | match / no match | the quota warning | `cargo test -p toolu-epic-orchestrator hosts` |
| AC-6 | the scripted herdr socket (`launch_rig`) with a started `muse` agent; live probe pane runs recorded in the evidence | call sequence and `Exited` | `enter` after the agent left → ignored | `cargo test -p toolu-epic-orchestrator lifecycle` |

## Documentation impact

- `plugins/epic-orchestrator/README.md`: host list, Muse row in the flags
  table, Muse routing defaults, epic home, probe summary.
- `docs/epic-orchestrator/README.md`: host list and token-source sentence.
- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md` host list and
  `references/worker-brief.md` skill syntax (names only; preflight is #605).
- `--kind` help text and `docs/cli/commands.json` via `cargo xtask docs-cli`,
  plus the CLI contract snapshot.

## Open Questions

- Muse's day-folder time zone is unobserved at a date boundary; the ±1 day scan
  makes it irrelevant. Owner: this PR. Non-blocking.
- A prompt Muse drops while busy right after a resume is seen once in the
  probe; prompt acknowledgement then sees `working` from Muse's own turn. The
  epic's live run decides whether a Muse-specific wait is needed. Owner: epic
  #591. Non-blocking.
- Live end-to-end delivery of a PR by a Muse worker through the engine needs
  the merged build; it is the epic's acceptance run, not this PR's gate.
  Owner: epic #591. Non-blocking.

## Spec review

```
Failure modes: 🟡 should-fix: resumed Muse can drop a prompt while busy (seen live). Recorded as a non-blocking open question owned by #591. Fixed.
Acceptance criteria: 🟡 should-fix: usage-limit detection was only a host-name list; Muse's own throttle strings did not match HOST_LIMIT. Added AC-7 from the 1.4.12 binary's strings. Fixed.
Acceptance evidence: 🟡 should-fix: AC-5/AC-6 checks were vague. Named the scripted herdr rig, the policy/status input and the probe-evidence negative case. Fixed.
Architecture: 🔵 consider: the exit confirmation read was a second herdr round trip and untestable on the rig; replaced by an unconditional enter verified live in both states. Fixed.
```

Jev (spec review): issue coverage 0.63 before the fixes, 0.82 after; PR-delivery
deferral to the epic run 0.91; required tests 0.97.
