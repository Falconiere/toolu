# OpenCode: context7 helper publication and documentation workflow — Design

**Date:** 2026-10-03   **Status:** Approved   **Author:** Claude Code (epic worker, #348)   **Topic:** Port context7's published search helper, its documentation-first startup instruction and its generated skill to OpenCode (OP-14)

## Problem

The shared substrate (#341, #342, #343, #345) already publishes `context7/search.sh` under the OpenCode data root and `shell.env` exports `TOOLU_CONFIG_DIR` and `TOOLU_BUN`. A real-hook probe (`createTooluHooks`, project selecting `toolu` and `context7`) shows four context7-specific defects:

- **The instruction belongs to the wrong plugin and is gated on the wrong host.** context7's SessionStart is silent. The only documentation-first instruction is the context7 line in toolu's `MANDATORY — proactive plugin use` block. That line is gated by `pluginActive("context7@toolu")`, which on every non-Codex host, OpenCode included, reads Claude Code's `installed_plugins.json`. A user who has Claude Code without context7 therefore gets no context7 instruction on OpenCode, even though OpenCode selected it. The capability matrix assigns this instruction to `context7.startup`.
- **`.env` reaches the helper.** The instruction and the skill run `search.sh` directly, through its `#!/usr/bin/env bun` shebang. Bun loads `.env` from the working directory unless it is given `--no-env-file`, and the agent's bash runs in the project directory. A `CONTEXT7_API_KEY=ctx7sk…` in a project `.env` is sent as a Bearer token. The skill and README say the key is read "from the environment only, never from a `.env` file".
- **A mislabelled generated skill.** `generated/skills/context7-context7/SKILL.md` shows `# Codex` with the Codex path and labels the OpenCode path `# Claude Code`, then tells the agent to "choose the line for the active host".
- **No skill name.** Nothing in the startup text names the native skill `context7-context7`.

Nothing proves that library resolution and documentation lookup run from the installed paths in the agent's bash.

## Non-Goals

1. Changing Claude Code or Codex output. context7's SessionStart stays silent there, and toolu's block keeps its context7 line there. Their `.env` autoload gap is the same follow-up Jev's PR recorded.
2. Changing the helper itself (`search.ts`, requests, exit statuses) or `@toolu/core/rest`.
3. Changing exa-search, ast-grep or other lines of toolu's mandate block, or the shared `pluginActive` logic. exa-search is OP-15.
4. Changing the shared startup order, ledger or `shell.env`.
5. A new runtime state gate.

## Architecture

Decisions were checked with Jev against the evidence above. Run the helper with `--no-env-file`: 0.83. Remove the Claude presence check on OpenCode: 0.86. Owner: context7's own hook 0.54 against toolu's block 0.46. The tie went to context7's hook for two reasons. The capability matrix assigns the instruction to `context7.startup`. And a context7-owned line removes the presence bug without touching the shared presence logic.

1. **context7's SessionStart speaks on OpenCode.** `plugins/context7/hooks/src/session-start.ts` gains a host check (`TOOLU_HOST_OVERRIDE=opencode`, which every adapter child has). On OpenCode:
   - It publishes with `publishWrapper` instead of `publishBunCli`. The instruction runs Bun by absolute path, so the "bun not found on PATH" advisory would be false there.
   - When the result is `published` or `kept-user-file` and stdin's `source` is not `compact`, it writes one SessionStart context: the instruction. `renderHookOutput(sessionContext(...))` from `@toolu/core/startup`, as Jev does.
   - On compaction it relinks and writes nothing. The system transform already carries the startup lines on every request.

   The new helpers live in `plugins/context7/hooks/src/context7/opencode.ts`. On Claude Code and Codex the bundle behaves exactly as it does today.
2. **The instruction.** One line, the same verbs as toolu's line:
   - It names the command `'<bun>' --no-env-file '<path>'`, or `'<path>'` alone when the user owns a regular file there. `<bun>` is the hook's `process.execPath`: the adapter resolved it, and it is the same Bun `shell.env` exports. Quoting is POSIX single quotes, so spaces and quotes in paths survive.
   - It names `skill({ name: "context7-context7" })`.
   - It says that `CONTEXT7_API_KEY` is optional, read from the environment only, and that a non-zero exit (22 HTTP error, including 429 rate limits) means context7 is unavailable for that call.
3. **toolu's block skips context7 on OpenCode.** In `plugins/toolu/hooks/src/lifecycle/tool-mandates.ts` the context7 line requires `host !== "opencode"`. Each request then carries exactly one context7 instruction, and the Claude presence check never decides it on OpenCode.
4. **Generated skill rewrite.** `tools/toolu-opencode/scripts/lib/rewrite.ts`, for skill `context7-context7`, replaces the Codex/Claude block and the "Choose the line" paragraph with one OpenCode command, `"$TOOLU_BUN" --no-env-file "${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/context7/search.sh"`, plus one sentence: `shell.env` sets both variables, and a user-owned file at that path runs directly. `generated/` is regenerated.

Reused as they are: `createTooluHooks`, the startup ledger and readiness checks (the helper record), `shellEnvFor`/`applyShellEnv`, `spawnEntry` (already `--no-env-file`), `@toolu/conformance/https-fixture`, and the live-host harness under `tooling/src/opencode-host/`.

## Interfaces / Schema

- `plugins/context7/hooks/src/context7/opencode.ts`:
  - `export const OPENCODE_SKILL = "context7-context7"`
  - `export function onOpencode(): boolean`
  - `export function command(path: string, symlink: boolean): string`, giving `'<execPath>' --no-env-file '<path>'` or `'<path>'`
  - `export function instruction(command: string): string`
  - `export async function compacting(): Promise<boolean>`, which reads SessionStart stdin; invalid or empty stdin is not compact.
- Instruction text (OpenCode):

  `context7 (library docs) — for ANY third-party library/framework question (API usage, current docs, code examples, version behavior) you MUST run <command> FIRST (search <library> to resolve the ID, then docs <id> <query>) BEFORE answering from memory or searching the web. Web search is a FALLBACK ONLY when context7 lacks coverage or the command exits non-zero (22 is an HTTP error such as a 429 rate limit). Syntax: skill({ name: "context7-context7" }). CONTEXT7_API_KEY is optional and read from the environment only, never from .env.`
- Generated skill command block:

  ```bash
  # OpenCode
  "$TOOLU_BUN" --no-env-file "${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/context7/search.sh" <command> [options]
  ```

## Failure modes and edge cases

- **No key in the environment.** The helper sends no `Authorization` header and the request goes out; the service is rate-limited. The instruction is unchanged; a key is optional, so there is no notice.
- **Key only in a project `.env`.** The OpenCode command ignores it: no `Authorization` header is sent. The value appears in no context line or log.
- **Key in the bash environment (`ctx7sk…`).** It is sent as `Bearer <key>`.
- **Rate limit or other HTTP error.** The command exits 22 and writes `context7: HTTP 429 from <url>` to stderr. The instruction makes it a fallback trigger.
- **Bun absent from PATH.** The instruction names Bun by absolute path, so it runs with `PATH=/usr/bin:/bin`. The generated skill uses `$TOOLU_BUN`, which `shell.env` always exports. No PATH advisory is printed on OpenCode.
- **Paths with spaces or quotes.** POSIX single quoting in the instruction; double quotes around `$TOOLU_BUN` and the path in the skill.
- **User-owned regular file at `search.sh`.** It is kept (`kept-user-file`) and named alone, without Bun or `--no-env-file`, because it brings its own interpreter.
- **Helper missing or unwritable** (`source-missing`, `unwritable`, `link-failed`). No instruction. Readiness already reports the helper record (#342).
- **Compaction.** The relink is idempotent; no compaction context comes from context7.
- **context7 deselected.** Its bundle does not run, the ledger removes the helper, and toolu's block has no context7 line on OpenCode, so no instruction appears.
- **Concurrent sessions.** Startup lines are captured once per directory, so every session's system transform carries the same single instruction.

## Acceptance criteria

- **AC-1:** Given an OpenCode project selecting `toolu` and `context7`:
  - Every `experimental.chat.system.transform` call, across two sessions, carries exactly one context7 instruction. It names `'<bun>' --no-env-file '<dataRoot>/context7/search.sh'` and `skill({ name: "context7-context7" })`.
  - `experimental.session.compacting` adds no context7 instruction.
  - The `config` hook's `skills.paths` include `generated/skills/context7-context7`.
- **AC-2:** The instruction's command, run in `/bin/sh` from the project directory with the env that `shell.env` builds, does three things against the loopback HTTPS fixture:
  - It resolves a library: a GET to `/api/v2/libs/search?libraryName=…`, and stdout equals the planned JSON.
  - It looks up documentation: a GET to `/api/v2/context?libraryId=…&query=…&type=json`.
  - On a planned 429 it exits 22 with the `context7: HTTP 429 from …` stderr line.

  The generated skill's command, run the same way, makes the same `search` request.
- **AC-3:** Deterministic boundaries. This holds when the project directory contains a space, and when PATH is `/usr/bin:/bin` (no Bun):
  - With a project `.env` holding `CONTEXT7_API_KEY=ctx7sk_…` and no key in the bash env, the request carries no `Authorization` header, and the secret is in no system line or log.
  - With a key in the bash env, it carries `Bearer <key>`.
  - With a user-owned executable regular file at `search.sh`, the instruction names `'<path>'` alone, and running that command executes the user's file.
- **AC-7:** Given a project selecting only `toolu`, no system line and no compaction line names context7, although toolu's `skills.context7` defaults to enabled.
- **AC-4:** The generated `context7-context7` skill has an active `# OpenCode` command with `"$TOOLU_BUN" --no-env-file` and the OpenCode data-root path. It has no `CODEX_HOME` line, no `# Claude Code` label, and no "Choose the line" paragraph. `bun run check:opencode-surface` passes.
- **AC-5:** Claude Code and Codex are unchanged:
  - context7's SessionStart prints nothing and keeps its PATH advisory.
  - toolu's lifecycle goldens, including the context7 line, pass unmodified.
  - The existing context7 tests pass unmodified.
- **AC-6:** Live evidence, gated and reported separately:
  - `TOOLU_LIVE_OPENCODE=1`: on the pinned OpenCode host, the model's system request carries the instruction and its tool list carries `context7-context7`. A scripted bash call runs the instruction's command against the fixture and completes. The `.env` secret is never sent.
  - `TOOLU_LIVE_CONTEXT7=1`: the same published command makes one real `context7.com` `search` call and returns JSON results.

## Acceptance evidence

| AC | Real input / fixture | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Temp OpenCode project; real `createTooluHooks`; real context7 and toolu bundles | 1 instruction per system call in 2 sessions; exact command and skill; 0 in compaction; skill path in config | Two sessions | `bun test tools/toolu-opencode/src/plugin/__tests__/context7-delivery.test.ts` |
| AC-2 | HTTPS fixture for `context7.com`; `shell.env` output | Exact request paths and stdout; 429 → exit 22 | Generated skill command too | same file |
| AC-3 | Project dir with a space; `.env` with `ctx7sk_` secret; PATH `/usr/bin:/bin`; a user `search.sh` script | No auth header from `.env`; Bearer from env; secret absent from lines/logs; user file named alone and run | No Bun on PATH; spaces | same file |
| AC-7 | Same project, selection `["toolu"]` | 0 context7 lines | Claude `installed_plugins.json` listing context7 in `HOME` | same file |
| AC-4 | Generated catalog | Exact OpenCode block; no Codex/Claude lines | — | `bun test tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts` + `bun run check:opencode-surface` |
| AC-5 | Existing Claude/Codex tests; new Claude-host session-start case | Silent output; advisory kept; goldens unchanged | Claude source `compact` | `bun test plugins/context7/hooks/src/__tests__/ plugins/toolu/hooks/src/__tests__/` |
| AC-6 | Pinned `opencode-ai@1.18.34`, scripted provider, fixture; real context7.com | Instruction + skill in requests; completed bash; real JSON | `.env` secret present | `TOOLU_LIVE_OPENCODE=1 bun test …/context7-delivery.live.test.ts`; `TOOLU_LIVE_CONTEXT7=1 …` |

## Documentation impact

- `plugins/context7/README.md`: a new OpenCode section covering:
  - the published path under the data root;
  - the `context7-context7` skill;
  - the startup instruction and why it uses `--no-env-file`;
  - compaction;
  - the selection rule.
- `docs/context7/README.md`: the same OpenCode install note, in the shape `docs/jev/README.md` uses.
- `docs/opencode.md`: one line: context7's instruction comes from its own SessionStart on OpenCode.
- The generated skill (from `rewrite.ts`) and `generated/resources` copies.

## Spec review

Jev checked requirement/evidence alignment: issue coverage 0.83, scope 0.81, and failure-case evidence 0.55 before the fix.

- Acceptance criteria: 🟡 should-fix: the user-owned `search.sh` case and the deselected case had no criterion that would fail on a regression. Fixed: AC-3 adds the user-file case. AC-7 adds the deselected case, run with a Claude `installed_plugins.json` that lists context7, so the case also proves that the Claude record does not decide on OpenCode.
- Failure modes: 🔵 consider: quotes in paths are covered by the quoting function and are not tested separately; spaces are tested. Kept as is.

## Open Questions

None blocking. The Claude Code and Codex `.env` autoload gap for `search.sh` is the cross-host follow-up the Jev PR already recorded (owner: epic orchestrator).
