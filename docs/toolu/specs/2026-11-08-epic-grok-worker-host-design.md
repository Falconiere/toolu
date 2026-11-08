# Epic: Grok as a worker host — Design

**Date:** 2026-11-08   **Status:** Approved   **Author:** Claude (delivery-flow, #846)   **Topic:** `toolu epic` launches, resumes, meters and tears down Grok Build workers

## Problem

herdr 0.9.5 can start Grok Build (`herdr agent start --kind grok`), but the
epic engine rejects the host:

```text
$ toolu epic route --graph graph.json --hosts grok:1 --dry-run
toolu epic route: unknown host grok; use one of claude, codex, cursor, muse
```

`HostKind` (`crates/epic-orchestrator/src/hosts.rs`) has no Grok variant. So
there are no launch arguments, routing defaults, session capture, token
metrics, status capacity row, limit host name or lifecycle evidence for Grok.
The engine's no-CI merge gate (`gate/local.rs`) and the status plan
(`status_plan.rs`) also look up the worker's ledger with `Host::parse(kind)`.
For `grok` that lookup finds nothing, so a Grok worker under `--no-babysit`
would wait forever on `local gate: no host recorded`.

## Non-Goals

1. Making Grok a first-class toolu plugin host. That means a `toolu --host
   grok`, `toolu plugins install --host grok`, SessionStart matchers for
   Grok's `new`/`load` sources, Grok tool names in hook payloads and host-smoke.
   Grok already loads Claude-installed plugins through its Claude
   compatibility layer. It runs toolu's UserPromptSubmit, PreToolUse and
   PostToolUse hooks (seen live on 1.0.77), but never the SessionStart hooks,
   so rules that register at session start, such as git-better, stay off. A
   follow-up issue owns this gap, and with it #846's "deliver a PR with toolu
   hooks active" acceptance. This PR refers to #846 and does not close it.
2. pr-babysit fixer hosts on Grok.
3. Running the epic-orchestrator skill itself from a Grok session.
4. A change to the default host pool: Grok runs only where `--hosts`,
   `--kind` or a saved route names it.
5. Encoded working directories longer than 255 bytes: Grok then names the
   group with a slug and a hash plus a `.cwd` file. Capture does not read
   that layout and returns no session, which fails the launch safely.

## Architecture

Add `HostKind::Grok` (`name() == "grok"`: herdr's kind and the binary name, so
no alias) to `HOST_KINDS` after Muse. The compiler then flags every `match`
that must handle it.

**Live facts** (Grok Build 1.0.25 → self-updated to 1.0.77 mid-probe,
herdr 0.9.5, 2026-11-08, recorded in the probe evidence):

- Unattended: `--always-approve`; footer `· always-approve`.
- Attended: `--permission-mode default`. This flag is needed because a user's
  `[ui] permission_mode = "always-approve"` in `~/.grok/config.toml`
  otherwise wins. A write then blocks on Grok's approval menu, and herdr
  reports `blocked`.
- Model and effort: `--model <id>`, `--reasoning-effort low|medium|high|xhigh`.
  Models: `grok-4.7` (default), `grok-4.7-build-fast`, `grok-4.6`,
  `grok-4.5`. `grok-4.7` accepted `xhigh`.
- Resume: `grok --resume <uuid> …flags` restores that exact conversation (no
  new session directory). `--continue` resumes the most recent session for the
  working directory. Right after Grok's self-update, the first plain `--wait`
  prompt after a resume was not submitted. Two later resumes then took the
  engine's own call (`agent prompt … --wait --until working --until blocked`)
  at once.
- Login: `XAI_API_KEY` is set, or `$GROK_HOME/auth.json` (else
  `~/.grok/auth.json`) exists; `grok login` signs in. `grok models` printed
  `You are not authenticated.` while prompts worked, so it is no login check.
- Sessions: `$GROK_HOME` (default `~/.grok`) `/sessions/<encoded cwd>/<uuid>/`
  is written at start, before the first prompt. The cwd is percent-encoded
  with the RFC 3986 unreserved set kept (`A-Za-z0-9-._~`) and uppercase hex
  for every other byte. This was verified on
  `/private/tmp/grok enc_~+@,=#%x.y-z` →
  `%2Fprivate%2Ftmp%2Fgrok%20enc_~%2B%40%2C%3D%23%25x.y-z`.
  `summary.json` holds `info.id`, `info.cwd`, `created_at` and `session_kind`:
  `null` when interactive, `headless`, or `subagent`.
- Subagents write their own session under the same cwd group with
  `session_kind: "subagent"` and `parent_session_id: null`. The parent's
  `subagents/<child>/meta.json` names the child.
- Tokens: `<session>/usage.json` `session` holds `inputTokens`,
  `outputTokens`, `cachedReadTokens`, `cacheCreationTokens`,
  `reasoningTokens` and `totalTokens`, and already includes subagent calls (a
  parent made 2 model calls and its child 3, and the parent's `modelCalls` is
  5). The file appears after the first turn.
- Cancel: `esc` does **not** cancel a running turn. `ctrl+c` cancels it and
  kills the foreground tool (`sleep 120` was gone), while a background task
  (`sleep 901`) keeps running. `ctrl+c` on an idle Grok is harmless.
- Exit: `/exit` (alias of `/quit`) exits at once, even with a live
  background task, which it kills. It prints `grok --resume <uuid>`. No grok
  process or leader stays.
- Plugin skills are invoked as `/<plugin>:<skill>`, as in Claude Code.
- toolu inside Grok detects the host as Claude Code. Grok's shell exports only
  `GROK_AGENT` and `GROK_SESSION_ID`, and `toolu ledger path` in a Grok pane
  printed `<worktree>/.claude/tmp/plan-ledger/main.json`.
- Throttle text in the 1.0.77 binary: `You've hit the rate limit for your
  plan. Upgrade your account or try again later.` (already matched),
  `<team>'s API rate limit …` (already matched), `You've hit the credit limit
  for your plan.` (new) and the error code `free-usage-exhausted` (new).

**Changes:**

| Area | Change |
| --- | --- |
| `hosts.rs` | Variant, `HOST_KINDS` (5), parse `grok`, error text lists `grok`. Approvals are `--always-approve` or `--permission-mode default`. Effort is `--reasoning-effort <e>`. Resume uses lead `--resume <id>`, else tail `--continue` (as Claude/Cursor). `skill_ref` is `` `/{p}:{s}` ``. `HOST_LIMIT` adds `hit the credit limit` and `free[-_]usage[-_]exhausted`. New `ledger_host(name) -> Option<Host>`: `grok` maps to `Host::Claude`, anything else to `Host::parse`. |
| `route/table.rs` | Grok tiers: `grok-4.7-build-fast`/low, `grok-4.7`/medium, `grok-4.7`/high, `grok-4.7`/xhigh. `routing.json` `hosts.grok` overrides them. |
| `launch/grok_session.rs` (new) | `grok_home(env)` and `group(env, worktree)` (the encoded dir). `capture` lists `<group>/<uuid>/` (≤ 256), reads `summary.json` written since `since` (bounded head), and accepts `info.id == dir name`, `info.cwd == worktree` and `session_kind != "subagent"`. It returns exactly one match or none. |
| `launch/session.rs` | Dispatch `HostKind::Grok` to the new module. |
| `launch/lifecycle.rs` | `request_exit` sends `ctrl+c` instead of `esc` for Grok, then `/exit` as for every host. |
| `metrics/tokens.rs` | `("grok", Some(id))` reads `<group(worktree)>/<id>/usage.json` `session` and keeps the six token fields. The source is `grok usage record`. |
| `gate/local.rs`, `status_plan.rs` | Look up the ledger host through `hosts::ledger_host` instead of `Host::parse`. |
| `limit.rs`, `probe_evidence.rs` | Add `grok`. |
| `screen.rs`, `tmux_backend.rs` | Exhaustive `match` arms: no known approval screen; the binary is `grok`. |
| `verbs_hosts.rs`, `verbs_actions/launch.rs`, `status_schema.rs` | Help text and host lists name `grok`. Add a status-probe row for Grok. The ledger-directory sentence says Grok uses Claude Code's. |
| `docs/toolu/evidence/epic-hosts-v1.json` | Add the live Grok probe record. |

Trade-off: computing the encoded group name, rather than scanning every group
and decoding it, reads one directory and needs no decoder. It costs the
>255-byte layout (Non-Goal 5).

## Interfaces / Schema

- `HostKind::Grok`, `HOST_KINDS: [HostKind; 5]`.
- `host_kind` error: `unknown host <n>; use one of claude, codex, cursor, muse, grok`.
- `hosts::ledger_host(name: &str) -> Option<toolu_protocol::host::Host>`.
- `agent_args(Grok, …)`:
  - unattended, fresh: `["--always-approve", "--model", "grok-4.7", "--reasoning-effort", "medium"]`
  - attended: `["--permission-mode", "default", …]`
  - resume captured: `["--resume", "<uuid>", "--always-approve", …]`
  - resume without id: `["--always-approve", …, "--continue"]`
- `launch::grok_session::{grok_home(&Env) -> PathBuf, group(&Env, &str) -> PathBuf, capture(&str, u64, &Env) -> Result<Option<String>, String>}`.
  The home is `GROK_HOME` (when non-empty), else `$HOME/.grok`.
- Token row: `{"key", "host": "grok", "source": "grok usage record",
  "complete", "tokens": {"inputTokens", "outputTokens", "cachedReadTokens",
  "cacheCreationTokens", "reasoningTokens", "totalTokens"}}`.
- Evidence: `hosts.grok` with the existing host schema; `HOSTS` keys
  `claude, codex, cursor, muse, grok`.
- `toolu epic status` resources: `hosts.items` gains `{"host": "grok", …}`.

## Failure modes and edge cases

- Unknown host names still fail with the listed hosts; `opencode` stays unknown.
- No Grok home, no group directory, or a `summary.json` that is missing or
  unparseable gives no candidate (`Ok(None)`). An unreadable directory (not
  `NotFound`) is `Err`, as for other hosts.
- More than 256 session directories in the group → `Ok(None)` (no guess).
- Two fresh top-level sessions for one worktree → `Ok(None)` (ambiguous). These
  are ignored: a `subagent` session, an older session, another cwd, a non-UUID
  directory, and an `info.id` other than its directory.
- A session vanishing mid-scan is skipped.
- Tokens:
  - No captured id → `unavailable`, "no session id was captured at launch".
  - No `usage.json` → "no usage record for the session".
  - Unparseable, or no `session` object → "the usage record cannot be read".
  - A missing field counts as 0.
- Ledger host: an unknown name gives `None`, so the local gate still says `no
  host recorded`. Claude, Codex, Cursor and Muse map exactly as before.
- Exit: if `ctrl+c` fails because the agent already left, the exit request
  succeeds, as today. If Grok stays, the 30 s wait ends `cleanup-incomplete`.
- Shell safety: Grok arguments pass the existing safe-argument check; a model
  with a space is refused.

## Acceptance criteria

- **AC-1:** `toolu epic route --hosts grok:1 --dry-run` on a real graph routes
  issues to `grok` with the tier's Grok model and effort: trivial
  `grok-4.7-build-fast`/`low`, standard `grok-4.7`/`medium`, complex
  `grok-4.7`/`high`, critical `grok-4.7`/`xhigh`. A `routing.json`
  `hosts.grok` row replaces them. `launch --kind grok --dry-run` prints a
  `herdr agent start … --kind grok -- …` argv.
- **AC-2:** `agent_args` produce the probed Grok argv for the unattended,
  attended, model/effort, resume-by-id and continue combinations.
  `skill_ref(Grok, "delivery-flow", "delivery-flow")` is
  `` `/delivery-flow:delivery-flow` ``.
- **AC-3:** Given real Grok 1.0.77 `summary.json` files in Grok's layout,
  `capture_session(Grok, …)` returns exactly the top-level session's UUID. It
  ignores the subagent session in the same group and rejects older,
  foreign-cwd, mismatched-id and ambiguous sessions.
  `require_captured_session` accepts only the exact id. The encoded group of
  the special-character path equals Grok's observed directory name.
- **AC-4:** Given a real `usage.json`, the token row for a `grok` worker
  carries its six `session` token fields with source `grok usage record`. A
  missing record is `unavailable`.
- **AC-5:** `toolu epic status` resource hosts, the provider-limit host list
  and `toolu epic probe --check-evidence` include `grok`. The committed
  evidence holds the live Grok record (start, tool task, resume, background
  work, cancel/replace, teardown) and validates.
- **AC-6:** Shutting down a started Grok agent on the scripted herdr socket
  sends `ctrl+c` then `/exit` and ends `exited`. The other hosts still send
  `esc` first.
- **AC-7:** `You've hit the credit limit for your plan.` and
  `Error: subscription:free-usage-exhausted` count as provider limits, and
  `You've hit the rate limit for your plan. Upgrade your account or try again
  later.` still does. Ordinary output such as `credit card form validated`
  does not.
- **AC-8:** The local merge gate and the status plan read a `grok` worker's
  ledger from `.claude/tmp/plan-ledger/<branch>.json`. A fresh-green ledger
  there lets the local gate say `merge` instead of `no host recorded`.
- **AC-9:** A live (not dry-run) `launch --kind grok` on the launch rig runs a
  real shell agent in a real checkout's worktree. The agent writes Grok's
  `summary.json` layout for its `$PWD`. The launch records `stage: running`,
  `kind: grok` and the agent's session UUID as `session_id`, and the herdr
  start argv is `grok --always-approve …`.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
| --- | --- | --- | --- | --- |
| AC-1 | `fixtures/epic/epic248-graph.json`, temp `routing.json` | route JSON host `grok`, models per tier, override applied; launch dry-run argv | unknown host error lists grok | `cargo test -p toolu-cli --test epic_graph` and `--test epic_launch` (new Grok cases), `cargo test -p toolu-epic-orchestrator table` |
| AC-2 | `fixtures/epic/hosts.json` new Grok `args`/`skill`/`alias` cases from the probed argv | exact argv | unsafe model refused | `cargo test -p toolu-epic-orchestrator hosts` |
| AC-3 | `fixtures/epic/tokens/grok/sessions/…` trimmed from probe sessions `01a1bd42-396e-7cb3-9e06-4c45a5eda666` and subagent `01a1bd47-d8a4-7a30-ae61-170f1a6f55cd` | the UUID | subagent / old / foreign / mismatched / two sessions; encoded special path | `cargo test -p toolu-epic-orchestrator grok_session` |
| AC-4 | same fixture's `usage.json`, issue record | six token fields | missing record → unavailable | `cargo test -p toolu-epic-orchestrator tokens` |
| AC-5 | committed `epic-hosts-v1.json`; temp resource policy with a `grok` cap; a `rate-limited` failed report from a `grok` worker | `probe-evidence: valid`; status `hosts.items[4].host == "grok"`; the limit cools `grok` | evidence without `hosts.cursor` → `expected keys claude, codex, cursor, grok, muse` (sorted) | `cargo test -p toolu-cli --test epic_probe`, `cargo test -p toolu-epic-orchestrator probe_evidence status_resources limit` |
| AC-6 | scripted herdr socket (`launch_rig`) with a started `grok` agent | call sequence `ctrl+c`, `/exit`, outcome `Exited` | Claude unchanged | `cargo test -p toolu-epic-orchestrator lifecycle` |
| AC-7 | `fixtures/epic/hosts.json` `limit` cases with the Grok strings | match / no match | `credit card form validated` | `cargo test -p toolu-epic-orchestrator hosts` |
| AC-8 | temp worktree with a real branch and a ledger at `.claude/tmp/plan-ledger/` | `ledger_host("grok") == Some(Claude)`; status plan reads it | unknown host → `None` | `cargo test -p toolu-epic-orchestrator hosts status_plan` |
| AC-9 | launch rig (`tests/launch_rig_test.rs`): origin-backed checkout, herdr socket fixture, agent shell script writing `$GROK_HOME/sessions/<encoded $PWD>/<uuid>/summary.json` | record `running`, `kind` `grok`, `session_id` the UUID; start argv | no session written → launch fails `cannot attest grok session identity` | `cargo test -p toolu-epic-orchestrator grok_launch` |

## Documentation impact

- `plugins/epic-orchestrator/README.md`: host list, a Grok row in the flags
  table, Grok routing defaults, probe summary, and the plugin-hook limitation.
- `docs/epic-orchestrator/README.md`: host list and token-source sentence.
- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md` host list.
  `references/worker-brief.md` gets the skill syntax.
  `references/preflight.md` gets a Grok row: `grok` on `PATH` and logged in
  (`$GROK_HOME/auth.json`). Its plugins are the Claude Code ones (`--host
  claude` checks), and SessionStart hooks do not run.
- `status_schema.rs` text (host-pause list, Grok status-probe row, ledger
  directory).
- `--kind`/`--host` help text and `docs/cli/commands.json` via
  `cargo xtask docs-cli`, plus the CLI contract snapshot.
- `fixtures/README.md` and `fixtures/index.json` for the new fixture and
  cases.

## Open Questions

- The dropped first prompt after a resume happened once, right after a
  self-update, and not again with the engine's call shape. If it recurs, the
  engine's acknowledgement wait returns `agent_prompt_stalled`, the launch is
  `uncertain` and `--reprompt` recovers it. The prompt is never silently lost.
  Owner: follow-up. Non-blocking.
- A live end-to-end PR delivered by a Grok worker through the engine needs the
  merged build and Non-Goal 1. Owner: the follow-up issue. Non-blocking.
- The follow-up issue for Grok as a toolu plugin host (Non-Goal 1). Owner:
  user decides whether to file it. Non-blocking.
- Grok self-updates on launch, so the probe crossed versions (1.0.25 →
  1.0.77). The evidence records 1.0.77, the version every later check ran on.
  Non-blocking.

## Spec review

```
Acceptance criteria: 🟡 should-fix: the issue's "a real launch records running with a captured grok session" had no AC. Added AC-9, a live launch on the launch rig with a real agent shell writing Grok's layout. Fixed.
Architecture: 🟡 should-fix: the dropped post-resume prompt was judged from a plain --wait call. Re-probed twice with the engine's own call shape; both submitted. Fixed.
Acceptance criteria: 🔵 consider: AC-1 said "the tier's model". Named the four tier values. Fixed.
Documentation impact: 🔵 consider: `grok models` reports "not authenticated" while prompts work. Preflight uses XAI_API_KEY or auth.json instead. Fixed.
```

Jev (spec review): delivery scope worker-only with follow-up, confidence 1.0;
in-scope requirement coverage 2.52/3, then `none_weak` (top, 0.33) once AC-9
and the AC-1 tier values were added; deferral of hooks-active delivery 0.68.
