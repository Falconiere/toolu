# OpenCode: review and core workflow skills on native tools — Design

**Date:** 2026-10-03   **Status:** Approved   **Author:** Cursor agent (epic worker, #358)   **Topic:** Make the toolu and toolu-review skills, agents and commands executable on OpenCode: native tool names, agent ids, model routing and helper paths, with review, commit and debug workflows proven on real isolated repositories (OP-24)

## Problem

The shared substrate (#337–#345) discovers toolu's 6 skills, 5 agents and 2 commands and toolu-review's skill on OpenCode, but their text still routes through Claude Code and Codex interfaces. A read of `tools/toolu-opencode/generated/` against the pinned host `opencode-ai@1.18.34` finds:

- **Host mapping has no OpenCode column.** `resources/toolu/workflows/host-mapping.md`, which commit, review-and-commit, orchestrator and deep-research defer to, lists only `Agent`/`Task`, `spawn_agent`, `AskUserQuestion`, `request_user_input`, `EnterWorktree` and `WebSearch`. On OpenCode the real interfaces are the `task` tool (`description`, `prompt`, `subagent_type`, optional `task_id`/`command`, no model argument, subagent depth 1 by default), the `skill` tool, the `question` tool (CLI and app clients, or `OPENCODE_ENABLE_QUESTION_TOOL`), and `webfetch`/`websearch`. Built-in subagents are `general` and `explore`; `plan` is a primary agent. Evidence: strings of the pinned binary and the SDK `PermissionConfig` keys (`read edit glob grep list bash task external_directory todowrite question webfetch websearch lsp skill`).
- **Unreachable debug helpers.** `toolu-debug` says `bun plugins/toolu/scripts/debug-testfail.ts`. That repository-relative path does not exist in a user project, and the npm package does not stage `plugins/toolu/scripts`.
- **Claude-only discovery in debug.** The Sentry adapter says to discover tools with `ToolSearch` and names `mcp__claude_ai_Sentry__authenticate`. OpenCode has no `ToolSearch`; MCP tools are `<server>_<tool>` for servers in `opencode.json`.
- **Wrong agent names and tiers.** The orchestrator lists `toolu:quick-task / Codex quick-task`, `Explore`, `Plan` and `general-purpose`; on OpenCode the agents are `toolu-quick-task` … `toolu-architect`, `explore` and `general`. Every generated agent says "This agent runs on **Sonnet**" (or Haiku/Opus), but the generator drops Claude model aliases, so on OpenCode each agent runs `agent.<id>.model` from the user's config or the session model.
- **The session protocol asks for a model argument the host lacks.** SessionStart's `model-routing.md` says "Pass `model:` on EVERY Agent call" with Claude aliases. OpenCode's `task` has no `model` parameter.
- **Review state path and host blocks.** `toolu-review-review` keeps a `# Codex` block beside the OpenCode one and says the state lands under `.claude/tmp/push-review/` or `.codex/tmp/push-review/`; on OpenCode it is `<repo>/.opencode/tmp/push-review/`.
- **Codex-only setup with no capability message.** `toolu-setup` installs Codex TOML profiles under `${CODEX_HOME:-$HOME/.codex}/agents`. Run from OpenCode it would silently write Codex files. On OpenCode the agents are already contributed by the config hook (#345).
- **Silent rewrite rot.** The existing per-skill replacements in `scripts/lib/rewrite.ts` are `String.replace` calls that become no-ops when the source changes.

## Non-Goals

1. Surfaces owned by other work packages: brainstorm and delivery-flow (OP-21), epic-orchestrator (OP-22), pr-babysit (OP-23), statusline (OP-25), Jev (done in #350).
2. Changing Claude Code or Codex surface text, except the shared host-mapping table, which gains a column while its Claude Code and Codex cells stay byte-identical.
3. Choosing models for OpenCode agents. Provider/model ids are user-specific; the documented override `agent.<id>.model` stays the only route.
4. New gates or gate-mode changes. Workflows are proven against the existing core gates.
5. The OpenCode CLI installer (OP-26) and mandatory CI acceptance (OP-28).

## Architecture

Decisions were checked with Jev: an exact-match replacement table over a marker scheme or overlay files (choice 0.89), and staging the debug scripts in the package over copying them into the skill (0.51 vs 0.42; decided for a single source of truth and an unchanged relative layout under the plugin root).

1. **Shared host mapping.** `plugins/toolu/workflows/host-mapping.md` gains an `OpenCode` column. It is copied verbatim into `generated/resources/toolu/workflows/`, so every workflow that defers to it routes correctly.
2. **OpenCode port table.** New `tools/toolu-opencode/scripts/lib/opencode-port.ts` exports `OPENCODE_PORTS: Record<sourcePath, Array<[from, to]>>`, keyed by repo-relative source path, for the toolu and toolu-review sources only. `applyPort(sourcePath, text)` replaces each `from` and throws `opencode port: <path>: expected exactly one match for <from…>` when it matches zero or several times. It is applied:
   - in `renderMarkdown` to skill, agent and command bodies (before `rewriteBody`), and
   - in `planSkillResources` to every copied Markdown resource, by its real source path: `orchestrator/references/model-routing.md`, and `workflows/semantic-judgments.md`, whose "installed `jev` skill" becomes `skill({ name: "jev-jev" })`.
   The two `toolu-review-review` replacements in `rewrite.ts` move into the table. Agents also get one generic rewrite: `This agent runs on **<Tier>**` becomes `On Claude Code this agent runs on **<Tier>**`, followed by an OpenCode sentence naming `agent.<id>.model`.
3. **Debug helpers ship.** `bundle-plugins.ts` stages `plugins/toolu/scripts/debug-*.ts` into the package's `plugins/toolu/scripts/`. The port table rewrites `plugins/toolu/scripts/debug-…` to `"$TOOLU_PLUGIN_ROOT_TOOLU/scripts/debug-…"`; `shell.env` already sets that variable for both the clone and the npm layout.
4. **Setup capability message.** `setup.ts` checks `TOOLU_HOST_OVERRIDE` (set by `shell.env` in every OpenCode bash) before parsing its verb. On `opencode` it writes nothing, prints the capability message to stderr and exits 2. The generated `toolu-setup` body says the same and how to override or disable an agent.
5. **Session routing on OpenCode.** `plugins/toolu/hooks/docs/model-routing-opencode.md` routes each class by `subagent_type`. `session-docs.ts` renders it instead of `model-routing.md` when `host === "opencode"`. It is budgeted in `context-budget.ts` beside `model-routing`.

Reused: `createTooluHooks`, `shell.env` (`TOOLU_PLUGIN_ROOT_TOOLU`, `TOOLU_CONFIG_DIR`), the published `toolu-review/write-state.sh`, core push-review and quality gates, `@toolu/conformance` sandboxes, the pinned-host harness (`openSession`, `runHost`, scripted provider).

## Interfaces / Schema

- `scripts/lib/opencode-port.ts`:
  - `export const OPENCODE_PORTS: Readonly<Record<string, readonly (readonly [string, string])[]>>`
  - `export function applyPort(sourcePath: string, text: string): string` (repo-relative POSIX path; unknown paths return `text`)
  - `export function portAgent(surfaceId: string, body: string): string`
- Host-mapping OpenCode cells:
  - Invoke: `skill({ name: "<plugin>-<name>" })`, or `/<command-id>`.
  - Delegate: `task` with `subagent_type` `toolu-quick-task`, `toolu-deep-explore`, `toolu-research-agent`, `toolu-implementer`, `toolu-architect`, `explore` or `general`. No model argument; the agent's `agent.<id>.model` applies.
  - Ask: the `question` tool when listed; otherwise one concise question.
  - Inspect/steer: the `task` result; pass its `task_id` to continue the same subagent.
  - Isolate: native `git worktree` commands with an exact, validated path.
  - External information: `webfetch`/`websearch` or the installed exa-search and context7 skills.
- Setup refusal (stderr, exit 2): `setup.ts: OpenCode registers toolu's agents itself (toolu-quick-task, toolu-deep-explore, toolu-research-agent, toolu-implementer, toolu-architect); there are no Codex profiles to install. Set agent.<id>.model in opencode.json to pin a model, or disable: true to drop one.`
- Package: `plugins/toolu/scripts/debug-io.ts`, `debug-log.ts`, `debug-stack.ts`, `debug-testfail.ts` in `@toolu/opencode`.
- Review state on OpenCode: `<repo>/.opencode/tmp/push-review/<branch-slug>.json`, schema `version: 2`.

## Failure modes and edge cases

- **Source drifts under a port entry.** Generation throws, naming the source path and the expected text. `check:opencode-surface` fails, and no stale rewrite ships silently.
- **Port text matches twice.** Same failure: ambiguous anchors are rejected.
- **`setup.ts` on OpenCode with any verb, including invalid ones.** Exit 2 and the capability message; nothing under `CODEX_HOME` or `HOME` is created. Without the override (Claude Code, Codex, a plain shell), behavior is unchanged.
- **Debug helper input unrecognized or empty.** The helpers keep their capped raw passthrough; the port changes only the path.
- **Review with open findings.** `--findings-count 1` writes state that keeps the push denied (`pushReview: block`).
- **Review on a detached checkout or another worktree.** The skill keeps `--branch` and `--repo`; unchanged.
- **Commit while the quality gate is failing.** `git commit` is denied before it runs, with or without `--no-verify`; no commit object is created. The default `balanced` preset blocks the quality gate on OpenCode.
- **`question` tool absent** (for example a non-CLI client without the flag). The mapping's fallback is one concise question.
- **Nested delegation.** The default `subagent_depth` of 1 makes the host refuse a task from a subagent; the orchestrator text says so.

## Acceptance criteria

- **AC-1:** The generated host mapping has an OpenCode column naming `skill`, `task` with `subagent_type`, `question`, `task_id`, native `git worktree` and `webfetch`/`websearch`. The Claude Code and Codex cells of every row are byte-identical to `origin/main`.
- **AC-2:** For every generated toolu and toolu-review skill, agent and command, plus every resource they link: no `ToolSearch`, `AskUserQuestion`, `request_user_input`, `spawn_agent`, `EnterWorktree`, `ExitWorktree`, `mcp__`, `CODEX_HOME`, `general-purpose`, `$toolu:`/`/toolu:`, `.claude/tmp`/`.codex/tmp`, `Pass \`model:\`` or repository-relative `plugins/toolu/scripts/` path appears outside the host-mapping file. Each `skill({ name })` names a generated skill. Each `toolu-<agent>` id named is a generated agent. Each `$TOOLU_PLUGIN_ROOT_TOOLU/…` path exists in the staged npm plugin tree, and each `${TOOLU_OPENCODE_ROOT}/generated/…` path exists in `generated/`. Every one of these 14 surfaces has canonical frontmatter that the host accepts: a skill's `name` equals its directory and passes the host name rule, with a description of at most 1024 characters; an agent is `mode: "subagent"`; a command has a description and loads an existing generated skill. With toolu and toolu-review selected, the plugin's `config` hook registers all 7 skill directories, 5 agents and 2 commands.
- **AC-3:** Generation fails with an error naming the source and the anchor when a port anchor is missing or matches more than once.
- **AC-4:** Each generated toolu agent is schema-valid canonical frontmatter (`mode: "subagent"`, unchanged permission rules) and says it runs `agent.<id>.model` on OpenCode, else the session model; no agent claims a fixed tier on OpenCode. The generated orchestrator skill and its `model-routing.md` route by `subagent_type` and state that `task` has no model argument and that subagents do not nest by default.
- **AC-5:** On OpenCode, toolu's SessionStart context routes by `subagent_type` and contains no `Pass \`model:\``; the Claude Code and Codex model-routing output is unchanged; the new doc is within its context budget.
- **AC-6:** In a real isolated git repository with a local bare remote and `pushReview: block`, through `createTooluHooks`: `git push` is denied before review and the remote is unchanged; the generated skill's OpenCode write-state command, run in bash with the `shell.env` environment, writes `.opencode/tmp/push-review/<branch>.json` v2 with `reviewed_files`; then the same push is allowed and the remote has the commit. With `--findings-count 1` the push stays denied.
- **AC-7:** In a real isolated repository with the default preset, after a bash `bun run test` that exits nonzero passes through `tool.execute.after`, `git commit -m "fix: x"` and `git commit --no-verify -m "fix: x"` are denied before they run and `HEAD` is unchanged; after a passing `bun run test`, the commit is allowed and created.
- **AC-8:** A real failing `bun test` transcript piped to `bun "$TOOLU_PLUGIN_ROOT_TOOLU/scripts/debug-testfail.ts"` in the `shell.env` environment prints the failing test name and its `file:line`, for the clone layout and for a package staged by `stagePlugins`. `test:pack` requires the four debug scripts in `@toolu/opencode`.
- **AC-9:** `setup.ts preview`, `install` and `remove --yes` with `TOOLU_HOST_OVERRIDE=opencode` exit 2 with the capability message and create nothing under the sandbox `HOME` or `CODEX_HOME`. The generated `toolu-setup` skill names the five `toolu-*` agents, `agent.<id>.model` and `disable: true`. The existing Codex setup tests pass unchanged.
- **AC-10:** With `TOOLU_LIVE_OPENCODE=1`, on the pinned host with a scripted provider, the model:
  - loads `toolu-review-review` through the native `skill` tool;
  - sees a `git push` denied;
  - runs the write-state command, after which the push completes;
  - loads `toolu-debug` and runs the helper on a failing test, whose output names the failing test;
  - completes a `task` with `subagent_type: "toolu-quick-task"`.
- **AC-11:** Claude Code and Codex behavior is intact. Source skills, agents and commands are unchanged apart from the host-mapping column, and the existing plugin, setup, session-start and generator suites pass.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Regenerated `generated/resources/toolu/workflows/host-mapping.md`; `git show origin/main:plugins/toolu/workflows/host-mapping.md` | OpenCode column present; Claude/Codex cells equal | Row count unchanged | `bun test tools/toolu-opencode/scripts/__tests__/core-surfaces.test.ts` |
| AC-2 | The committed generated tree; `stagePlugins` into a sandbox; `createTooluHooks` `config` hook in a sandbox project selecting toolu and toolu-review | No banned token; every reference resolves; frontmatter canonical and host-valid; 7 + 5 + 2 surfaces registered | Host-mapping file excluded from the token scan only | same file, plus `bun test tools/toolu-opencode/src/plugin/__tests__/core-workflows.test.ts` for the hook registration |
| AC-3 | A sandbox copy of `plugins/toolu` with the debug anchor edited out, and one with an anchor duplicated | `planSurface` throws naming path and anchor | Zero and two matches | same file |
| AC-4 | Generated `agents/*.md`, orchestrator skill and reference | Canonical frontmatter; OpenCode routing sentences | No `This agent runs on` | same file |
| AC-5 | toolu `session-start.js` with `TOOLU_HOST_OVERRIDE=opencode`, then `claude` and `codex` | OpenCode routing doc; other hosts byte-unchanged | Budget | `bun test plugins/toolu/hooks/src/__tests__/` (session-start suite) and `bun run test:context-budget` |
| AC-6 | Sandbox repo, bare remote, toolu + toolu-review selected | Deny, write state, allow, remote updated; findings keep deny | Findings > 0 | `bun test tools/toolu-opencode/src/plugin/__tests__/core-workflows.test.ts` |
| AC-7 | Sandbox repo with a failing then passing `test` script | Both commits denied, HEAD unchanged; then created | `--no-verify` | same file |
| AC-8 | A failing `bun test` file in the sandbox | Test name and `file:line` printed | npm layout via `stagePlugins` | same file, plus `bun run test:pack` |
| AC-9 | Sandbox HOME/CODEX_HOME | Exit 2, message, no files | Invalid verb | `bun test plugins/toolu/skills/__tests__/setup-agents.test.ts` |
| AC-10 | Pinned `opencode-ai@1.18.34`, scripted provider | Tool states and outputs as listed | Live only | `TOOLU_LIVE_OPENCODE=1 bun test tools/toolu-opencode/src/plugin/__tests__/core-workflows.live.test.ts` |
| AC-11 | Source tree vs `origin/main` | Only host-mapping and setup.ts changed among sources | — | `git diff --stat origin/main -- plugins/toolu/skills plugins/toolu/agents plugins/toolu/commands plugins/toolu-review/skills` and `bun run test` |

## Documentation impact

- `docs/opencode.md`: a "Core workflows" subsection covering agent ids and model override, `question`, the review state path, debug helpers and the setup capability message.
- `plugins/toolu/README.md` and `plugins/toolu-review/README.md`: OpenCode notes.
- `docs/toolu/README.md` and `docs/toolu-review/README.md` when they describe host routing (regenerated copies under `generated/resources/repo/docs/`).
- Generated surface notes are regenerated.

## Spec review

- Acceptance criteria: 🟡 should-fix (resolved): issue AC-1 coverage was partial (Jev 0.59). Schema validity applied to agents only, and installed registration was unproven. AC-2 now covers frontmatter for all 14 surfaces and config-hook registration (Jev 0.73 after the fix).
- Architecture: 🟡 should-fix (resolved): `semantic-judgments.md` named "the installed `jev` skill", which cannot be loaded by that name on OpenCode. The port now covers linked resources by source path.
- No blockers. Status: Approved.

## Open Questions

- None blocking. `models.opencode` config routing (mapping classes to provider models automatically) is a possible follow-up; it is out of scope by Non-Goal 3.
