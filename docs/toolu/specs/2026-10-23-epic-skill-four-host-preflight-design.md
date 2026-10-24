# Epic-orchestrator skill: preflight and waiting on all four hosts — Design

**Date:** 2026-10-23   **Status:** Approved   **Author:** epic worker (Claude Code)   **Topic:** Falconiere/toolu#605, part of #591

## Problem

The epic-orchestrator skill was written for an orchestrator on Claude Code or
Codex:

- Step 4 ("Wait for judgments") tells only Claude Code and Codex how to wait.
  An orchestrator on Cursor Agent or Muse Code gets no instructions.
- Preflight checks only that each `--hosts` CLI is on `PATH` and logged in. It
  never checks that the worker plugins are installed or that their toolu hooks
  run. On Muse, an installed plugin's hooks stay inert until
  `muse plugins approve <name>`. A worker started before that runs without
  toolu's gates. This happens on this machine right now: toolu 8.2.0 replaced
  8.1.4, its hook hashes changed, and `toolu --host muse doctor` prints
  `warn plugins: toolu capabilities need approval; muse plugins approve toolu`.
  `cursor-agent` exits with `Your macOS login keychain is locked.` while the
  keychain is locked, and preflight misses that too.
- The skill never says that its commands must reach the resident engine. Live
  probes on 2026-10-23 ran one script inside each host's default shell
  sandbox. The script wrote each candidate state root and ran
  `toolu epic status`:

  | Sandbox | State roots (`~/.claude/epics`, `$CODEX_HOME/toolu/epics`, `~/.cursor/…`, `~/.config/muse/…`, resource root) | Engine socket |
  |---|---|---|
  | Muse Code 1.4.12 default | every write `Operation not permitted` | `connect: Operation not permitted` |
  | Codex `-s workspace-write` | every write fails | `connect: Operation not permitted` |
  | Cursor Agent `--sandbox enabled --force` | all writable | reached |
  | Claude Code (no sandbox by default) | all writable | reached |

  Muse's own sandbox text says its writable roots are the workspace, `/tmp`
  and `$TMPDIR`. Moving the state directory therefore cannot make a sandboxed
  orchestrator work: the socket stays blocked. Every orchestrator command
  (`start`, `wait`, `answer`, `ack`, `status`) uses that socket, and `herdr`
  uses its own socket.
- Epic state location: the README says Cursor uses `~/.claude/epics` and Codex
  uses `$CODEX_HOME/toolu/epics`, defaulting to `~/.codex/toolu/epics`. It
  says nothing about Muse. The probe showed that Codex shells do not export
  `CODEX_HOME`, Cursor shells export only `CURSOR_AGENT` (not
  `CURSOR_PROJECT_DIR`), and Muse shells export `MUSE_SESSION_ID`. So
  `epics_home` gives `~/.claude/epics` to every orchestrator, Codex included,
  unless the user exports `CODEX_HOME`. The table is wrong for Codex, Muse is
  missing from it, and no test pins any of this.
- The description, the options and the README call the orchestrator "Claude
  Code, Codex, or Cursor Agent".

