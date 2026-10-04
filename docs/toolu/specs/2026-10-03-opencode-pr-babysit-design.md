# OpenCode: pr-babysit native fixer dispatch and lifecycle — Design

**Date:** 2026-10-03   **Status:** Approved   **Author:** Claude Code (epic worker, #357)   **Topic:** Port pr-babysit's controller, fixer routing, fixer dispatch and lifecycle to OpenCode (OP-23)

## Problem

The shared substrate (#337–#345, #358) loads pr-babysit's generated skill and command on OpenCode and exports `TOOLU_PLUGIN_ROOT_PR_BABYSIT`, `TOOLU_BUN`, `TOOLU_CONFIG_DIR` and `TOOLU_USER_CONFIG_DIR` to bash. pr-babysit itself is still Claude Code and Codex only:

- **The generated skill is Codex text.** `generated/skills/pr-babysit-babysit-73c340c6/SKILL.md` tells the model to use `get_goal`/`create_goal`, `--host codex` and `../../hooks/dist/babysit-tick.js` (no such file under `generated/`). The generated workflow resource keeps Claude cron, Codex goal and `EnterWorktree` branches and no OpenCode controller.
- **No continuation.** OpenCode's plugin API has no cron or goal primitive (capability-matrix note owned by OP-23).
- **Routing rejects OpenCode.** `babysit-route-fix.js --host opencode` exits 2; `hostKind("opencode")` is `config_invalid`; config is read from Claude or Codex roots, and Jev from `~/.claude` or `~/.codex`, never from the OpenCode data root.
- **No OpenCode fixer.** Fixers run only as Claude Code, Codex or Cursor TUIs in herdr panes. herdr types arguments into a pane shell, so nothing can limit a fixer's permissions, and herdr does not exist in CI.
- **Broken npm install.** `@toolu/opencode` stages `hooks/dist/*.js` but not `skills/babysit/references/fixer-brief.md`, which `babysit-dispatch-fix.js start` reads, so `start` crashes on an npm install.

## Non-Goals

1. Changing Claude Code or Codex controllers, their herdr fixers, the tick collector, the reducer, or the reply/resolve/record helpers' GitHub requests.
2. A plugin-side scheduler (a `session.idle` re-prompt in the toolu adapter). The core adapter stays free of leaf-plugin code (recorded decision 5a002d41).
3. Running OpenCode fixers through herdr panes.
4. Real GitHub traffic in tests. Evidence uses an isolated git repository with a local bare `origin`; GitHub writes are covered by the existing write-side tests.
5. A new runtime state gate, or a new host-contract probe id.

## Architecture

Decisions checked with Jev: continuation — in-turn bounded loop 1.00 (session.idle re-prompt 0.00 once the adapter rule was supplied); fixer transport — `opencode run` subprocess 1.00 (herdr pane 0.00).

1. **OpenCode controller (in-turn bounded loop).** The canonical workflow (`plugins/pr-babysit/workflows/babysit.md`) gains `### OpenCode start or resume` and `### OpenCode cancel` beside the Claude and Codex controllers:
   - `SLOT` as today; `STATE_FILE="$REPO_ROOT/.opencode/tmp/pr-babysit/$SLOT.json"`; `PLUGIN_ROOT="$TOOLU_PLUGIN_ROOT_PR_BABYSIT"`; every helper runs as `"$TOOLU_BUN" --no-env-file "$PLUGIN_ROOT/hooks/dist/<helper>.js"` so a project `.env` (for example a `GH_TOKEN`) never reaches `gh`.
   - Run one complete cycle (Steps 1–6). On `keep_going`, `sleep` for `backoff.waitSeconds` (never more than 60) in one bash call and run the next tick, in the same turn, until the Success or Escalation stop. Pending checks are never completion.
   - The turn may end early (user interrupt, session end). State is the slot file; invoking the command again with no arguments resumes. A running fixer subprocess outlives the turn and is picked up by the next `babysit-dispatch-fix.js wait`.
   - `stop`/`cancel`: `babysit-dispatch-fix.js cleanup` (ends a live fixer, removes the clean worktree), then `babysit-record.js status --status cancelled`; the state file is kept as the durable record.
   - Inline delegation on OpenCode uses `task` with `subagent_type` `toolu-quick-task` (mechanical), `toolu-implementer` (implementation) or `toolu-architect` (architecture), per `host-mapping.md`'s OpenCode column. Inline fixes happen in a detached native worktree at exactly `$REPO_ROOT/.opencode/tmp/pr-babysit/$SLOT.inline` (`git worktree add --detach "$WORKTREE" "$HEAD_SHA"`, `HEAD_SHA` = `pr.head`); it is inside the project, so neither the controller nor a subagent hits an `external_directory` prompt, and it never collides with the dispatcher's `<slot>.worktree`. Push with `git -C "$WORKTREE" push origin "HEAD:$BRANCH"`; on success or cancel remove it with `git worktree remove <exact path>` only when clean.
2. **Generated OpenCode text** (`tools/toolu-opencode/scripts/lib/opencode-port.ts`): exact-anchor edits for `plugins/pr-babysit/skills/babysit/SKILL.md`, its `references/helper.md` and `workflows/babysit.md`. The OpenCode copies name only the OpenCode controller, `--host opencode`, the `.opencode/tmp/pr-babysit/` state path and the `"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_PR_BABYSIT/hooks/dist/…"` command form; Claude cron, Codex goal, `EnterWorktree`/`ExitWorktree`, `spawn_agent` and `CODEX_HOME` text is removed. A missing or duplicated anchor fails generation (existing behavior).
3. **Routing** (`plugins/pr-babysit/hooks/src/babysit/fixer-route.ts`):
   - `Host` gains `opencode` (alias `opencode`); its CLI is `opencode`. Any controller may list it in `hosts`/`prefer`/`routing`.
   - `routeFix` accepts `--host opencode`. For that host the config files are `@toolu/core/config`'s `configFiles({ host: "opencode" })` (user `TOOLU_USER_CONFIG_DIR/toolu.config.json`, project `.opencode/toolu.config.json`). Claude Code and Codex keep today's resolution byte for byte.
   - `routing.opencode` rows hold `model` = `provider/model` and `effort` = an OpenCode `--variant`. The default row is four `{}` entries: no `--model`/`--variant`, so the fixer runs the user's configured default model.
   - Jev on OpenCode: `PB_JEV`, else `$TOOLU_CONFIG_DIR/jev/jev.sh`, run with `--no-env-file`. The Claude and Codex wrappers are never used on OpenCode.
4. **OpenCode fixer transport** (`fixer-dispatch.ts`): a group whose host is `opencode` runs as a detached subprocess, never in a herdr pane:
   - argv: `opencode run --format json --dir <worktree> --agent pr-babysit-fixer [--auto] [--model M] [--variant E] "You are a pr-babysit fixer. Read <brief> and follow it exactly."`; `--auto` when the plan is unattended.
   - env: the dispatcher's own env plus `PWD=<worktree>` and `OPENCODE_CONFIG_CONTENT` = the caller's value (if any, parsed as JSON) deep-merged with `{"agent":{"pr-babysit-fixer":{"mode":"primary","description":…,"permission":{"task":"deny","bash":{"gh":"deny","gh *":"deny","git push":"deny","git push *":"deny","git * push":"deny","git * push *":"deny"}}}}}`. Agent rules are layered over the user's own permission rules, so nothing is widened; `--auto` approves asks but keeps explicit denies (host contract).
   - stdin closed; stdout and stderr to `<state>.fixer-r<R>g<S>.log`; own process group; the dispatcher does not wait. The group records `pid`, `pidStart` (`ps -o lstart=`) and `log`.
   - `wait`: polls once a second until the process group is gone or the deadline passes. Settled outcome = `fixerOutcome(report, last 40 log lines)`: `done`, `reported_failed`, `host_limited` (provider limit text; the host cools 60 min) or `no_report` (error = last log lines). A spawn failure or missing `opencode` is `agent_start_failed`. A subprocess never reaches `blocked`.
   - `cleanup` and a relaunch end a live group: SIGTERM, up to 10 s, then SIGKILL. A recorded pid whose `ps -o lstart=` no longer matches is treated as exited (pid reuse), never signalled.
   - The plan contract is unchanged: `babysit-route-fix.js` still answers `dispatch: "herdr"` whenever a fixer host is eligible (the value means "fixer agents through `babysit-dispatch-fix.js`", so Claude Code and Codex workflow text stays valid), and `inline` otherwise. `start` probes herdr only when the plan needs a pane.
   - `wait` handles groups in plan order whatever their transport: an `opencode` group is polled as above, any other group through `herdr agent wait` as today; a `pending` group of either kind is launched first in the call (existing rule, including the 250 s launch budget).
   - The fixer reaches the provider through the user's own OpenCode config (global or the worktree's committed project config); `OPENCODE_CONFIG_CONTENT` adds only the agent.
5. **Worktree without herdr.** A plan whose groups are all `opencode` needs no pane: the dispatcher reuses the recorded worktree when its path exists (fetch + fast-forward, as today), else runs `git -C <repo> worktree add -b pr-babysit/<slot> <state>.worktree origin/<pr>` and records `{path, workspaceId: null, paneId: null, …}`. A plan with any Claude Code, Codex or Cursor group keeps today's herdr path and, for a native worktree, opens a herdr workspace on it. `cleanup` of a native worktree runs `git worktree remove --force` after the existing clean check. Untracked `.opencode/` session files are not fixer work (like `.claude/`, `.codex/`, `.cursor/` today).
6. **Packaging.** `tools/toolu-opencode/scripts/bundle-plugins.ts` stages `plugins/pr-babysit/skills/babysit/references/fixer-brief.md`; the `@toolu/opencode` pack inventory requires it.

Reused as is: the tick helper and its state lock, reply/resolve idempotency ledger, `fixerOutcome`, `renderBrief`, `fixerAgentName`/paths, `SlotLock`, `atomicWriteJson`, the live harness under `tooling/src/opencode-host/`, the scripted provider and `@toolu/conformance` sandboxes.

## Interfaces / Schema

- `fixer-route.ts`: `type Host = "claude" | "codex" | "cursor" | "opencode"`; `hostKind("opencode") === "opencode"`; `hostCli("opencode") === "opencode"`; `configPaths(host: "claude" | "codex" | "opencode")`; `loadFixerConfig(host)` same union; `routeFix({ host: "opencode", … })`.
- `fixer-dispatch.ts` (exported for tests):
  - `opencodeFixerArgs(opts: { model: string | null; effort: string | null; unattended: boolean; worktree: string; prompt: string }): string[]`
  - `fixerConfigContent(existing: string | undefined): string` — throws `config_invalid` when `existing` is not a JSON object.
  - `groupAlive(pid: number, start: string): boolean`, `stopGroup(pid: number, start: string): boolean`
- Plan group (`babysit-route-fix.js` output, unchanged shape): `host` may be `"opencode"`.
- State `fixer.groups[]` adds, for `opencode` groups: `pid: number`, `pidStart: string`, `log: string`. `herdrWorktree.workspaceId` and `.paneId` may be `null` (native worktree).
- `babysit-dispatch-fix.js wait` output (unchanged shape): `groups[].host` may be `"opencode"`; `error` carries the last log lines for `no_report` and the spawn error for `agent_start_failed`.
- Config (`toolu.config.json`): `prBabysit.hosts`/`prefer` accept `"opencode"`; `prBabysit.routing.opencode` is four `{model?: "provider/model", effort?: "<variant>"}` entries.
- OpenCode controller paths: state `<repo>/.opencode/tmp/pr-babysit/<slot>.json`; dispatcher worktree `<repo>/.opencode/tmp/pr-babysit/<slot>.worktree`; inline worktree `<repo>/.opencode/tmp/pr-babysit/<slot>.inline`; fixer brief/report/log `<state minus .json>.fixer-r<R>g<S>.{md,report.json,log}`.
- Live harness: `startScriptedProvider` (and `openSession`/`install`) accept a `"*"` script, used only when the first user message carries no `PROBE:` token (a nested fixer session started by the dispatcher). New `tooling/src/opencode-host/scenarios-babysit.ts` exports `BABYSIT_SCENARIOS` (`babysit.fixer`, `babysit.no-report`, `babysit.cancel`), registered in `tooling/src/opencode-entry-smoke.ts`.

## Failure modes and edge cases

- **`opencode` not on PATH** at route time: the host is dropped (`dropped (CLI not on PATH): opencode`), or the round goes inline when no host is left. At launch: `agent_start_failed` with the reason; no process, state records the failure.
- **Fixer exits without a report** (model gave up, provider error, crash): `failed`/`no_report`, `error` = last log lines. A later `wait` returns the same settled state; nothing is replied or resolved.
- **Fixer reports `failed`**: `reported_failed`; committed work stays in the worktree for the controller.
- **Provider usage limit** in the log: `host_limited`, `hostCooldowns.opencode` for 60 min; the next route skips OpenCode.
- **Fixer tries `git push`, `git -C x push`, `gh …` or `task`**: denied before execution by the agent rules; the tool error reaches the fixer; origin is unchanged.
- **User `OPENCODE_CONFIG_CONTENT` set**: merged, user keys kept; not a JSON object → `config_invalid`, no process started.
- **Controller turn interrupted while a fixer runs**: the subprocess continues; the next invocation's tick lists the items under `fixing[]` and `wait` settles it.
- **Cancel while a fixer runs**: the process group is ended, `fixer` is `null`, the clean worktree is removed, status `cancelled`. Uncommitted fixer edits → `worktree_dirty` with `changes[]`, nothing removed.
- **Stale pid**: a recorded pid that now belongs to another process (start time differs) is treated as exited and never signalled.
- **Two OpenCode sessions on one PR**: the slot lock serializes helper calls; replies and resolves stay single through the `actions` ledger (`duplicate_reply`, exit 4); a second `start` is refused with `fixer_running`.
- **Mixed plan without herdr**: `herdr_unavailable`, as today.
- **State file of another PR**: `slot_mismatch`, as today.
- **Long controller turn**: OpenCode compacts the session; the state file, not the transcript, is authoritative.

## Acceptance criteria

- **AC-1:** On pinned `opencode-ai@1.18.34` with the scripted provider, a project selecting `pr-babysit` runs, through its own bash, `babysit-route-fix.js --host opencode` (config `prBabysit.routing.opencode` from `.opencode/toolu.config.json`), `babysit-dispatch-fix.js start` and `wait`. In an isolated git repository whose PR branch has a failing `bun test`, the OpenCode fixer edits the file, runs the test, commits, has `git push` denied, writes the done report, and `wait` returns `status: done` with exactly one commit. `bun test` then passes in the worktree, only the seeded file changed, and `origin/<pr>` is unchanged.
- **AC-2:** A fixer that ends without a report settles `failed`/`no_report` with log text in `error`; a `cleanup` during a running fixer ends its whole process group, sets `fixer` to `null`, removes the worktree, and `record status --status cancelled` leaves `status: cancelled`; in both, the `actions` ledger is unchanged.
- **AC-3:** On the pinned host the fixer's `git push`, `git -C <dir> push` and `gh` bash calls and its `task` call end as permission errors before execution, and the `origin/<pr>` ref is unchanged. `fixerConfigContent` keeps every key of a caller's `OPENCODE_CONFIG_CONTENT` (including a `permission.bash` string or object), so user rules are never widened, and rejects a non-object value.
- **AC-4:** `babysit-route-fix.js --host opencode` reads `TOOLU_USER_CONFIG_DIR` and `.opencode/toolu.config.json`, accepts `routing.opencode`, defaults to four `{}` rows, drops a missing `opencode` CLI, and uses only `$TOOLU_CONFIG_DIR/jev/jev.sh`; Claude Code and Codex routing output is unchanged on the captured fixtures.
- **AC-5:** The generated `pr-babysit-babysit-73c340c6` skill, its references and `resources/pr-babysit/workflows/babysit.md` name the OpenCode controller, `--host opencode`, `.opencode/tmp/pr-babysit/` and `$TOOLU_PLUGIN_ROOT_PR_BABYSIT`, every relative link resolves, and none contains `get_goal`, `create_goal`, `update_goal`, `CronCreate`, `CronList`, `EnterWorktree`, `ExitWorktree`, `spawn_agent`, `CODEX_HOME`, `.codex/tmp`, `/tmp/pr-babysit-` or `--host claude|codex`.
- **AC-7:** In `babysit.fixer`, the controller's own bash runs `babysit-tick.js` on the captured `toolu-165` snapshot (`--snapshot-in`) with `--state-file "$REPO_ROOT/.opencode/tmp/pr-babysit/falconiere-toolu-165.json"`: the state is created there as version 2, and after the fixer settles `babysit-record.js round --had-rejection false --fix-pushed` clears the done fixer record and sets `fixAttempts` to 1. The generated skill keeps the shared "Authorization and execution handoff" section verbatim.
- **AC-6:** The packed `@toolu/opencode` contains `plugins/pr-babysit/skills/babysit/references/fixer-brief.md` and every pr-babysit bundle; on the pinned host the skill loads through the npm route and `$TOOLU_PLUGIN_ROOT_PR_BABYSIT/hooks/dist/babysit-tick.js` exists.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
| --- | --- | --- | --- | --- |
| AC-1 | `tooling/src/opencode-host/scenarios-babysit.ts` `babysit.fixer`: sandbox repo with `src/sum.ts` bug, failing `src/sum.test.ts`, bare `origin`, PR branch | controller bash ran route/start/wait; `done`, 1 commit; worktree `bun test` exit 0; diff = `src/sum.ts`; origin ref unchanged | push denied mid-run | `bun run smoke:opencode-entry babysit.fixer` |
| AC-2 | `babysit.no-report`, `babysit.cancel` scenarios; `fixer-native.test.ts` process-group tests with real `sh`/`sleep` processes | `no_report` + log text; group dead, fixer null, worktree gone, `cancelled`; ledger equal | child + grandchild, pid reuse | `bun run smoke:opencode-entry babysit.no-report babysit.cancel`; `bun test plugins/pr-babysit/hooks/src/__tests__/` |
| AC-3 | `babysit.fixer` push/gh/task attempts; `fixerConfigContent` over user configs (`bash: "ask"`, object, invalid) | tool errors, refs unchanged; merged JSON keeps user keys | invalid JSON → `config_invalid` | same scenario; `fixer-native.test.ts` |
| AC-4 | `review-items.json` + `fix-tiers.json` fixtures, sandbox configs | groups for `opencode`; defaults; missing CLI dropped; jev path | Claude/Codex outputs equal before/after | `bun test plugins/pr-babysit/hooks/src/__tests__/fixer-native.test.ts` |
| AC-5 | committed `generated/` tree | required strings present, banned absent, links resolve | anchor drift fails generation | `bun test tools/toolu-opencode/scripts/__tests__/`; `bun run check:opencode-surface` |
| AC-7 | captured `snapshots/toolu-165.json` through the installed bundles | state v2 at the `.opencode` path; `fixer: null`, `fixAttempts: 1` after `round` | — | `bun run smoke:opencode-entry babysit.fixer`; `generate-surface.test.ts` |
| AC-6 | `bun pm pack --dry-run`; npm-route session | file present; skill loads; bundle path exists | — | `bun run test:pack`; `babysit.fixer` observations |

The `smoke:opencode-entry` scenarios need the pinned binary and run locally (CI acceptance is OP-28, #362); their pass lines are recorded in the PR. Everything else runs in `bun run test`.

## Spec review (2026-10-03)

- Architecture: 🟡 should-fix: inline OpenCode worktree path was unstated. Fixed: `<slot>.inline` under `.opencode/tmp/pr-babysit/`.
- Architecture: 🟡 should-fix: plan `dispatch` value, herdr probing and mixed-transport `wait` were implicit. Fixed: explicit bullets under item 4.
- Acceptance criteria: 🟡 should-fix: tick/record on the OpenCode state path and the handoff section had no criterion. Fixed: AC-7.
- Acceptance evidence: 🟡 should-fix: where the live scenarios run was unstated. Fixed: paragraph above.
- Jev: coverage 1.62/2; weakest section "none" 0.49 (evidence 0.33); buildable Noul 0.37 (uncertain, no named gap). No blocker remains.

## Implementation notes

- A denied `task` is not offered to the fixer at all; a model's `task` call becomes OpenCode's `invalid` tool. The bash denials refuse with "a rule which prevents you from using this specific tool call".
- `stopGroup` snapshots the group and every ppid descendant before signalling, because OpenCode runs each bash tool call in its own process group.
- Deny patterns match command text, so wrapper forms (`env git push`, `bash -c 'gh …'`) slip past them. The fixer's environment therefore also drops `GH_TOKEN`/`GITHUB_TOKEN` (and the enterprise ones), points `GH_CONFIG_DIR` at a path never created (no `gh` login), and sets `GIT_TERMINAL_PROMPT=0` and `GIT_ALLOW_PROTOCOL=file`: git refuses https, ssh (scp-style aliases included) and git:// remotes, while local bare remotes, which this repository's own tests push to, keep working. A rejected first design rewrote push URLs with `pushInsteadOf`; it broke those local-remote tests and missed relative and alias forms. `OPENCODE_CONFIG_CONTENT` is validated before any side effect.
- `host_limited` is read from the host's own errors only (`error` events and non-INFO stderr), so tool output that mentions a rate limit is not a provider limit.

## Documentation impact

- `plugins/pr-babysit/workflows/babysit.md` (OpenCode controller, cancel, state path, inline column), `skills/babysit/references/helper.md` (OpenCode state path, `--host opencode`, subprocess transport, new fields), `plugins/pr-babysit/README.md` (OpenCode section).
- `docs/config.md` (`prBabysit` hosts/routing for `opencode`), `docs/opencode.md` (PR babysitting section), `docs/pr-babysit/README.md` if it repeats host lists.
- Capability matrix note for pr-babysit tick scheduling (resolution text) and the generated host-contract doc block.
- Regenerated `tools/toolu-opencode/generated/`.

## Open Questions

None blocking. If the pinned host does not honor `--agent` from `OPENCODE_CONFIG_CONTENT`, or a deny pattern misses `git -C x push`, the scenario fails and this spec is revised before merge (owner: this worker).
