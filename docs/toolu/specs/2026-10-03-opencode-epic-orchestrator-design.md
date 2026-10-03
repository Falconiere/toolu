# OpenCode: epic-orchestrator native agent routing and worktrees — Design

**Date:** 2026-10-03   **Status:** Approved   **Author:** Cursor agent (epic worker, #356)   **Topic:** Make an OpenCode worker launched by the epic orchestrator start on a supported host with a brief whose skills exist, keep toolu's runtime state out of its worktree's commits and snapshots, and give an OpenCode orchestrator a skill with no Claude-only steps (OP-22)

## Problem

The substrate (#337–#345) and the delivery-chain port (#355) make delivery-flow, brainstorm, toolu-review and pr-babysit load on OpenCode. The epic orchestrator still assumes Claude-shaped hosts. On `origin/main` (f98f013b):

- **Worker brief names skills that do not exist.** `scripts/hosts.ts` renders an OpenCode skill as "the `delivery-flow--delivery-flow` skill". #344 replaced double-hyphen ids. The generated ids are `delivery-flow-delivery-flow`, `pr-babysit-babysit-73c340c6` and `toolu-debug`. A Jira brief tells an OpenCode worker to run a bare `jira.sh`, which is not on its PATH.
- **No guard on the OpenCode release.** herdr types the canonical `opencode` into the pane. toolu's `@toolu/opencode` targets `opencode-ai` 1.x (pinned 1.18.34). On this machine `opencode` is v2.0.21, a different CLI with no `--model` flag and a different plugin API. A worker there would exit at once or run without toolu's gates and skills, and nothing would say why.
- **Routing accepts model ids OpenCode cannot use.** `--model sonnet --kind opencode` would reach `opencode --model sonnet`. The pinned TUI needs `provider/model` and has no `--variant` flag. The README and a test still advertise `provider/model#variant`.
- **Worker runtime state pollutes the worktree.** On OpenCode, toolu writes its per-project data root to `<worktree>/.opencode/toolu/state/` (registry bundles, helper symlinks, startup ledger). Gate state, the plan ledger and telemetry go to `<worktree>/.opencode/tmp/`. In an epic worktree these are untracked files. As a result:
  - every 15-minute snapshot captures them, so `refs/epic-wip/<key>` churns;
  - `finish-issue.ts` lists them as leftovers;
  - a non-forced `--abandon` removal is refused;
  - a worker doing `git add -A` would commit them.
- **Helper lookup misses OpenCode.** `route.ts` looks for `jev.sh`, and `trackers/jira.ts` for `jira.sh`, only under `CLAUDE_CONFIG_DIR` and `CODEX_HOME`. OpenCode publishes both under `TOOLU_CONFIG_DIR`, which its `shell.env` sets. On a pure OpenCode install, routing silently falls back to heuristic tiers and Jira epics fail with "jira.sh not found".
- **The generated orchestrator skill keeps Claude-only steps:**
  - it runs the watcher with Claude's `run_in_background: true`, ends the turn and waits to be "re-invoked" (OpenCode's bash has no background mode, and nothing re-invokes the session);
  - it resolves `ROOT` through `CLAUDE_PLUGIN_ROOT`/`PLUGIN_ROOT` fallbacks;
  - it names `delivery-flow:delivery-flow` and `npx @toolu/plugins install … --host codex`.
- **The capability matrix is inaccurate.** It says epic-orchestrator routes workers "to OpenCode agents per sub-issue" through `.opencode/agents` and the `task` tool. Workers are herdr sessions in their own worktrees.

Already correct, verified on the pinned host: the TUI accepts `--auto`, `-m provider/model`, `--continue`, `--agent` and `/exit`. `--continue` in worktree A resumes A's session even after worktree B ran (live probe, session ids `ses_…S` → `ses_…S`). OpenCode `task` calls already reach agent-tier as `Task` with `subagent_type` (#337/#338). With no selection file, OpenCode enables every installed plugin.

## Non-Goals

1. Replacing herdr dispatch with OpenCode `task` subagents. Workers keep one herdr worktree and one interactive agent per sub-issue (Jev choice 0.99: isolation, parallelism and recovery need separate worktrees and sessions).
2. Changing Claude Code, Codex or Cursor worker launches, briefs or state. Only OpenCode launches gain the version check, the model-format check and the worktree exclude.
3. Changing toolu's OpenCode state roots or making core state writers self-ignore. That is cross-host core behavior (OP-09). This issue makes epic worktrees deterministic.
4. Remapping agent-tier's model-mismatch rule to OpenCode agent names. The adapter passes a native `task`'s optional `model` to agent-tier as is (#337). A call without one inherits the tier and is recorded, never flagged, as decided in #355.
5. pr-babysit's cron and fixer dispatch on OpenCode (OP-23, #357), CLI management (OP-26), packaging (OP-27) and mandatory CI acceptance (OP-28).
6. Moving the OpenCode epics state home (`$TOOLU_OPENCODE_HOME/toolu/epics`, else `~/.opencode/toolu/epics`). Moving it would orphan existing runs.

## Architecture

Decisions checked with Jev:
- keep herdr dispatch (choice 0.99);
- for OpenCode launches only, the launcher adds toolu's runtime paths to the checkout's shared `info/exclude` (choice 0.97, over self-ignoring core writers or docs only);
- run the watcher in the foreground with a bounded wait on OpenCode (choice 0.98);
- a launch-time `opencode --version` check is justified (noul 0.76).

1. **`scripts/hosts.ts`.**
   - The OpenCode `skill` renders `` `skill({ name: "<id>" })` `` from an explicit map: `delivery-flow:delivery-flow` → `delivery-flow-delivery-flow`, `pr-babysit:babysit` → `pr-babysit-babysit-73c340c6`, `toolu:debug` → `toolu-debug`. An unmapped pair throws, so a wrong id can never be rendered silently.
   - `agentArgs("opencode", …)` throws when a model is not `provider/model`.
   - A pure `opencodeVersionProblem(text)` returns `null` for a 1.x `--version` line, else the refusal text.
2. **`scripts/launch-issue.ts`.** For an OpenCode launch:
   - Before any clone, gh, herdr or state write: refuse when the model is not `provider/model`, or when `opencode --version` cannot run or is not 1.x (non-dry runs only).
   - After the worktree exists and before `agent start`: idempotently append `/.opencode/toolu/state/` and `/.opencode/tmp/` under a marker line to `<git-common-dir>/info/exclude` of the checkout. Linked worktrees read that file. Dry-run logs the step and writes nothing.
   - The Jira issue-read line becomes `"$TOOLU_BUN" --no-env-file "$TOOLU_CONFIG_DIR/jira/jira.sh" issue get KEY` (from `skill({ name: "jira-jira" })`), matching #351's generated skill.
   - `START_PROMPT` is exported so the live fixture sends the launcher's exact prompt.
3. **`scripts/common.ts`.** `helperCandidates(rel, env)` returns `TOOLU_CONFIG_DIR/rel` (the published root on every host, and OpenCode's data root), then `CLAUDE_CONFIG_DIR` (else `~/.claude`), then `CODEX_HOME` (else `~/.codex`). `jevScript` (route.ts) and `jiraScript` (trackers/jira.ts) use it after their `EPIC_*` override.
4. **Source skill and brief** (all orchestrator hosts launch OpenCode workers):
   - `SKILL.md` preflight says `opencode --version` must be 1.x, and that an OpenCode worker reads its worktree's own selection: a committed `.opencode/toolu/plugins.json`, else the global one, else every installed plugin.
   - Launch step 4 states the OpenCode version check and exclude.
   - The `worker-brief.md` maintainer comment names the new OpenCode form.
5. **OpenCode port entries** (`tools/toolu-opencode/scripts/lib/opencode-port.ts`) for `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md`:
   - `ROOT` comes from `TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR` alone, failing with a message when unset.
   - The skill-list preflight names the four generated skill ids and drops the Codex install command.
   - The watch section runs `epic-watch.ts --max-wait 480` in the foreground with the bash tool's `timeout` at `600000` (the pinned bash tool's maximum; default 120000). It handles events and restarts without ending the turn while issues are active. It re-runs the graph on a heartbeat only about every 45 minutes.
   - The section header and the Stop line lose "background" and "watcher task".
6. **Capability matrix.** epic-orchestrator's `task` axis says what happens: each worker is an OpenCode session in its own herdr worktree, and its delegation goes through `task`. Its evidence stays `deny.task-child` and `surface.files`. `docs/opencode-host-contract.md` is regenerated.
7. **Regenerate** `tools/toolu-opencode/generated/` and the plugin bundles if any change (none expected: no `hooks/src` edit).

Reused:
- the exact-anchor port table and the surface audit helpers (`surface-audit.ts`);
- `createTooluHooks`, `shell.env` and the jev-fixture bindings;
- agent-tier telemetry;
- `checkpoint.ts`, `epic-watch.ts` and `report.ts` as shipped;
- the pinned-host harness (`openSession`, `runHost` with `cwd`, the scripted provider);
- `@toolu/conformance` sandboxes.

## Interfaces / Schema

- `hosts.ts`:
  - `export const OPENCODE_SKILL_IDS: Readonly<Record<string, string>>`, keyed `plugin:skill`;
  - `skillRef("opencode", "delivery-flow", "delivery-flow")` → `` `skill({ name: "delivery-flow-delivery-flow" })` ``;
  - `skillRef("opencode", "x", "y")` throws `no OpenCode skill id for x:y`;
  - `export function opencodeVersionProblem(versionOutput: string): string | null`, where `null` means a 1.x release.
- `agentArgs("opencode", { model: "sonnet", … })` throws `OpenCode model must be provider/model, got sonnet`.
- `launch-issue.ts`:
  - `export const START_PROMPT`;
  - `export const OPENCODE_EXCLUDE = ["/.opencode/toolu/state/", "/.opencode/tmp/"]`;
  - `export async function excludeOpencodeState(checkout: string): Promise<string>`, returning the exclude path. The file gets `# toolu epic-orchestrator: OpenCode worker runtime state` once, then each missing line once.
- Launch refusal (exit 1, stderr JSON from the existing `CommandError` path), with no clone, gh, herdr or state write:
  - `{"issue":"<ref>","error":"CommandError: opencode --version reports \"2.0.21\"; toolu's OpenCode plugin targets opencode-ai 1.x (tested on 1.18.34). Put a 1.x opencode first on PATH, or route this issue to another host."}`;
  - `opencode is not runnable here (<reason>); …` when it is missing.
- Dry-run log line: `# opencode: exclude /.opencode/toolu/state/ /.opencode/tmp/ in <checkout>'s info/exclude`.
- `common.ts`: `export function helperCandidates(rel: string, env?: NodeJS.ProcessEnv): string[]`.
- Generated OpenCode skill: the watcher line `bun "$S/epic-watch.ts" --state-dir <state_dir> --max-wait 480` with bash `timeout: 600000`; root line `ROOT="${TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR:?epic-orchestrator is not enabled in this OpenCode session}"`.

## Failure modes and edge cases

- **`opencode` missing, or not 1.x:** launch exits 1 before any side effect; no `issues/<key>.json`; the orchestrator reroutes (`--kind`) or fixes PATH. Dry-run skips the binary check (it executes nothing) but still rejects a bad model.
- **Model not `provider/model`:** refused before any side effect, dry or not. No model at all keeps OpenCode's configured model, as today.
- **`info/exclude` absent, or `info/` absent:** created. File without a trailing newline: a newline is added before the block. Lines already present (including from an earlier launch): nothing appended. Unwritable: `CommandError`, launch exits 1 before `agent start`; the worktree stays and a re-run reuses it.
- **User-owned `.opencode` files** (`plugins.json`, `toolu.config.json`, agents, commands): not excluded; they stay visible to git.
- **Data root overridden** (`TOOLU_OPENCODE_HOME`/`TOOLU_CONFIG_DIR`): state lives elsewhere; the exclude is harmless.
- **Excluded state and teardown:** snapshots, leftovers and non-forced `git worktree remove` all ignore excluded files. Verified: a worktree with only excluded state removes without `--force`.
- **Unknown skill pair in `skillRef`:** throws at render, so the launch fails loudly.
- **A generated skill id changes** (e.g. OP-23 renames pr-babysit's): the hosts test that cross-checks `OPENCODE_SKILL_IDS` against `generated/skills/` fails.
- **Helper not published anywhere:** `jevScript` returns null (heuristic tiers, noted as today); `jiraScript` returns null ("jira.sh not found", as today).
- **`TOOLU_CONFIG_DIR` set on Claude Code or Codex** without a helper there: the next candidate is used; behavior matches the skills' published-path rule.
- **Watcher on OpenCode:** bounded at 480 s plus one interval, under the 600 s bash cap. A second watcher still gets `watcher-busy`. A killed bash call releases the lock through the signal handler, or a dead pid is taken over.
- **Worker cancelled mid-turn** (process killed): status keeps the last reported phase. The next checkpoint snapshots the uncommitted edit without toolu's state. A relaunch with `--continue` in that worktree resumes that worktree's session, not a sibling's.
- **Worker reports `failed` with a `rate-limited:` note:** the watcher emits `failed`, puts the `opencode` host on cooldown and emits `host-limited` with `host: "opencode"`.
- **Port anchor moved or repeated:** generation throws naming the source path and anchor.

## Acceptance criteria

- **AC-1:** `skillRef("opencode", …)` for each brief placeholder returns `skill({ name })` with an id that is a generated skill directory whose frontmatter name matches. A rendered OpenCode brief contains no double-hyphen skill id. It also contains none of `/delivery-flow:`, `$delivery-flow:`, `/pr-babysit:`, `$pr-babysit:`, `/toolu:` or `$toolu:`. Its Jira issue-read line is the `"$TOOLU_BUN" --no-env-file "$TOOLU_CONFIG_DIR/jira/jira.sh"` command. An unmapped pair throws.
- **AC-2:** `launch-issue.ts --dry-run --kind opencode --model probe/scripted` on the #248 fixture prints `herdr agent start <key> --kind opencode --pane '<root-pane>' --timeout 90000 -- --auto --model probe/scripted`, the exclude log line, and a brief with `skill({ name: "delivery-flow-delivery-flow" })`. The same run with `--model sonnet` exits 1 with the provider/model message. The existing Claude, Codex and Cursor dry-run tests pass with their expectations untouched, and their output has no exclude line.
- **AC-3:** A non-dry `--kind opencode` launch with no `opencode` on PATH exits 1 naming opencode, and writes no issue record. `opencodeVersionProblem` returns null for `1.18.34` and the refusal for `2.0.21` and for empty output. The pinned `opencode-ai@1.18.34` binary's real `--version` output passes (live).
- **AC-4:** In a real repo with a linked worktree holding `.opencode/toolu/state/x`, `.opencode/tmp/y`, an untracked `.opencode/toolu/plugins.json` and a source edit, `excludeOpencodeState` twice leaves exactly one block in `info/exclude`. Afterwards:
  - `git status --porcelain` lists only `plugins.json` and the edit;
  - a real `checkpoint.ts` snapshot tree contains the edit and neither state path;
  - with only excluded state left, a non-forced `git worktree remove` succeeds.
- **AC-5:** `helperCandidates("jev/jev.sh", env)` lists the `TOOLU_CONFIG_DIR` path first. `jevScript` and `jiraScript` return a helper published only under `TOOLU_CONFIG_DIR`, and still find `CLAUDE_CONFIG_DIR`/`CODEX_HOME` ones.
- **AC-6:** The generated epic-orchestrator `SKILL.md`, its references and the `epic` command name none of:
  - `run_in_background`, `end your turn`, `re-invoked`;
  - `CLAUDE_PLUGIN_ROOT`, `${PLUGIN_ROOT`;
  - `npx @toolu/plugins`;
  - `delivery-flow:delivery-flow`, `pr-babysit:babysit`, `toolu:`;
  - a double-hyphen skill id.

  Every `skill({ name })` names a generated skill, and every `"$S/<script>"` exists in a `stagePlugins` copy of epic-orchestrator. The skill carries the foreground watcher line, `timeout` `600000`, and the `TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR:?` root. Generation fails naming the source and anchor when an epic port anchor is removed or duplicated.
- **AC-7:** Through `createTooluHooks` in a sandbox project selecting only `epic-orchestrator`:
  - the `config` hook registers `epic-orchestrator-epic-orchestrator` and the delivery chain's four skills;
  - the generated skill's `ROOT`/`S` lines run in the `shell.env` bash, and `bun "$S/report.ts" <file> execution` writes the status file;
  - a native `task` call (`subagent_type: "toolu-quick-task"`) passes `tool.execute.before` and appends one `delegation` telemetry line with that `subagent_type` under `.opencode/tmp/telemetry/`;
  - with `gates.agentTier.mode: block` and a plan ledger whose running step declares `model: haiku`, the agent-tier policy refuses a native `task` carrying `model: "opus"` before it runs, naming the step. The same call without a model passes and records `step_id`.
- **AC-8:** Failure and cancellation reach the orchestrator for an `opencode` issue record:
  - a worker that reports `failed --note "rate-limited: 429 Too Many Requests"` makes `limitEvents` emit `host-limited` with `host: "opencode"` and `cooldown: true`, and write an `opencode` entry to `hosts.json`;
  - `issueEvents` emits `failed` with the note;
  - for a running record whose agent is absent from the live herdr agent list (a cancelled or crashed worker), `issueEvents` emits `gone` once.
- **AC-9:** Live, gated on `TOOLU_LIVE_OPENCODE=1`. On the pinned host with the scripted provider, an isolated epic fixture runs a real OpenCode worker in a linked worktree, launched with the launcher's own `agentArgs("opencode")` flags and `START_PROMPT`. The fixture is a git repo with a bare remote, a committed `epic-orchestrator` selection, a state dir, a brief rendered by `renderBrief(…, "opencode")`, and `excludeOpencodeState` applied. Toolu is loaded through the global config's `file://` plugin. The worker:
  - reads the brief;
  - reports `execution`;
  - loads `delivery-flow-delivery-flow`;
  - delegates once through `task` to `toolu-quick-task`;
  - writes a bounded change;
  - is killed mid-turn.

  The killed host run exits non-zero, and the status file still holds `execution`. Then a real `checkpoint.ts` snapshots the edit without toolu's state. A `--continue` run in the same worktree resumes the same session id, commits, records the review, pushes and reports `ready --pr 1`. Assertions:
  - the remote branch head equals the worktree head;
  - the status file's last phase is `ready`;
  - `epic-watch.ts --peek` emits `ready` for the key;
  - delegation telemetry names `toolu-quick-task`;
  - `git status --porcelain` in the worktree lists no path under `.opencode/toolu/state/` or `.opencode/tmp/`;
  - the observed porcelain is recorded in the PR evidence (OpenCode's own config-directory install is not toolu's to hide).
- **AC-10:** Existing hosts are intact. Every existing epic-orchestrator test passes, with only the OpenCode skill-syntax and model expectations updated. Claude, Codex and Cursor `agentArgs`/`skillRef` outputs are unchanged, and `bun run test` passes.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | `renderBrief` on the #248 fixture, GitHub and Jira, kind `opencode`; `generated/skills/` | ids resolve; no `--`/`/`/`$` forms; Jira command | unmapped pair throws | `bun test plugins/epic-orchestrator/scripts/__tests__/hosts.test.ts plugins/epic-orchestrator/scripts/__tests__/launch-issue.test.ts` |
| AC-2 | `launch-issue.ts --dry-run` on the #248 fixture in a sandbox state dir | exact start line, exclude line, brief | `--model sonnet` refused | `launch-issue.test.ts` |
| AC-3 | Non-dry launch with `PATH` = Bun's dir only; version strings; pinned binary | refusal, no record; parse results | empty output | `launch-issue.test.ts`, `hosts.test.ts`, live AC-9 |
| AC-4 | Sandbox repo + `git worktree add` | porcelain, snapshot tree, remove as stated | idempotent second call; no trailing newline | `bun test plugins/epic-orchestrator/scripts/__tests__/worktree-exclude.test.ts` |
| AC-5 | Sandbox dirs holding real `jev/jev.sh` / `jira/jira.sh` files | lookup order as stated | helper only in one root | `route.test.ts`, `jira-tracker.test.ts` |
| AC-6 | Committed `generated/`; `stagePlugins` copy; sandbox copy with an anchor edited | no banned token; ids/scripts resolve; generation throws | 0 and 2 matches | `bun test tools/toolu-opencode/scripts/__tests__/epic-surfaces.test.ts` |
| AC-7 | Sandbox git project, `createTooluHooks`, `shell.env` bash, a real plan ledger file | registration, status file, telemetry line, policy refusal | only `epic-orchestrator` selected (closure); task with and without `model` | `bun test tools/toolu-opencode/src/plugin/__tests__/epic-workflows.test.ts` |
| AC-8 | Sandbox state dir, `opencode` record and status, agent maps with and without the worker | `host-limited`, `failed`, `gone` events and `hosts.json` entry | `gone` once, not repeated | `epic-watch.test.ts` |
| AC-9 | Pinned `opencode-ai@1.18.34`, scripted provider, two worktree runs | as stated | mid-turn kill; resume by worktree | `TOOLU_LIVE_OPENCODE=1 bun test tools/toolu-opencode/src/plugin/__tests__/epic-worker.live.test.ts` |
| AC-10 | Existing suites | pass | — | `bun test plugins/epic-orchestrator` and `bun run test` |

## Documentation impact

- `plugins/epic-orchestrator/README.md`:
  - the OpenCode section covers workers: version 1.x, `provider/model`, the worktree exclude, and selection resolution in worktrees;
  - the OpenCode orchestrator runs the watcher in the foreground;
  - the hosts table drops `#variant`.
- `plugins/epic-orchestrator/skills/epic-orchestrator/SKILL.md` (source): the preflight and launch-step notes above.
- `docs/opencode.md`: an "Epic orchestrator" subsection.
- `docs/epic-orchestrator/README.md`: an OpenCode line.
- `docs/opencode-host-contract.md`: regenerated from the corrected matrix.
- Regenerated `generated/` copies and notes.

## Spec review

- Acceptance criteria: 🟡 should-fix (resolved). Issue criterion 2 (Jev 1.64) had no orchestrator-side signal for a cancelled worker. AC-8 now covers `gone` and `failed`, and AC-9 checks the killed run's exit and status.
- Acceptance criteria: 🟡 should-fix (resolved). Issue criterion 3 (Jev 1.75) never exercised an agent-tier decision. AC-7 now refuses a mismatched native `task` under `agentTier: block`.
- Acceptance criteria: 🟡 should-fix (resolved). AC-9's "porcelain is empty" depended on OpenCode's own config-dir install. It now asserts no toolu runtime path and records the observed output.
- Acceptance criteria: 🟡 should-fix (resolved). AC-2's "byte-identical" had no baseline, and AC-1's "no `/` or `$`" was ambiguous. Both now name concrete checks.
- Coverage after revision (Jev): issue criteria 1.89 / 1.75 / 1.81 of 2. The rest is real herdr in CI, which no test here can supply. herdr dispatch is covered by the exact dry-run commands and the existing herdr contract, and the worker by the live run.
- Non-Goal 4: 🟡 should-fix (resolved). It claimed OpenCode `task` takes no model, which contradicts the adapter (`tool-before.ts` forwards `model`) and the revised AC-7.
- No blockers. **Status:** Approved.

## Open Questions

- None blocking. The exclude covers only OpenCode launches. Claude and Codex workers' `.claude/tmp/` and `.codex/tmp/` have the same untracked-state shape; that change belongs to a cross-host follow-up, not this issue (Non-Goal 2).