The worker brief already names each host's delivery-flow invocation, Muse
included (#607 and #600 render it in `hosts.rs`). This change only verifies
that.

## Non-Goals

1. No new `toolu` verb or flag. Preflight runs commands that already exist:
   `toolu doctor`, `toolu plugins list`, `toolu epic status`, and each host's
   own login command. A new verb would become permanent public surface:
   `check-cli-compat` blocks removing it later (Jev, 0.76 for skill-only).
2. No change to what `toolu doctor` checks or how it exits. Its Muse notice
   (`… need approval; muse plugins approve <name>`, `… hooks are installed but
   inert; …`) already exists.
3. No change to how `epics_home` resolves (Jev, 0.98 for one host-independent
   default). In-flight epics, this one included, live in `~/.claude/epics`.
   Sending Muse, Cursor or Codex shells elsewhere would split resumes. This
   change documents the rule and pins it with tests.
4. No fix for Cursor doctor's retired-marketplace skew failure (#615).
   Preflight reports it as a failing doctor line.
5. No attempt to reach the engine from inside a default host sandbox. The
   skill requires orchestrator commands to run outside it and detects when
   they don't (Jev, 0.9).

## Architecture

All behavior stays in Markdown that runs existing commands. The only Rust
change is in tests and one doc comment.

1. **`SKILL.md` §1 Preflight** gets two parts:
   - *This session.* `toolu doctor`. Then `HERDR_ENV=1` and `herdr status`.
     Then `toolu epic status`, which must not fail with `Operation not
     permitted`. Then the `gh` checks, tracker variables and skills list it
     has today. A per-host line says how to run outside the sandbox: Claude
     Code runs the call with its sandbox disabled; Codex is started with
     `--sandbox danger-full-access`, or its escalation is approved; Cursor
     Agent is started with `--sandbox disabled`; Muse is started with
     `--disable-sandbox`, or its one-time unsandboxed run is approved.
   - *Each worker host in `--hosts`.* Four checks, then one readiness line
     per host:
     1. its CLI is on `PATH` (`claude`, `codex`, `cursor-agent`, `muse`);
     2. it is logged in (table below);
     3. `toolu plugins list --host <host>` lists `toolu`, `toolu-review`,
        `pr-babysit`, `brainstorm` and `delivery-flow` as installed;
     4. `toolu --host <host> doctor` prints no `fail` line and no
        `warn plugins` line. One exception: a `fail skew` line that names
        only plugins outside those five is reported but does not stop the
        host. These are retired plugins, such as `context7` on Claude Code
        and the cached `ast-grep` on Cursor. Their skew failure is #615, and
        their hooks are not the worker's.

     A failed check stops preflight and prints the fix: the login command, or
     the doctor line's own fix text, such as `muse plugins approve toolu`.
2. **`SKILL.md` §4 Wait** covers all four orchestrator hosts. Cursor Agent
   and Muse Code use the Codex foreground loop:
   `toolu --json epic wait --epic <epic-key> --max-seconds 480`, run again
   while any issue is active. The 2026-10-23 probe ran one 480-second
   foreground command to completion in a single call:
   - `cursor-agent -p --force` (2026.10.22): 22:45:32 → 22:53:32, replied
     `LONG DONE after 480 s`;
   - `muse exec` (1.4.12): 22:45:20 → 22:53:20, same reply.

   Muse's shell tool keeps a command that outlives `yield_time_ms` (300000
   at most) running, returns a session id, and delivers the result on its
   own. So §4 tells a Muse orchestrator not to poll that id. Jev chose the
   loop for both hosts (0.59) over background mode on Muse (0.21). The
   loop's only dependency is the observed foreground duration.
3. **`SKILL.md` description and Options** name Claude Code, Codex, Cursor
   Agent and Muse Code.
4. **`plugins/epic-orchestrator/README.md`**:
   - the intro names the four hosts as orchestrator and worker hosts;
   - Install gains a Muse Code section;
   - the waiting paragraph covers the four hosts;
   - the State directory section states the real rule and the sandbox
     requirement.
5. **`docs/epic-orchestrator/README.md`**: the wait comment names the four
   hosts.
6. **`crates/epic-orchestrator/src/refs.rs`**: the `epics_home` doc comment
   states the rule. **`tests/refs_test.rs`** gains a test that feeds the
   variables each host's shell really exports (from the probe) and expects
   the documented directory.

Login checks, verified on this machine on 2026-10-23:

| Host | Check | Logged in | Fix |
|---|---|---|---|
| Claude Code | `claude auth status` | JSON `"loggedIn": true` | `claude auth login` |
| Codex | `codex login status` | `Logged in …` | `codex login` |
| Cursor Agent | `cursor-agent status` | `Logged in as …` | `cursor-agent login`; `Your macOS login keychain is locked` → `security unlock-keychain` |
| Muse Code | auth file (`MUSE_AUTH_PATH`, else `$XDG_CONFIG_HOME/muse/auth.json`, else `~/.config/muse/auth.json`) or `META_API_KEY` | file exists | `muse login` |

## Interfaces / Schema

- No CLI, JSON or config change.
- Epic state (documented and tested):
  - `EPIC_STATE_HOME`, when set, wins.
  - Otherwise `$CODEX_HOME/toolu/epics` when `TOOLU_HOST_OVERRIDE=codex`, or
    when `CODEX_HOME` is set and `CLAUDE_CONFIG_DIR` is not. The override
    alone uses `~/.codex/toolu/epics`.
  - Otherwise `~/.claude/epics`. That covers Claude Code, Cursor Agent, Muse
    Code, and Codex shells, which don't export `CODEX_HOME`.
- Files touched: `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`,
  `plugins/epic-orchestrator/README.md`, `docs/epic-orchestrator/README.md`,
  `crates/epic-orchestrator/src/refs.rs`,
  `crates/epic-orchestrator/src/tests/refs_test.rs`. Possibly
  `tooling/conventions/markdown-cli/epic-orchestrator.json`, if a fenced
  block runs `cursor-agent`, `herdr` or `security`.

## Failure modes and edge cases

- **No `--hosts`:** `toolu epic route` defaults to `claude`, so preflight
  checks only Claude Code.
