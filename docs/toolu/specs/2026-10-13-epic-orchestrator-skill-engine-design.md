# Orchestrator skill, worker brief and commands for the engine (#452) — Design

**Date:** 2026-10-13   **Status:** Approved   **Author:** epic worker (Claude Code)   **Topic:** the epic-orchestrator Markdown for the resident engine

## Problem

The `epic-orchestrator` skill is 361 lines. It still teaches the engine's
internals and a manual merge gate, so the orchestrator LLM carries hours of
context it does not need and can run commands the engine owns. Workers still
run a `pr-babysit` cron after their PR, although the engine already checks
each PR every 3 minutes and prompts the worker only when there is something
to fix. On Codex and OpenCode nothing says how to wait for hours.

## Non-Goals

1. No engine or CLI change: no new `toolu epic` verb, flag or attention kind.
   The verbs the issue names that the CLI lacks are mapped onto existing ones
   (Architecture). A real `toolu epic preflight` or `stop` is a later CLI issue.
2. No change to the stall clock (45 min, 120 min in `babysit`) or to the
   prompts the engine sends.
3. No change to the authorization boundary or the merge invariant.
4. The pr-babysit skill itself is not changed.

## Architecture

Markdown only, plus the tests and the gate data that hold it. Decided in
`docs/toolu/brainstorms/2026-10-13-epic-orchestrator-skill-engine.md` (Jev
0.99 for Markdown only against 0.01 and 0.00 for adding verbs): the issue's
Goal is a Markdown rewrite, and the Markdown–CLI drift gate fails on any
command the CLI lacks.

| Issue names | The Markdown runs |
|---|---|
| `toolu epic preflight` | `toolu doctor`, then a checklist: `HERDR_ENV`, `herdr status`, `gh auth status`, each host on `PATH`, `gh api rate_limit`, tracker variables, plugins per host |
| `toolu epic start <epic> [--max --hosts]` | `toolu epic graph <epic> --max N --save`, `toolu epic route --graph <state_dir>/graph.json --hosts <spec>`, `toolu epic start <state_dir>` |
| plan / dry run | `toolu epic graph <epic> --dry-run` and `toolu epic route --graph <file> --dry-run`; the dry run stops there |
| `relaunch` | `toolu epic launch --graph … --issue … --reprompt` or `--replace`, in `references/recovery.md` only |
| `toolu epic stop` | `toolu epic pause <epic-key>`, then leave the wait loop; `toolu epic finish <state_dir> <key> --abandon` per issue only when the user asks |

Files:

- `skills/epic-orchestrator/SKILL.md`: inputs and options, authorization
  boundary (wording kept), preflight, plan, run, wait (per host), the judgment
  table, status, stop, resume, guardrails (kept, shortened). "The engine
  merges, not you" appears once. No `gate`, `launch`, `finish` or merge
  command in the normal flow. Under 150 lines.
- `references/worker-brief.md`: report through `toolu epic report`; expensive
  checks through `toolu epic job -- …`; at the babysit handoff the worker runs
  `report babysit --pr <n>` and stops instead of starting the babysit loop;
  the engine prompts it with a tick result (`pr-babysit: … needs you`), with
  `REBASE` only on a conflict, and with `STATUS?`; after acting on a tick it
  pushes, reports `babysit --pr <n>` again and stops. A green tick moves the
  issue to `ready` and queues it; the worker does not report `ready`. The
  rest of the pipeline and "never lose progress" are unchanged.
- `references/recovery.md`: rewritten around `toolu epic status`, the journal
  (`/api/v1/events`, `toolu epic status <key>`) and `toolu doctor`; holds the
  relaunch, the manual gate and finish repairs.
- `commands/epic.md`: `start`, `status`, `pause`, `resume`, `stop`, `serve`,
  each a thin wrapper over the CLI; an epic reference runs the skill.
- `evals/evals.json`: five evals — "work epic #N", "dry-run the epic", "where
  is the epic at", "which issues can run in parallel", "stop".
- `.toolu/skills/verify-epic-watcher/SKILL.md`: engine commands only, plus a
  `toolu epic wait` step.
