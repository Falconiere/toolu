# OpenCode install and discovery of generated surfaces — Design

**Date:** 2026-10-02   **Status:** Approved   **Author:** Claude (epic worker, #345)   **Topic:** The OpenCode plugin contributes the enabled plugins' generated skills, agents and commands through the host's native discovery. Selection can be project or global, precedence is deterministic, and nothing is duplicated (OP-11)

## Problem

`tools/toolu-opencode/generated/` holds a valid OpenCode catalog for all 16 plugins (#344), but it is package content, not a discovery location. A clean install therefore exposes nothing:

- `docs/opencode.md` step 5 tells users to "wire OpenCode to those paths" by hand, and the plugin contributes nothing. No toolu skill reaches the native `skill` tool, and no toolu agent or command is discovered.
- Manual wiring is unsafe. Copied files lose their links to shared procedures (`../../resources/…`, for example the commit workflow). They ignore the enabled-plugin selection, go stale after an update or removal, and can duplicate skills.
- Duplicate skills are nondeterministic on the pinned host. Every `SKILL.md` is parsed concurrently, and the last parse to finish wins, with only a `duplicate skill name` warning.
- Selection is project-only and fails open. `.opencode/toolu/plugins.json` is the only explicit selection, and an invalid file is silently ignored, which enables every installed plugin.

Host facts this design relies on come from the pinned source at tag `v1.18.34` (`config/config.ts`, `plugin/index.ts`, `skill/index.ts`, `agent/agent.ts`, `command/index.ts`, `permission/index.ts`, `tool/external-directory.ts`), plus the live probes `surface.config-hook` and `surface.files`. See also the brainstorm `docs/toolu/brainstorms/2026-10-02-opencode-install-discover-surfaces.md`.

1. **Config, then plugins.** The host merges `opencode.json` and every config directory's `{command,commands}/**/*.md` and `{agent,agents}/**/*.md` before any plugin loads. It then calls each plugin's `config(cfg)`, in load order, with that same merged object. A plugin can change `cfg.command`, `cfg.agent`, `cfg.skills.paths` and `cfg.permission`, and the change takes effect in the same start. A command or agent file a plugin writes during init is only seen on the next start.
2. **Lazy skill scan.** Skills are scanned after plugin init. Every scan follows symlinks. The roots are:
   - `$HOME/.claude/skills/**/SKILL.md` and `$HOME/.agents/skills/**/SKILL.md`, including dot files;
   - every `.claude` and `.agents` directory from the instance directory up to the worktree, inclusive (to `/` when the worktree is `/`), with the same pattern. `OPENCODE_DISABLE_EXTERNAL_SKILLS` drops all four of these roots. `OPENCODE_DISABLE_CLAUDE_CODE` and `OPENCODE_DISABLE_CLAUDE_CODE_SKILLS` drop the two `.claude` roots. These three are Effect booleans: `true`, `yes`, `on`, `1` and `y` mean set, and anything else means unset;
   - each config directory's `{skill,skills}/**/SKILL.md`, without dot files. The config directories are the XDG `opencode` config directory, the `.opencode` directories from the instance directory up to the worktree (unless `OPENCODE_DISABLE_PROJECT_CONFIG` is `true` or `1`), `$HOME/.opencode`, and `OPENCODE_CONFIG_DIR`;
   - each `cfg.skills.paths` entry (`**/SKILL.md`), with `~/` expanded and relative paths resolved against the instance directory;
   - remote `cfg.skills.urls`.
3. **Skill identity.** The frontmatter `name` keys a skill, which is parsed with `gray-matter`, with a retry after sanitizing unquoted colons. The host tells the model that relative paths in a skill resolve against the skill's own directory.
4. **Permissions.** Reads and edits outside the project ask `external_directory` with the pattern `<dirname>/*`. The host's default rules allow `<skillDir>/*` for every discovered skill directory and ask for everything else. Rules are evaluated last match wins (`findLast`). A config-defined agent (every toolu agent) gets the host defaults, then the user's `cfg.permission` rules in key order, then its own rules. The native agents (build, plan, general, explore) merge the user's rules last. Probe `permission.order` shows `opencode run` rejects an ask unless `--auto` is passed; the TUI prompts.
5. **Native layering.** Config sources deep-merge, and later sources win per key. Agent entries override defaults field by field and merge permissions. `disable: true` removes an agent. A command entry replaces a built-in by name. `GET /command` also lists every skill as a `source: "skill"` command.

## Non-Goals

1. CLI management (`toolu install --host opencode` editing `opencode.json`): OP-26 (#360).
2. Context and instruction delivery (`instructions`, system, prompt and compaction hooks): OP-07 (#341).
3. What each leaf skill does once loaded, and leaf helper behavior: OP-12 to OP-25.
4. The published file list and independent loadability: OP-27 (#361). Mandatory real-host CI: OP-28 (#362). The final install and migration guide: OP-29 (#363). This change still updates today's docs to match its behavior.
5. Duplicate detection against skills the host only learns about after toolu's hook: remote `cfg.skills.urls`, and `skills.paths` added by another plugin's `config` hook that runs after toolu's.
6. Changing the generated catalog's layout or the generator's link rewriting (OP-10, merged). The generator changes only in wording.
7. Claude Code and Codex behavior. Only `tools/toolu-opencode`, its docs and the OpenCode live harness change.

## Architecture

**Decision: configure, do not materialize** (Jev `ask`: config hook 0.58 over a hybrid 0.40, materializing 0.01). On the ready path, the plugin's `config` hook adds the selected plugins' contributions to the merged config. No file is written into any OpenCode discovery directory or the user's repository.

- It is the only route that reaches agents and commands on the first start (fact 1).
- It keeps every link inside the package's own `generated/` tree.
- It leaves nothing to orphan when the package is removed.
- **Mapping the issue's file-tracking wording.** The issue's "track toolu-owned installed files" and "atomic updates/removal" are moot by design, because no file is installed. The owned set is exactly the IDs this start contributes. It is recomputed from the current package and selection at every start, logged by ID, and applied to an in-memory object that no other process shares.

**Package root.** `generatedDir = realpath(<@toolu/opencode package root>/generated)`, the same root that `TOOLU_OPENCODE_ROOT` already names. It holds `opencode.toolu.json`. The catalog's repository-relative `surfaceRoot` is ignored, and every catalog path resolves against `generatedDir`. A `TOOLU_REPO_ROOT` override changes only where plugin manifests are read.

**Selection** (`src/inventory/selection.ts`):

1. `<project>/.opencode/toolu/plugins.json`, when it exists.
2. Otherwise `<config root>/toolu/plugins.json`, when it exists. This file is new. The config root is `opencodeConfigRoot`: `TOOLU_CONFIG_DIR`, then `TOOLU_OPENCODE_HOME`, then `$XDG_CONFIG_HOME/opencode`, then `~/.config/opencode`.
3. Otherwise every installed plugin, as today.

In every case, `skills.<name>: false` in the project `toolu.config.json` still removes a plugin, and dependencies still close.

- An explicit file that is unreadable, not JSON, or fails `{version: 1, enabled: string[]}` (strict) makes toolu **not ready** (Jev 1.0).
- An enabled name that is not installed becomes a note (Jev: project replaces global, 0.96).

**Surface plan** (`src/surfaces/`, built in `prepareEnforcement`):

- **`readSurfaceCatalog(generatedDir)`.** Validates the catalog with Zod:
  - IDs match `^[a-z0-9]+(-[a-z0-9]+)*$` with at most 64 characters, and are unique per kind across the catalog;
  - every path resolves inside `generatedDir` to an existing regular file;
  - a skill path is exactly `skills/<id>/SKILL.md`, and that file's frontmatter `name` equals the ID, so the skip-if-defined check below cannot be bypassed by drift.
- **`planSurfaces`.** Takes the selected plugins in catalog order, and their surfaces in catalog order.
  - Agent and command files are parsed in the generator's canonical frontmatter format: one `key: <JSON>` line per key, keys sorted. The body is what follows.
  - Each is validated against the fields the generator emits. Agents allow `description`, `mode`, `permission`, `model`, `temperature`, `steps`, `hidden` and `color`. Commands allow `description`, `agent`, `model` and `subtask`.
  - The config entry is the frontmatter plus `prompt` (agent) or `template` (command), each set to `body.trim()`. These are the fields the host's own Markdown loaders produce, minus `name`, which the entry's key supplies.
  - A selected plugin missing from the catalog becomes a note.
- Any catalog or file failure makes toolu not ready: `surfaces: <reason>`.

**Applying** (`src/surfaces/apply.ts`, from the `config` hook):

- **Skills.** `existingSkillNames(scope)` mirrors fact 2: the roots, flags, dot rules, symlink following and up-walk. It reads each `SKILL.md`'s frontmatter `name` the way `gray-matter` does: an optional BOM, a `---` fence with LF or CRLF line ends, `Bun.YAML.parse` with the host's colon-sanitizer retry, and duplicate top-level keys counted as a parse failure. It maps each string name to its first location. A toolu skill whose ID is already defined is not added: the user's copy stays in force, and the note names it. Otherwise the skill's directory is appended to `cfg.skills.paths`, keeping the existing paths and every other `cfg.skills` key.
- **Agents and commands** (Jev: user-wins merge 0.93). Toolu's entry is the lowest layer: `cfg.agent[id] = deepMerge(toolu, user)` when an entry with that ID exists, else toolu's entry is added.
  - Plain objects merge recursively. Arrays and scalars come from the user.
  - **Trade-off:** this matches how OpenCode layers config and overrides built-in agents. A user's partial override, such as `agent.toolu-quick-task.model`, keeps toolu's prompt.
  - **Consequence:** a user's complete Markdown agent with the same ID inherits any key it omits, such as `mode: subagent` or toolu's permission rules. Users opt out with `disable: true`, or by setting those keys.
  - Entries toolu does not own are never read or written.
- **Shared procedures** (Jev 0.79 over copying resources into every skill, which would duplicate about 384 KB and still leave cross-skill links outside). Generated skills link shared files under `generatedDir/resources/`, which fact 4 would ask about.
  - When the user's `cfg.permission` has no key that could match `external_directory` (no `external_directory` key, and no key containing `*` or `?`), the hook adds `external_directory: { "<generatedDir>/resources/*": "allow" }`.
  - Otherwise it adds nothing and notes that the user's own rules decide.
  - This mirrors the host's own skill-directory allowance, extended only to the shared resources tree. Because it is added only when no user rule could match, every explicit user rule still decides (Jev 0.83 that no user rule is bypassed). A toolu agent's own rules, such as the generated `*: deny`, come later and win.
  - **Accepted trade-off:** `external_directory` is not specific to one tool. Like the host's skill-directory allowance, this one also lets edit and write tools touch `generatedDir/resources/` without the directory ask; the `edit` permission still applies. toolu never writes there, and an update replaces the package.
  - A link from a selected skill into an unselected plugin's skill directory (pr-babysit to `jev-jev`) still asks, by design.
- **Never throws.** Each kind (skills, agents, commands, permission) is computed first and assigned once, so each is applied whole or not at all. Any exception is caught and logged as `toolu: surfaces not applied: <reason>`; enforcement is unaffected.

**Reporting.** The host log gets, on every ready start:

- `toolu: surfaces (<source>, <ms> ms): skills <ids…>; agents <ids…>; commands <ids…>`, where source is `project selection`, `global selection` or `all installed plugins`;
- when there are any, one bounded `toolu: surface notes: …` line, such as `skill toolu-debug kept from <path>`, `agent toolu-quick-task merged under your config`, `external_directory left to your permission rules`, or `enabled plugin "x" in <path> is not installed`.

**Wiring:**

- `HostBinding` gains `worktree`, the host's raw value.
- The ready `Enforcement` gains `surfaces: SurfacePlan` and `selectionSource`.
- `createTooluHooks` returns `config` beside `tool.execute.before` and `shell.env` only when ready. A not-ready instance and the duplicate instance (`claimInstance`) add nothing, so loading both routes yields one set.

## Interfaces / Schema

```ts
// src/inventory/selection.ts
export type SelectionSource = "project" | "global" | "default";
export function resolveEnabledPluginNames(pluginsRoot: string, projectRoot: string, globalConfigRoot?: string):
  | { ok: true; enabled: Set<string>; source: SelectionSource; path?: string; unknown: string[] }
  | { ok: false; reason: string };            // invalid explicit file → reason names the path

// src/select/resolve.ts (success arm gains selection facts; third parameter optional, omitted = no global file)
export type SelectResult =
  | { ok: true; plugins: PluginManifest[]; source: SelectionSource; notes: string[] }
  | { ok: false; reason: string; missingDependency?: string };
export function selectPluginsWithDependencies(pluginsRoot: string, projectRoot: string, globalConfigRoot?: string): SelectResult;
// selectPluginsByEnabledNames keeps its signature; its success arm reports source "default", notes [].

// src/host/roots.ts
export function opencodeGlobalPluginSelectionPath(configRoot: string): string; // <configRoot>/toolu/plugins.json

// src/surfaces/catalog.ts
export type CatalogEntry = { id: string; file: string };                 // absolute
export type CatalogPlugin = { name: string; skills: CatalogEntry[]; agents: CatalogEntry[]; commands: CatalogEntry[] };
export function readSurfaceCatalog(generatedDir: string): { ok: true; plugins: CatalogPlugin[] } | { ok: false; reason: string };

// src/surfaces/plan.ts
export type SurfacePlan = {
  generatedDir: string;
  plugins: string[];
  skills: Array<{ id: string; plugin: string; dir: string }>;
  agents: Array<{ id: string; plugin: string; entry: Record<string, unknown> }>;
  commands: Array<{ id: string; plugin: string; entry: Record<string, unknown> }>;
  notes: string[];
};
export function planSurfaces(generatedDir: string, selected: readonly string[]): { ok: true; plan: SurfacePlan } | { ok: false; reason: string };

// src/surfaces/skill-names.ts
export type SkillScanScope = { directory: string; worktree: string; env: Record<string, string>; configuredPaths: readonly string[] };
export function existingSkillNames(scope: SkillScanScope): Map<string, string>;   // name → first SKILL.md path
export function frontmatterName(text: string): string | undefined;

// src/surfaces/apply.ts
export type SurfaceReport = {
  skills: string[]; agents: string[]; commands: string[];
  kept: Array<{ id: string; location: string }>;
  merged: Array<{ kind: "agent" | "command"; id: string }>;
  resourcesAllowed: boolean;
  notes: string[];
};
export function applySurfaces(config: object, plan: SurfacePlan, scope: Omit<SkillScanScope, "configuredPaths">): SurfaceReport;
```

The global selection file is `<config root>/toolu/plugins.json` with `{ "version": 1, "enabled": ["toolu", …] }`, the same strict schema as the project file. There is no new package export: callers import inside the package, and OP-26 can add one when it needs one.

## Failure modes and edge cases

| Input | Behavior |
|---|---|
| Project selection file invalid (bad JSON, wrong version, extra key, non-string name, unreadable) | Not ready: `plugin selection: invalid <path>: <detail>`. Every tool call is refused, and no surfaces are added |
| Project file absent, global file invalid | Not ready, naming the global path |
| Project file present (even `enabled: []`) | The project file decides, and the global file is not read |
| Enabled name not installed | Dropped, with the note `enabled plugin "<name>" in <path> is not installed` |
| Project `toolu.config.json` invalid | Unchanged from today: its `skills` disables are ignored, as `readSkillsDisabled` does now |
| Catalog missing, not JSON or schema-invalid; duplicate ID; path outside `generatedDir`; missing file; skill `name` ≠ ID; agent or command frontmatter not canonical or invalid | Not ready: `surfaces: <reason>` |
| Selected plugin absent from the catalog (a `TOOLU_REPO_ROOT` newer than the package) | Note `no generated surface for plugin "<name>"`, other plugins still apply |
| Same-name skill in any root of fact 2 | No path is added for it. The note names the winning location, and the host sees one copy |
| User `SKILL.md` with BOM or CRLF | Parsed like `gray-matter` |
| Scan duration | Not bounded by toolu. The scan covers only the host's own roots, which the host scans again right after, so toolu at most doubles that cost. The duration is logged (`<ms> ms`) and observed in the live runs |
| User `SKILL.md` with unparsable frontmatter, duplicate keys, or a non-string `name` | Not counted; the host also skips it |
| Unreadable or missing scan root, or a symlink loop | Skipped. Loops are cut by realpath |
| User `agent.<id>` or `command.<id>`, from JSON or Markdown | Deep-merged over toolu's entry with the user's keys winning, plus a note. `disable: true` removes the agent natively |
| User `cfg.permission` with an `external_directory` key or a wildcard key | No toolu permission is added, plus a note. The user's rules decide reads of shared procedures |
| `cfg.skills` not `{paths?: string[]}` | No skills are added, with the note `skills config unreadable`. Agents and commands still apply |
| Unexpected exception in the hook | Caught, with the error log `toolu: surfaces not applied: <reason>`; enforcement is unaffected |
| npm route plus local shim | Only the admitted instance has a `config` hook, so one set |
| Not ready for any reason | No `config` hook, so no toolu surfaces or permission |
| Two OpenCode processes on one project | Each changes only its own in-memory config; nothing is written |
| Plugin disabled, package updated or package removed | The next start recomputes from the current selection and package; nothing persists |
| Paths with spaces | Absolute paths, unchanged |

## Acceptance criteria

- **AC-1:** Setup: a clean isolated profile with the packed tarball through the npm route, the project selection `["pr-babysit"]`, and default permissions. Expected:
  - Of all catalog skill IDs, `debug skill` lists exactly the 7 of the closure {pr-babysit, toolu}, each under the package's `generated/skills/<id>/`.
  - `GET /agent` lists the 5 toolu agents, and `GET /command` lists the 3 `source: "command"` toolu commands of the closure. Nothing from an unselected plugin appears (for example `jev-jev`).
  - In a scripted `opencode run`, the native `skill` tool loads `toolu-commit-e11d9d00` and its output contains the skill body. The recorded model request's `skill` tool description names it. A `read` of the skill's linked `resources/toolu/workflows/commit.md` completes without a permission ask.
  - The project's `.opencode/` gains no `skills`, `agents` or `commands` directory.
- **AC-2:** The AC-1 project also holds the unrelated user skill `.opencode/skills/my-skill/SKILL.md`. Expected:
  - Re-selected to `["jev"]`: the host lists `jev-jev` and no surface from AC-1, and `my-skill` survives unchanged.
  - Switched to a second tarball whose `toolu-debug` description differs: `debug skill` shows the new description.
  - With toolu removed from `opencode.json`: no toolu skill, agent or command is discovered, and the project has no toolu-written surface file.
- **AC-3:** User definitions win:
  - `.opencode/skills/toolu-debug/SKILL.md` (different description): `debug skill` lists `toolu-debug` once, at the user's path.
  - `opencode.json` `agent.toolu-quick-task.description`: the resolved agent has the user's description with toolu's prompt.
  - `.opencode/agents/toolu-implementer.md`, a complete user agent with no `mode` or `permission`: its prompt wins, and it resolves with toolu's `mode: subagent` and toolu's deny-all permission rule. That is the documented consequence of layering.
  - `opencode.json` `command.toolu-commit-1e9b92d5.template`: the user's template wins.
  - The user's files are byte-identical afterwards, and the log notes the kept skill and the merges.
  - With `permission.external_directory: "ask"`, the shared-procedure read is rejected in `opencode run`. The user's rule decides.
- **AC-4:** Each host skill root of fact 2 holds a `toolu-debug` in turn: `$HOME/.claude/skills`, `$HOME/.agents/skills`, a project `.claude/skills` reached by the up-walk from a subdirectory instance, `OPENCODE_CONFIG_DIR/skills`, `$HOME/.opencode/skills`, and a user `skills.paths` directory. Each time, `debug skill` lists exactly one `toolu-debug`, at that root. Three parity cases follow:
  - With `OPENCODE_DISABLE_EXTERNAL_SKILLS=1`, a `$HOME/.claude/skills` copy is ignored by both, so the one `toolu-debug` is toolu's.
  - A copy under a dot directory `.opencode/skills/.hidden/toolu-debug` is ignored by both.
  - A BOM-prefixed CRLF user copy is honored by both.
- **AC-5:** With the npm route and a local `.opencode/plugins` shim loaded together, each toolu skill and agent appears exactly once. The log has one `toolu: surfaces` line and one `toolu: duplicate load skipped` line.
- **AC-6:** Selection sources:
  - A global `<XDG config>/opencode/toolu/plugins.json` `["jev"]` alone: `jev-jev` is discovered, and the log says `global selection`.
  - Adding a project file `["toolu-review"]`: `toolu-review-review` is discovered and `jev-jev` is not.
  - An invalid project file: a bash call is refused with `toolu: not ready: plugin selection: invalid …/plugins.json`, and no toolu skill is listed.
- **AC-7:** Against the committed catalog and real temporary trees:
  - every catalog failure row above yields a not-ready reason naming it;
  - plans follow catalog order;
  - `existingSkillNames` honors every root, flag value, dot rule, BOM or CRLF file and duplicate-key file in the table;
  - `applySurfaces` preserves unrelated entries, existing `skills.paths` and user keys;
  - the `config` hook returns normally on malformed config.
- **AC-8:** The repository gate (`bun run test`) is green, which includes surface, bundle and pack drift and `check:opencode-host`, and the Claude Code and Codex suites pass unchanged.

## Acceptance evidence

AC-1 to AC-6 are live scenarios in the new `tooling/src/opencode-host/scenarios-install.ts`, registered in `tooling/src/opencode-entry-smoke.ts` (`ALL_SCENARIOS`). They need the network on first run and the pinned host, so they are manual (`bun run smoke:opencode-entry <ids>`) until OP-28 makes live acceptance mandatory in CI. Their pass lines are recorded in the PR body. AC-7 and AC-8 run in `bun run test`.

| AC | Real input | Observable result | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Packed tarball, isolated pinned host, `["pr-babysit"]`, default permissions | `debug skill` names and locations; `/agent`; `/command` (`source: "command"`); skill-tool output and recorded tool description; completed `read` of `commit.md`; `.opencode` listing | An unselected plugin's surfaces are absent | `bun run smoke:opencode-entry surfaces.npm-clean` |
| AC-2 | Same project, reselected; a second tarball with a changed description; then the plugin removed | New set only; new description; user skill intact; nothing toolu-written | Update and removal | `bun run smoke:opencode-entry surfaces.lifecycle` |
| AC-3 | User skill, user Markdown agent, `opencode.json` agent, command and permission overrides | One `toolu-debug` at the user's path; merged agent; user prompt and template; identical bytes; notes; read rejected under the user's ask | Partial and complete overrides; a user permission rule | `bun run smoke:opencode-entry surfaces.precedence` |
| AC-4 | `toolu-debug` placed in each host root in turn | Exactly one `toolu-debug`, at that root | Up-walk from a subdirectory; env-selected roots | `bun run smoke:opencode-entry surfaces.skill-roots` |
| AC-5 | npm spec plus local shim | Each surface once; one surfaces line; one duplicate line | Two module copies | `bun run smoke:opencode-entry surfaces.both-routes` |
| AC-6 | Global file, then a project file, then an invalid project file | Global, then project discovery; not-ready refusal | Invalid explicit file fails closed | `bun run smoke:opencode-entry surfaces.selection` |
| AC-7 | Committed `generated/`, temp catalog copies, temp HOME, XDG and project trees | Unit assertions | Every failure row | `bun test --timeout 60000 tools/toolu-opencode/src/surfaces tools/toolu-opencode/src/inventory tools/toolu-opencode/src/select tools/toolu-opencode/src/plugin` |
| AC-8 | The repository | Green gate | Claude Code and Codex regressions | `bun run test` |

## Documentation impact

- **`docs/opencode.md`.** Replace step 5's manual wiring with automatic discovery: precedence, notes, the shared-procedure permission and how to override it, and that toolu writes no surface files. Document the global selection file, its order, and the invalid-file failure. Cover enable, disable, update and remove. Add the migration line: remove an old manual `skills.paths` entry or copied files, because a user copy wins over toolu's.
- **`tools/toolu-opencode/README.md`.** The surface section and install selection text.
- **Generator wording.** `scripts/lib/emit.ts` replaces "handled separately by OP-11" with the `config` hook. Then the full `bun run generate:opencode-surface` refreshes `GENERATED-NOTES.md` and the generated copies `generated/resources/repo/docs/{opencode,opencode-host-contract}.md`, so `check:opencode-surface` stays green.
- **`docs/opencode-host-contract.md`.** The two host constraints owned by OP-11 ("Install through…" and "Surfaces can be registered…") name the shipped mechanism. They are edited in `contract/capability-matrix.json` and regenerated with `bun run check:opencode-host --write-doc`, or edited directly where the text is not generated.
- **`AGENTS.md`.** None. Its key-file table lists no `tools/toolu-opencode` internals, and the docs-sync gate is satisfied by `docs/opencode.md`.

## Open Questions

None blocking. Each decision below was made by the #345 epic worker and is recorded above with its evidence. The PR reviewer (the repository owner) signs off on them at merge, and the PR body lists them for that sign-off:

- Configure, not materialize.
- User-wins deep merge for agents and commands, and keep-the-user's-copy for skills.
- The project selection replaces the global one.
- An invalid explicit selection is not ready. This changes today's silent enable-all, and `docs/opencode.md` calls it out.
- The default-layer `external_directory` allow for `generated/resources/*`, added only when no user rule could match, and accepting the edit exposure.

The skill-scan mirror is pinned to `opencode-ai@1.18.34`. A pin bump re-runs `surfaces.skill-roots` and `surfaces.precedence`, beside the re-probe the host-contract pin-bump steps already require.