- **Host CLI missing:** the `PATH` check fails, and preflight names the CLI
  and the host. Its login and doctor checks are skipped.
- **Logged out, or keychain locked:** the login check fails, and preflight
  prints that host's fix. A locked Cursor keychain gets
  `security unlock-keychain`.
- **Worker plugin missing:** a `- <name> not installed` line from
  `toolu plugins list --host <host>` fails the check. The fix is
  `toolu plugins install delivery-flow --host <host>`.
- **`toolu plugins list` itself fails:** found at execution. Inside
  Cursor Agent's shell, `/usr/local/bin` (a root-owned toolu 8.1.3) comes
  before `/opt/homebrew/bin` (8.2.0). The old binary exits 64 with
  `--host cursor is not an installer target`, and a check that only looked
  for `not installed` rows would pass. Step 2 therefore requires the command
  to succeed, and its failure points at the `binary` line of this session's
  doctor.
- **Muse plugin unapproved:** doctor prints `warn plugins: <name> capabilities
  need approval; muse plugins approve <name>`, or `… hooks are installed but
  inert; …`. Doctor still exits 0, so the skill keys on the line, not the exit
  code. The fix is that command.
- **Doctor fails:** preflight stops and prints the line, so no worker is
  routed to a host whose hooks may not run. The exception is a `fail skew`
  that names only plugins outside the five worker plugins. This machine has
  one on Claude Code: retired `context7`, `exa-search` and `jira` at 7.11.0
  lack `hookProtocol`. Cursor has another for its cached `ast-grep`.
  Preflight prints such a line as a note and keeps the host ready. If the
  line names any of the five plugins, the host stops.
- **Sandboxed orchestrator:** `herdr status` or `toolu epic status` fails with
  `Operation not permitted`. Preflight stops with that host's way out of the
  sandbox. With no engine running, `toolu epic status` exits 0 with
  `"engine":"down"`, so `herdr status` (always live in a herdr pane) is the
  check that still catches the sandbox. Later `Operation not permitted` from
  `graph --save` or `start` gets the same fix.
- **`CODEX_HOME` exported, or `TOOLU_HOST_OVERRIDE=codex`:** state goes to the
  Codex directory, as documented. Resuming such an epic from another host
  needs the same environment or `EPIC_STATE_HOME`.

## Acceptance criteria

- **AC-1:** From each of the four hosts (a Claude Code session, `codex exec`,
  `cursor-agent -p` and `muse exec`, each run outside its sandbox), the
  skill's worker-host preflight commands for `--hosts
  claude:1,codex:1,cursor:1,muse:1` print a readiness verdict for every host.
  On this machine, as the 2026-10-23 dry run in this session already showed,
  Claude Code is ready with a retired-plugin skew note and Codex is ready.
  Cursor is not ready, with `toolu plugins install delivery-flow --host
  cursor`. Muse is not ready, with `muse plugins approve toolu`. A Muse Code
  agent given only `SKILL.md` §1 reaches the same Muse verdict.
- **AC-2:** `SKILL.md` §4 gives a waiting procedure for an orchestrator on
  Claude Code, Codex, Cursor Agent and Muse Code. The Cursor and Muse
  procedures match the foreground-duration probe of each host's shell tool.
- **AC-3:** A unit test feeds `epics_home` the variables each host's shell
  exports, plus the `EPIC_STATE_HOME`, `TOOLU_HOST_OVERRIDE` and `CODEX_HOME`
  overrides. Each gets the directory the README documents.
- **AC-4:** The skill's description and Options, and the plugin README's
  intro, install, waiting and state sections, name all four hosts.
  `docs/epic-orchestrator/README.md` names them for waiting.
- **AC-5:** The rendered worker brief names delivery-flow the way each host
  invokes it. The existing `hosts.rs` and launch tests pass unchanged.
- **AC-6:** `cargo xtask check-markdown-cli` passes, and so does the full
  `cargo xtask gate`.
- **AC-7:** Inside Muse Code's default sandbox (`muse exec` without
  `--disable-sandbox`), the skill's session preflight stops. `herdr status`
  fails with `Operation not permitted`, and so does `toolu epic status`
  while an engine runs. `SKILL.md` names the Muse way out
  (`--disable-sandbox`, or an approved unsandboxed run). The README states
  that orchestrator commands need the engine socket and state directory
  outside the host sandbox.

## Acceptance evidence

- **AC-1:** One readiness script per host is saved under
  `/tmp/w605-preflight/` and runs the skill's commands verbatim. It runs
  inside a Claude Code Bash call, `codex exec --dangerously-bypass-approvals-and-sandbox`,
  `cursor-agent -p --force` and `muse exec --disable-sandbox`. Each transcript
  is checked for four readiness lines, with Muse not ready because of
  `muse plugins approve toolu`. `/tmp/w605-preflight/check-evidence.sh`
  asserts those lines and exits non-zero on any gap. To show that the skill
  text itself, not only the mirror script, drives a host agent, one more
  `muse exec --disable-sandbox` run gets only `SKILL.md` §1, with the
  instruction to run its worker-host preflight for
  `claude:1,codex:1,cursor:1,muse:1` and report one verdict per host. Its
  transcript must name `muse plugins approve toolu`, and the same checker
  asserts that. The output goes into the PR body. Boundary case: the mirror
  script under `muse exec` *with* its sandbox prints the sandbox failure.
- **AC-2:** The probe of 2026-10-23 ran one `long.sh 480` foreground call
  under `cursor-agent -p --force` and under `muse exec --disable-sandbox`.
  Both logs show start and end exactly 480 s apart, and both agents replied
  `LONG DONE after 480 s`. That supports a 480-second limit on both hosts.
  Boundary case: Muse's yield above 300 s hands back a session id, and §4
  says not to poll it. Check: §4 has a Cursor Agent and a Muse Code bullet,
  each with `--max-seconds 480` (`grep` in step s2), and the PR body quotes
  the probe.
- **AC-3:** `cargo test -p toolu-epic-orchestrator refs` runs the new
  `epics_home_for_each_orchestrator_host` test. Before the doc and test
  change, the README table contradicts the Codex-shell case.
- **AC-4:** The inputs are the edited Markdown files. Plan steps s2 and s3
  check each named surface with exact `grep`s:
  - the `description:` line names `Muse Code` and `Cursor Agent`;
  - the Options line lists the four host names;
  - the plugin README intro names all four as orchestrator hosts;
  - the README has a `### Muse Code` install section;
  - the waiting paragraph names Cursor Agent and Muse Code;
  - the state-directory section names Muse Code;
  - `docs/epic-orchestrator/README.md` names Muse in its wait comment.

  A missing surface fails the step.
