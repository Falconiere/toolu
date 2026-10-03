# OpenCode: Jev startup, prompt reminders and typed judgments — Design

**Date:** 2026-10-03   **Status:** Approved   **Author:** Cursor agent (epic worker, #350)   **Topic:** Port Jev's executable, skill, startup mandate and per-prompt reminder to OpenCode's native hooks, with typed judgments through the published wrapper and presence-only credential checks (OP-16)

## Problem

The shared OpenCode substrate (#341, #342, #343, #345) already runs Jev's SessionStart and UserPromptSubmit bundles, but a real-hook probe in a temp project with `toolu` and `jev` selected shows four Jev-specific defects:

- **Duplicate startup context on compaction.** `experimental.chat.system.transform` appends the Jev mandate to every model request. Jev's `hooks.json` SessionStart matcher includes `compact`, so `experimental.session.compacting` re-runs the bundle and appends the same mandate to the compaction context. The capability matrix records Jev's compaction use as `none`.
- **The wrong skill.** The mandate and reminder say `Syntax and linked examples: <plugin>/skills/jev/SKILL.md`. That file only shows the Codex and Claude Code wrapper paths. On OpenCode the skill is discovered natively as `jev-jev`.
- **A mislabelled generated skill.** The generated `generated/skills/jev-jev/SKILL.md` keeps `# Codex` with the Codex assignment active and labels the OpenCode assignment `# Claude Code: use this assignment instead`. `references/problem-solving.md` has the same pair.
- **`.env` leaks into credential checks.** Bun loads `.env` from the working directory unless it is given `--no-env-file`. This was verified: a key present only in a project `.env` is visible to `bun p.js` and absent with `bun --no-env-file p.js`. The OpenCode spawner runs every hook bundle as `[bun, bundle]` with the project root as the working directory. As a result, Jev's presence check reports a key that exists only in the project `.env`. The mandate's `'<bun>' '<wrapper>'` command, run from the project, also sends that key. Jev's README says the key "is never read from a `.env` file". Every other plugin README says the same about its own key.

Nothing proves that a typed judgment runs through the OpenCode-published wrapper with the `shell.env` environment.

## Non-Goals

1. Changing Claude Code or Codex output. Their hooks.json launchers also let Bun load `.env`; the PR reports that gap and leaves it for a cross-host follow-up.
2. Changing the shared compaction substrate or other plugins' compaction contributions.
3. Rewriting other plugins' generated skills (context7, exa-search, jira and agent-browser belong to OP-14, OP-15, OP-17 and OP-12).
4. Controlling OpenCode's own process environment. A key that the host process already carries is "present" by definition.
5. A new runtime state gate.
6. The one shared change is `--no-env-file` in the OpenCode hook spawner, which every plugin's OpenCode hooks pass through. It is in scope because a credential presence check cannot be honest while the spawner loads `.env`, and every plugin README already promises never to read one. The existing `context-delivery`, `startup-*` and `tool-*` suites guard the other plugins' behavior.

## Architecture

Decisions were checked with Jev against the evidence above: native skill reference (confidence 1.0), one OpenCode wrapper block (0.89), Jev-local compaction skip (0.76 over a shared dedupe), and an OpenCode-only `.env` scope (0.67).

1. **Jev hooks know the host.** `plugins/jev/hooks/src/jev/availability.ts` gains `onOpencode(env)`, which calls core `detectHost` (every toolu child on OpenCode has `TOOLU_HOST_OVERRIDE=opencode`). On OpenCode:
   - `invocation(wrapper)` for the published symlink becomes `'<bun>' --no-env-file '<wrapper>'`. A user override still runs alone with its own interpreter.
   - `skillReference(pluginDir)` returns `skill({ name: "jev-jev" })` instead of `<plugin>/skills/jev/SKILL.md`. Both bundles use it.
2. **No compaction mandate on OpenCode.** `session-start.ts` reads the SessionStart stdin `source`. On OpenCode with `source: "compact"` it still publishes (an idempotent relink) but writes no context: the system transform already carries the mandate on every request. Claude Code and Codex keep re-injecting on compact.
3. **Hook children ignore `.env`.** `spawnEntry` (`tools/toolu-opencode/src/bootstrap/spawn.ts`) spawns `[bun, "--no-env-file", bundle]`. Startup and context delivery both use it. Every hook already receives its whole environment explicitly from the host.
4. **Generated skill rewrite.** `scripts/lib/rewrite.ts`, for skill `jev-jev` (its `SKILL.md` and its references), does two things:
   - It replaces the Codex/Claude assignment pair with `# OpenCode\nJEV="${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/jev/jev.sh"`.
   - It rewrites `"$JEV_BUN" "$JEV"` to `"$JEV_BUN" --no-env-file "$JEV"`.

   `generated/` is regenerated.

Reused as they are: `createTooluHooks`, `createContextHooks` (one reminder per prompt, claimed per message ID), `shellEnvFor`/`applyShellEnv` (`TOOLU_CONFIG_DIR` and `TOOLU_BUN` in bash), core `publishWrapper`, and `@toolu/conformance/https-fixture`.

## Interfaces / Schema

- `availability.ts`:
  - `export function onOpencode(): boolean`
  - `export function invocation(wrapper: string): string` adds `--no-env-file` on OpenCode, for symlinks only.
  - `export function skillReference(plugin: string): string`
  - `export const OPENCODE_SKILL = "jev-jev"`
- SessionStart stdin: `{ "source"?: "startup" | "resume" | "clear" | "compact", ... }`. Invalid or empty stdin counts as not-compact.
- Command on OpenCode (symlink): `'<TOOLU_BUN>' --no-env-file '<dataRoot>/jev/jev.sh'`.
- Generated OpenCode skill wrapper block: `# OpenCode\nJEV="${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/jev/jev.sh"`.

## Failure modes and edge cases

- **Hook key empty or absent.** Startup and prompt text both carry the credential notice. The value is never printed.
- **Key only in the project `.env`.**
  - Hooks: they do not see it, so the notice appears.
  - The published command: it exits 1 with `jev: TYPESAFE_API_KEY unset` and makes no request.
  - Recovery: the agent falls back on the evidence.
- **Key in the host env.** No notice. The agent's bash inherits the key, and the command authenticates. The key appears in no context line or log.
- **Service failure.** For example, a fixture 401 or 500 makes the command exit non-zero (22 on HTTP errors) and print no answer map. The mandate requires an explicit evidence fallback.
- **Trivial prompt (`ok`).** No reminder. A replay of the same message ID gets no second reminder.
- **Unexecutable wrapper.** This keeps the existing "Jev unavailable" text. The reminder stays silent.
- **Compact stdin missing or not JSON.** The bundle behaves as on startup. On OpenCode the compact payload is always well-formed JSON from `compactStdin`.
- **User override at `jev.sh`.** It is invoked directly. No `--no-env-file` is added, because it has its own interpreter.

## Acceptance criteria

- **AC-1:** Given an OpenCode project with `toolu` and `jev` selected:
  - Every `experimental.chat.system.transform` call carries exactly one Jev mandate.
  - Each substantive `chat.message` gets exactly one Jev reminder part.
  - A trivial `ok`, or a replayed message ID, gets none.
  - `experimental.session.compacting` adds no Jev mandate.
- **AC-2:** On OpenCode, both the mandate and the reminder name `skill({ name: "jev-jev" })` and the command `'<bun>' --no-env-file '<dataRoot>/jev/jev.sh'`. The generated skill `jev-jev` (SKILL.md and problem-solving reference) has an active `# OpenCode` assignment, no `CODEX_HOME` line, and `"$JEV_BUN" --no-env-file "$JEV"`.
- **AC-3:** Run the command from the OpenCode startup line in `/bin/sh` from the project directory, with the env that `shell.env` builds, Bun off `PATH`, the HTTPS fixture and the key in the command env. It returns the typed answer map (`{"q":{"type":"noul","noul":0.92}}`). The fixture sees `Bearer <key>`. A fixture 401 makes the same command exit 22 and print no answer map.
- **AC-4:** With the hook key empty, a project `.env` containing a secret key, and no key in the command env:
  - Startup and prompt text both carry the credential notice.
  - No context line or host log contains the secret.
  - The published command exits 1 with `jev: TYPESAFE_API_KEY unset` and makes no fixture request.
- **AC-5:** Claude Code and Codex Jev output is unchanged. Their existing session-start and user-prompt-submit tests pass unmodified. On Claude, compact still re-injects the mandate.
- **AC-6:** Live evidence, gated and reported separately:
  - `TOOLU_LIVE_OPENCODE=1`: the pinned OpenCode host shows the mandate in the system request and the reminder in the user request. A scripted bash call runs the published wrapper against the HTTPS fixture and completes with the typed answer.
  - `TOOLU_LIVE_JEV=1` with a real `TYPESAFE_API_KEY`: the same published path makes one real TypeSafe call and returns a typed noul.

## Acceptance evidence

| AC | Real input / fixture | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Temp OpenCode project; real `createTooluHooks`; real Jev bundles | Mandate count 1 per system call; one reminder per prompt; 0 for `ok` and replay; 0 Jev lines in compaction | Two sessions; replay | `bun test tools/toolu-opencode/src/plugin/__tests__/jev-delivery.test.ts` |
| AC-2 | Same project; generated catalog | Exact command and skill reference; generated text | User override keeps bare path | same file + `bun test plugins/jev/hooks/src/__tests__/` + `bun test tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts` + `bun run check:opencode-surface` |
| AC-3 | HTTPS fixture for `api.typesafe.ai`; `shell.env` output; empty PATH | Typed answer; Bearer header; 401 → exit 22 | Bun off PATH | `jev-delivery.test.ts` |
| AC-4 | Project `.env` with `TYPESAFE_API_KEY=dotenv-secret`. (a) The real Jev bundles go through `spawnEntry` with an explicit env lacking the key; this case is red before `--no-env-file`, because an explicitly empty variable already blocks the `.env` value. (b) Full `createTooluHooks` with binding key `""`. (c) The startup command runs in `/bin/sh` from the project without the key | Notice in both; secret absent from system lines, prompt parts and logs; exit 1, 0 requests | `.env` in cwd | `jev-delivery.test.ts` |
| AC-5 | Existing Claude/Codex tests; new Claude compact case | Unchanged output | `source: compact` on Claude | `bun test plugins/jev/hooks/src/__tests__/` |
| AC-6 | Pinned `opencode-ai@1.18.34`, scripted provider, fixture; real key | Mandate, reminder, completed bash with typed answer; real noul | — | `TOOLU_LIVE_OPENCODE=1 bun test …/jev-delivery.live.test.ts`; `TOOLU_LIVE_JEV=1 …` |

## Documentation impact

- `plugins/jev/README.md` and `docs/jev/README.md`: an OpenCode section covering the published path under the data root, the `jev-jev` skill, compaction behavior, and `--no-env-file`.
- `docs/opencode.md`: hook children run with `--no-env-file`.
- The generated skill (from `rewrite.ts`).
- `tools/toolu-opencode/contract/capability-matrix.json` stays as it is (compaction `none` now holds).

## Open Questions

None blocking. The Claude Code and Codex `.env` autoload gap is recorded as a follow-up in the PR body (owner: epic orchestrator).

## Spec review

Jev checked requirement/evidence alignment: AC-6 0.94 and AC-4 0.77 once the spawner-level red case was stated; scope 0.64 before the shared spawner change was named as a non-goal boundary.

- Acceptance evidence: 🟡 should-fix: the AC-4 full-path case cannot fail before the fix, because an explicitly empty variable blocks `.env`. Fixed: added the spawner-level case with the key absent.
- Non-Goals: 🟡 should-fix: the shared spawner change was justified only in Architecture. Fixed: Non-Goal 6 states the boundary and the suites guarding other plugins.

**Status:** Approved
