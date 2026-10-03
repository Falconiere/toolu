# OpenCode: Jira startup and issue workflow — Design

**Date:** 2026-10-03   **Status:** Approved   **Author:** Claude Code (epic worker, #351)   **Topic:** Port jira's published REST helper, its issue-workflow startup instruction and its generated skill to OpenCode (OP-17)

## Problem

The shared substrate (#341, #342, #343, #345) already publishes `jira/jira.sh` under the OpenCode data root, and `shell.env` exports `TOOLU_CONFIG_DIR`, `TOOLU_BUN`, `TOOLU_HOST_OVERRIDE=opencode` and `TOOLU_PROJECT_CONFIG_DIRNAME=.opencode`. The capability matrix assigns `jira.startup` ("SessionStart publishes the jira helper and issue-workflow instructions") and `jira.tools` to OP-17. Reading the code and the generated catalog shows these jira-specific defects:

- **No startup instruction.** jira's SessionStart only publishes the helper and is silent on every host. On OpenCode nothing tells the model which command runs the wrapper or which skill to load.
- **A wrong skill reference.** toolu's per-prompt hint says "use the `jira` skill". On OpenCode the skill is `jira-jira`, and the hint fires whether or not jira is selected.
- **A mislabelled generated skill.** `generated/skills/jira-jira/SKILL.md` shows the Codex command and labels the OpenCode path `# Claude Code` with `TOOLU_HOST_OVERRIDE=claude`, and tells the agent to choose one. Its plan section names `.claude/tmp/...` or `.codex/tmp/...`, while on OpenCode the helper writes `.opencode/tmp/...`.
- **`.env` reaches the helper.** The helper is a `#!/usr/bin/env bun` symlink; Bun loads `.env` from the working directory unless given `--no-env-file`. The skill promises credentials "never [from] a `.env` file". A project `.env` that sets `JIRA_BASE_URL` overrides the server in the user's jira-cli config while the keyring or config token still fills `JIRA_API_TOKEN`, so the user's token goes to the `.env` host. Plan checks call `"$JIRA"` (the same symlink) in a child shell, so even a top-level run with `--no-env-file` reloads `.env` one level down; there a `.env` `JIRA_PAT` replaces Basic auth with its own Bearer token.
- **The write boundary is implicit.** The skill says mutating verbs "have no undo prompt (the verb is the confirmation)". Nothing says that loading the skill does not authorize a write.

## Non-Goals

1. Changing Claude Code or Codex output or behavior. jira's SessionStart stays silent there with its PATH advisory; toolu's prompt hint keeps its jira line there; plan checks there keep today's environment. The cross-host `.env` autoload gap there is the follow-up #378 and #381 already recorded.
2. Changing the REST client (`@toolu/core/rest`), Jira request shapes, exit statuses or credential discovery (`creds.ts`).
3. Adding a Jira-specific permission rule. A mutation is a bash call and already passes OpenCode's bash permission and toolu's pre-tool gates.
4. Changing the shared startup order, ledger, `shell.env` or the presence logic.
5. Real Jira tenant traffic. No credentials are available to this worker, and a live write to someone's tenant is never acceptable as test evidence.
6. A new runtime state gate.

## Architecture

Decisions were checked with Jev against the evidence above. Owner of the instruction: jira's own SessionStart, with toolu's prompt hint omitting its jira line on OpenCode — 0.98 (against keeping toolu's hint, 0.00, or both, 0.02). Append `--no-env-file` to `BUN_OPTIONS` for plan children on OpenCode — 0.76.

1. **jira's SessionStart speaks on OpenCode** (`plugins/jira/hooks/src/session-start.ts`, new `plugins/jira/hooks/src/jira/opencode.ts`), the same shape as context7 (#348):
   - `TOOLU_HOST_OVERRIDE=opencode` (every adapter child has it) selects the OpenCode path. It publishes with `publishWrapper` instead of `publishBunCli`, so the "bun not found on PATH" advisory, false when Bun is named by path, is not printed.
   - When the result is `published` or `kept-user-file` and stdin's `source` is not `compact`, it writes one SessionStart context with the instruction, through `renderHookOutput(sessionContext(...))`.
   - On compaction it relinks and writes nothing; the system transform already carries the startup lines on every request.
   - Elsewhere the bundle behaves exactly as it does today.
2. **The instruction** names the command `'<bun>' --no-env-file '<helper>'` (`<bun>` is the hook's `process.execPath`, the Bun the adapter resolved), or `'<helper>'` alone for a user-owned file, in POSIX single quotes. It names `skill({ name: "jira-jira" })`. It says read-only calls may run directly and that no ticket change is made unless the user asked for that change. It says credentials come from `JIRA_*` in the environment or the jira CLI login, never `.env`.
3. **toolu's prompt hint skips Jira on OpenCode.** `promptHints` gets a `jira` option; `user-prompt-submit.ts` passes `host !== "opencode"`. On OpenCode the jira instruction (present only while jira is selected) replaces it.
4. **Plan children ignore `.env` on OpenCode.** In `plugins/jira/hooks/src/jira/plan-run.ts`, when `TOOLU_HOST_OVERRIDE` is `opencode`, the probe's and every check's environment gets `--no-env-file` appended to `BUN_OPTIONS` (an existing value is kept, space-separated). Bun 1.4.2 applies `BUN_OPTIONS` to `#!/usr/bin/env bun` scripts (verified). Only those children are affected; the agent's own bash is not.
5. **Generated skill rewrite** (`tools/toolu-opencode/scripts/lib/rewrite.ts`, skill `jira-jira`): the Codex/Claude block becomes one `# OpenCode` command, `"$TOOLU_BUN" --no-env-file "${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/jira/jira.sh" [--api-version N] [--lean] <family> <action> [options]`; the "Choose the complete command" paragraph becomes an OpenCode paragraph; the two plan-path sentences name `.opencode/tmp/...`. `generated/` is regenerated.
6. **Write boundary in the source skill.** `plugins/jira/skills/jira/SKILL.md`'s "Mutating operations" section gains one sentence: run one only when the user asked for that change; loading this skill or reading an issue never authorizes a write. This is text on every host and changes no behavior.

Reused as they are: `createTooluHooks`, the startup ledger, `shellEnvFor`/`applyShellEnv`, `publishWrapper`, `@toolu/conformance/https-fixture`, the jira test harness's recorded bodies, `jev-fixtures.ts` helpers and the live-host harness under `tooling/src/opencode-host/`.

## Interfaces / Schema

- `plugins/jira/hooks/src/jira/opencode.ts`:
  - `export const OPENCODE_SKILL = "jira-jira"`
  - `export function onOpencode(): boolean`
  - `export function command(path: string, symlink: boolean): string`
  - `export function instruction(command: string): string`
  - `export async function compacting(): Promise<boolean>`
- Instruction text (OpenCode):

  `jira (issue tracker) — when the user mentions Jira, a JQL query, an issue key like ABC-123 or an atlassian.net/browse link, run <command> <family> <action> [options] (syntax: skill({ name: "jira-jira" })) instead of the Atlassian MCP. Read-only calls (search, issue get, board/sprint/project/user lookups) may run directly. Never create, update, comment on, transition, assign or delete an issue, or change a sprint, worklog or attachment, unless the user asked for that change; plan it first as the skill describes. Credentials come from JIRA_* in the environment or the jira CLI login, never from .env; without them the command prints setup help and exits 1.`
- `plan-run.ts`: `export function childEnv(env: Env): Env` — `env` unchanged off OpenCode; on OpenCode `{ ...env, BUN_OPTIONS: "<existing> --no-env-file" }`.
- `prompt-hints.ts`: `HintOptions` gains `jira: boolean`.
- Generated skill block:

  ```bash
  # OpenCode
  "$TOOLU_BUN" --no-env-file "${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/jira/jira.sh" [--api-version N] [--lean] <family> <action> [options]
  ```

## Failure modes and edge cases

- **No credentials.** The command exits 1 and prints the setup help; no request is made. The instruction says so.
- **HTTP error (401, 404, 429).** Exit 22, stderr `jira: HTTP <status> from <url>`; the URL carries no credential. The token appears in no stdout, stderr, system line or log.
- **`.env` with `JIRA_BASE_URL` while the jira-cli config supplies server and token.** The instruction and skill commands ignore `.env`: the request goes to the configured server. (Through the shebang it would go to the `.env` host with the token.)
- **`.env` with `JIRA_PAT` while the environment uses email + API token.** Top-level and nested plan-check calls send Basic auth; the `.env` Bearer never leaves.
- **Existing `BUN_OPTIONS`.** Kept; `--no-env-file` is appended.
- **Bun absent from PATH.** The instruction names Bun by path; the skill uses `$TOOLU_BUN`. A plan check's `"$JIRA"` resolves through `env bun`: `shell.env` appends Bun's directory to PATH when PATH has none, so the check still runs.
- **Paths with spaces or quotes.** POSIX single quoting in the instruction; double quotes in the skill.
- **User-owned regular file at `jira.sh`.** Kept and named alone.
- **Helper missing or unwritable.** No instruction; readiness reports the helper record (#342).
- **Compaction.** Relink only; no jira context.
- **jira deselected.** Its bundle does not run, and toolu's prompt hint has no jira line on OpenCode, so no Jira guidance appears.
- **Loading the skill or starting a session.** No request reaches Jira: startup, system transform and skill discovery make zero requests.

## Acceptance criteria

- **AC-1:** Given an OpenCode project selecting `toolu` and `jira`, every `experimental.chat.system.transform` call across two sessions carries exactly one jira instruction naming `'<bun>' --no-env-file '<dataRoot>/jira/jira.sh'` and `skill({ name: "jira-jira" })`; compaction adds none; the `config` hook's `skills.paths` include `generated/skills/jira-jira`.
- **AC-2:** The instruction's command, run in `/bin/sh` from a project directory containing a space with `PATH=/usr/bin:/bin` and the `shell.env` environment, against the loopback HTTPS fixture for `acme.atlassian.net`:
  - `issue get ABC-1` makes one `GET /rest/api/3/issue/ABC-1` with the environment's credentials, and stdout is the recorded body;
  - `issue comment ABC-1 <text>` makes one `POST /rest/api/3/issue/ABC-1/comment` with an ADF body, and a planned 400 on that mutation exits 22 with `jira: HTTP 400 from …/comment`;
  - a planned 401 exits 22 with `jira: HTTP 401 from https://acme.atlassian.net/rest/api/3/issue/ABC-1`;
  - with no credentials it exits 1 with the setup help and makes no request;
  - the generated skill's command makes the same `GET`.
  No output, system line or log contains the token.
- **AC-3:** Given a jira-cli config naming `https://acme.atlassian.net` with a token and a project `.env` setting `JIRA_BASE_URL=https://media.example.net`, the instruction's command sends its request to `acme.atlassian.net`; the same helper run through its shebang sends the token to `media.example.net` (the hazard).
- **AC-4:** Given `TOOLU_HOST_OVERRIDE=opencode`, environment credentials `JIRA_EMAIL` + `JIRA_API_TOKEN`, and a project `.env` with `JIRA_PAT=<secret>`, `plan run` with a check `"$JIRA" user whoami` makes its probe and check requests with Basic auth, never `Bearer <secret>`, and writes the ledger under `.opencode/tmp/plan-ledger/jira-<KEY>.json`. Without the override the existing behavior is unchanged.
- **AC-5:** Loading never writes: starting the session, the system transform and the `config` hook make zero fixture requests; the instruction says mutations happen only when the user asked for that change; and the source and generated skills state that loading the skill never authorizes a write.
- **AC-6:** Given a project selecting only `toolu`, no system line, compaction line or prompt reminder for a Jira prompt names Jira; given `toolu` and `jira`, the prompt reminder has no `jira` skill hint either. On Claude Code the prompt hint is unchanged.
- **AC-7:** The generated `jira-jira` skill has one `# OpenCode` command with `"$TOOLU_BUN" --no-env-file` and the OpenCode data-root path, no `CODEX_HOME`, no `TOOLU_HOST_OVERRIDE=claude`, no `# Claude Code`, no "Choose the complete command", and names `.opencode/tmp/jira/plans/<KEY>.md`; `bun run check:opencode-surface` passes.
- **AC-8:** Claude Code and Codex are unchanged: jira's SessionStart prints nothing and keeps its PATH advisory; toolu's lifecycle goldens pass unmodified; the existing jira tests pass unmodified.
- **AC-9:** Live, gated: with `TOOLU_LIVE_OPENCODE=1` on the pinned OpenCode host, the model's system request carries the instruction and lists `jira-jira` as a skill; a scripted `skill` call loads `jira-jira` and then a scripted bash call runs `"$TOOLU_BUN" --no-env-file "$TOOLU_CONFIG_DIR/jira/jira.sh" issue get ABC-1` against the fixture. Both complete, and the fixture sees exactly one `GET` and no write method, with no `.env` secret sent.

## Acceptance evidence

| AC | Real input / fixture | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Temp project; real `createTooluHooks`; real jira and toolu bundles | 1 instruction per call in 2 sessions; exact command and skill; 0 in compaction; skill path | Two sessions | `bun test tools/toolu-opencode/src/plugin/__tests__/jira-delivery.test.ts` |
| AC-2 | HTTPS fixture; recorded `issue.json`; `shell.env` output | Exact methods, paths, auth, stdout, exit codes | Space in dir; no Bun on PATH; 401 read; 400 mutation; no creds | same file |
| AC-3 | jira-cli config file; `.env` host override | Instruction → acme; shebang → media host | — | same file |
| AC-4 | Plan doc with `"$JIRA" user whoami`; `.env` `JIRA_PAT` | Basic auth on probe and check; `.opencode` ledger | Without override unchanged | `bun test plugins/jira/hooks/src/__tests__/opencode.test.ts` |
| AC-5 | Same project with fixture proxy active | 0 requests after startup + transform + config; instruction wording | — | `jira-delivery.test.ts` + `opencode.test.ts` |
| AC-6 | Selection `["toolu"]` and `["toolu","jira"]`; `chat.message` with a Jira prompt | No Jira hint on OpenCode | Claude golden unchanged | `jira-delivery.test.ts`, `bun test plugins/toolu/hooks/src/__tests__/` |
| AC-7 | Generated catalog | Exact OpenCode block and paths | — | `bun test tools/toolu-opencode/scripts/__tests__/generate-surface.test.ts`, `bun run check:opencode-surface` |
| AC-8 | Existing Claude/Codex suites | Unchanged | Claude `compact` source | `bun test plugins/jira/ plugins/toolu/hooks/src/__tests__/` |
| AC-9 | Pinned `opencode-ai`, scripted provider, fixture | Instruction + skill in request; one GET, no write | `.env` secret present | `TOOLU_LIVE_OPENCODE=1 bun test …/jira-delivery.live.test.ts` |

## Documentation impact

- `plugins/jira/README.md` and `docs/jira/README.md`: an OpenCode section (helper path, `jira-jira`, the instruction and why it uses `--no-env-file`, plan paths under `.opencode/tmp`, nested checks ignoring `.env`, compaction, selection).
- `plugins/jira/skills/jira/SKILL.md`: the write-boundary sentence.
- `docs/opencode.md`: the context line names jira's instruction and toolu's omitted prompt hint.
- Generated skill and `generated/resources` copies.

## Spec review

Jev checked requirement/evidence alignment: issue coverage 1.84/2 (0.86 full), scope 0.79.

- Acceptance criteria: 🟡 should-fix: AC-2 tested mutation transport only on success; the issue asks for error handling on mutations too. Fixed: a planned 400 on `issue comment` exits 22.
- Acceptance criteria: 🟡 should-fix: AC-5's "no imperative to mutate" was not checkable. Fixed: it names the exact boundary phrase and the skill sentence.
- Acceptance criteria: 🔵 consider: AC-9 did not load the skill on the real host. Fixed: a scripted `skill` call precedes the read, proving the load alone makes no request.

## Open Questions

None blocking. Real Jira tenant evidence is out of scope (no credentials; writes to a real tenant are not acceptable test evidence); the HTTPS fixture replays recorded Jira bodies instead. The Claude Code and Codex `.env` autoload gap is the cross-host follow-up recorded in #378 and #381 (owner: epic orchestrator).