- **AC-5:** `cargo test -p toolu-epic-orchestrator skill_syntax_matches_every_host`
  (the captured skill-syntax cases for all four hosts) and
  `cargo test -p toolu-epic-orchestrator brief` (the rendered-brief tests)
  pass unchanged.
- **AC-6:** `cargo xtask check-markdown-cli`; `toolu epic job -- cargo xtask gate`.
- **AC-7:** The input is `/tmp/w605-preflight/evidence/muse-sandboxed.out`:
  the mirror script run by `muse exec` with Muse's default sandbox, while
  this machine's engine runs. Expected: `FAIL herdr status` and
  `Operation not permitted`. `check-evidence.sh` asserts both. `grep -q
  -- '--disable-sandbox' SKILL.md` and `grep -q 'Operation not permitted'
  README.md` check the documentation (plan steps s2 and s3).

## Documentation impact

- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`: description,
  Options, §1 Preflight and §4 Wait.
- `plugins/epic-orchestrator/README.md`: intro, Install (Muse), waiting
  paragraph, State directory table and sandbox note.
- `docs/epic-orchestrator/README.md`: wait comment.
- The release note comes from the conventional commit (`feat(epic): …`).

## Open Questions

None. The waiting mode for Cursor Agent and Muse Code was settled by the
2026-10-23 probe (Architecture item 2).

## Spec review (2026-10-23)

Round 1 findings, all fixed in this revision:

- Acceptance evidence AC-4: 🟡 should-fix: "read the files" was not a
  runnable check. Replaced with exact per-surface `grep`s in plan steps s2
  and s3.
- Acceptance evidence AC-5: 🔴 blocker: `cargo test … hosts launch` passes
  two filters, which cargo rejects. Replaced with
  `skill_syntax_matches_every_host` and `brief`.
- Acceptance evidence AC-1: 🟡 should-fix: the mirror script alone did not
  show that the skill text drives a host agent. Added a `muse exec` run given
  only `SKILL.md` §1.
- Acceptance criteria: 🟡 should-fix: the sandbox deviation from the issue's
  "writable from inside the sandbox" had no AC of its own. Added AC-7 and
  verified its premise live: `herdr status` inside Muse's default sandbox
  prints `Operation not permitted` and exits 1.
- Failure modes: 🔵 consider: behavior without `--hosts` was unstated. Added
  (`route` defaults to `claude`).

Jev on requirement and evidence alignment: coverage score 1.45 of 2, real
evidence 0.59 after the fixes. AC-7 was added after that call. The remaining
deviation, that orchestrator commands run outside the host sandbox, is
stated in Non-Goal 5 with the probe table as evidence.

**Status:** Approved