- Tests: `crates/cli/tests/epic_markdown.rs` (surfaces include the
  references; line limit; no engine-owned command in the skill; the rendered
  brief's babysit stop), `crates/epic-orchestrator/src/launch/tests/brief_test.rs`,
  `crates/xtask/src/context_budget.rs` (+ its test) for the skill description,
  `tools/toolu-opencode/scripts/__tests__/epic-surfaces.test.ts` and the
  OpenCode port map plus the regenerated tree.

Per-host waiting: Claude Code runs `toolu --json epic wait` as a background
Bash call (`run_in_background`, default limit 2700 s) and ends its turn; the
call's completion wakes it. Codex and OpenCode run
`toolu --json epic wait --max-seconds 480` in the foreground and run it again
until no issue is active. The OpenCode port replaces the Claude paragraph, so
the generated skill keeps none of `run_in_background` or "end your turn".

## Interfaces / Schema

No CLI or file-format change. Markdown contracts:

- SKILL.md body (after the front matter) is at most 149 lines.
- SKILL.md names only these `toolu epic` verbs: `graph`, `route`, `start`,
  `wait`, `status`, `queue`, `answer`, `ack`, `release`, `pause`, `resume`,
  and `toolu doctor`. The engine closes the epic; a manual
  `toolu epic close` lives in `references/recovery.md`. It names no `toolu epic gate`, `launch`, `finish` or
  `gh pr merge`.
- The judgment table lists the attention kinds the engine raises:
  `needs-human`, `failed`, `blocked`, `stall`, `babysit`, `coupling-hold`,
  `relaunch-limit`, `reopened`, `pr-closed`, `rebase`, `cleanup-incomplete`,
  `refresh-failed`, `herdr-error`, `dependency-cycle`, `epic-complete`,
  `merge-ready` (test seam), each with one command.
- Context budget row: `epic-orchestrator`, `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`,
  a ceiling of 80 description words with the trigger phrases `epic`,
  `dry-run the epic` and `where is the epic at` (today's description is
  about 150 words).

## Failure modes and edge cases

- **Engine not running** when the orchestrator waits: `toolu epic wait` prints
  `{"state": "waiting"}` at once. The skill says to run `toolu epic status`
  and `toolu epic start <state_dir>` again instead of looping.
- **Wait returns `waiting`** at the limit: run it again (Claude: another
  background call).
- **A Claude background wait outlives the turn and the user speaks**: the
  orchestrator answers and keeps the one background wait; it never starts a
  second wait while one runs.
- **Worker idle in `babysit` past 120 min:** the engine sends one `STATUS?`;
  the worker answers in one line and stays idle. Re-reporting `babysit` after
  each fix resets the clock.
- **Tick prompt arrives while a queue rebase runs:** the engine holds it
  (`babysit_next::hold`); the brief needs no rule.
- **Drift:** any `toolu` command the CLI lacks fails `cargo xtask
  check-markdown-cli`; an OpenCode anchor that moved fails generation.
- **Dry run on a closed epic:** the graph reports `complete`; the skill stops
  with "nothing to launch".

## Acceptance criteria

- **AC-1:** Given the shipped `SKILL.md`, its body is under 150 lines and its
  description passes the context-budget gate's new `epic-orchestrator` row.
- **AC-2:** Given every `toolu` command in the skill, the brief, the command
  file and both references, `cargo xtask check-markdown-cli` passes and every
  `toolu epic <verb>` they name is a verb of the real binary.
- **AC-3:** Given each host (`claude`, `codex`, `cursor`, `opencode`) and each
  tracker (GitHub, Jira, Linear), the brief `toolu epic launch --dry-run`
  renders has no placeholder, reports through `toolu epic report`, runs jobs
  through `toolu epic job --`, and tells the worker to stop after
  `report babysit --pr <n>` with no cron or polling loop.
- **AC-4:** Given the skill, the normal flow names no `toolu epic gate`,
  `launch`, `finish` or merge command; it previews with `graph --dry-run` and
  `route --dry-run`, starts with `graph --save`, `route`, `start`, and its
  judgment table pairs each engine attention kind with a command.
- **AC-5:** On Claude Code, `toolu --json epic wait` run as a background Bash
  call against a real isolated engine returns the judgment raised after it
  started. On Codex and OpenCode, a real session running the skill's bounded
  `--max-seconds` loop sees an event that arrives after several iterations.
- **AC-6:** The OpenCode generated skill, references and command carry no
  Claude-only step (`run_in_background`, "end your turn") and describe the
  bounded loop; generation and its drift check pass.
- **AC-7:** Given `/epic start|status|pause|resume|stop|serve`, the shipped
  command file gives each one CLI command (`stop` = `toolu epic pause
  <epic-key>`), and an epic reference runs the skill.
- **AC-8:** Given a failure in `references/recovery.md`, its first step is
  `toolu epic status <key>`, the journal (`/api/v1/events`) or
  `toolu doctor`, and every command in it is a real `toolu` command.
- **AC-9:** Given `.toolu/skills/verify-epic-watcher`, its procedure runs only
  `toolu epic` and `herdr` commands (including `toolu epic wait`) and names no
  watcher script (`epic-watch`, `.ts`).
- **AC-10:** Given the five eval prompts, each run by a fresh subagent that
  reads the shipped SKILL.md, the read-only ones ("dry-run the epic" on #402,
  "which issues can run in parallel" on #402, "where is the epic at") and
  "stop" (with `TOOLU_RESOURCE_HOME` pointing at an isolated engine, the real
  one's pause state unchanged) meet their assertions.

## Acceptance evidence

| AC | Real input | Expected | Check |
|---|---|---|---|
| AC-1 | shipped SKILL.md | ≤149 body lines; budget row green | `cargo test -p toolu-cli --test epic_markdown`; `cargo xtask gate --only context-budget`; `cargo test -p xtask context_budget` |
| AC-2 | plugin Markdown | no drift | `cargo xtask check-markdown-cli`; `epic_markdown` verb test (now over references) |
| AC-3 | `fixtures` graph via the real binary | brief per host × tracker | `cargo test -p toolu-cli --test epic_markdown`; `cargo test -p toolu-epic-orchestrator brief` |
| AC-4 | shipped SKILL.md | banned verbs absent, table complete | `epic_markdown` tests |
| AC-5 | isolated `TOOLU_RESOURCE_HOME`, real engine, a `needs-human` report sent after the waiter's 3rd return | wait returns that item; earlier iterations return `waiting` | this session's background Bash; a `codex exec` and an `opencode run` session (the installed CLIs; versions recorded) running a short `--max-seconds` loop; record in `docs/toolu/evidence/2026-10-13-epic-wait-hosts.md` |
| AC-6 | generated tree | banned tokens absent | `bun run generate:opencode-surface`, `bun run check:opencode-surface`, `bun test tools/toolu-opencode/scripts/__tests__/epic-surfaces.test.ts` |
| AC-7 | `commands/epic.md` | six subcommands | `epic_markdown` test |
| AC-8, AC-9 | the files | commands present, no script | `epic_markdown` (recovery), review; `cargo xtask check-markdown-cli` does not read `.toolu/`, so review |
| AC-10 | Epic #402 (read-only GitHub), isolated engine for status and stop | each assertion met; no write outside the isolated root | one subagent per eval with the prompt verbatim; transcript excerpts and grades in the evidence file; `toolu epic status` of the real engine before and after |

## Documentation impact

`plugins/epic-orchestrator/README.md` (waiting per host, workers idle after
babysit, the command's subcommands) and `docs/epic-orchestrator/README.md`
(the quick-start wait line). `docs/cli/` is unchanged (no CLI change).

## Open Questions

1. **The live two-issue sandbox epic on Claude Code** (issue acceptance). It
   needs a GitHub sandbox repository with two issues and merges; this worker is
   authorized for this issue's branch and PR only. Owner: orchestrator/user.
   Non-blocking for this PR; the PR says it was not run and why.
2. **The "work epic #N" eval** launches agents; it runs with Open Question 1.
   Non-blocking.

## Spec review (round 1)

- Interfaces: 🟡 should-fix: `close` listed conditionally. Fixed: the engine closes; manual close only in recovery.
- Interfaces: 🟡 should-fix: the budget row had no number. Fixed: 80 description words.
- Acceptance criteria: 🟡 should-fix: AC-7–AC-10 read as activities (Jev noul 0.23 on observability). Fixed: each is given/then on a real input.
- Acceptance evidence: 🟡 should-fix: eval runs had no method. Fixed: one subagent per eval, isolated engine for stop, real engine checked before and after.
- Acceptance evidence: 🟡 should-fix: the installed OpenCode is 2.0.21, not the 1.x the plugin targets; the wait loop is plain bash, so the version is recorded rather than pinned.
- Open Questions: both owned and non-blocking (Jev coverage score 2.24, mode 3 at 0.56).

**Status:** Approved
