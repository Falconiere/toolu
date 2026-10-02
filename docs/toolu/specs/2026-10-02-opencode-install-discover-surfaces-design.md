# OpenCode install and discovery of generated surfaces — Design

**Date:** 2026-10-02   **Status:** Draft   **Author:** Claude (epic worker, #345)   **Topic:** The OpenCode plugin contributes the enabled plugins' generated skills, agents and commands through the host's native discovery, with explicit project/global selection, deterministic precedence and no duplicates (OP-11)

## Problem

`tools/toolu-opencode/generated/` holds a valid OpenCode catalog for all 16 plugins (#344), but it is package content, not a discovery location.

- **Nothing reaches the host.** `docs/opencode.md` step 5 tells users to "wire OpenCode to those paths" by hand, and the plugin contributes nothing. A clean install therefore exposes no toolu skill to the native `skill` tool and no toolu agent or command.
- **Manual wiring is unsafe.** Copying the generated files into `.opencode/` breaks the skills' relative resource links (`../../resources/…`). It ignores the enabled-plugin selection, leaves stale copies after an update or removal, and can duplicate skills.
- **Duplicate skills are nondeterministic on the pinned host.** `opencode-ai@1.18.34` parses every `SKILL.md` concurrently. The last parse to finish wins, with only a `duplicate skill name` warning.
- **Selection is project-only and fails open.** `.opencode/toolu/plugins.json` is the only explicit selection. An invalid file is silently ignored, which enables every installed plugin.

Host facts this design relies on (pinned source at tag `v1.18.34`, plus the live probes `surface.config-hook` and `surface.files`; brainstorm `docs/toolu/brainstorms/2026-10-02-opencode-install-discover-surfaces.md`):

1. The host merges `opencode.json` and every config directory's `{command,commands}/**/*.md` and `{agent,agents}/**/*.md` before plugins load. It then calls each plugin's `config(cfg)` with that same merged object. A plugin can change `cfg.command`, `cfg.agent` and `cfg.skills.paths`, and those changes apply in the same start. Files a plugin writes during init are only seen on the next start.
2. Skills are scanned lazily after plugin init. The scan covers:
   - `$HOME/.claude/skills/**/SKILL.md`, unless `OPENCODE_DISABLE_EXTERNAL_SKILLS`, `OPENCODE_DISABLE_CLAUDE_CODE` or `OPENCODE_DISABLE_CLAUDE_CODE_SKILLS` is set;
   - `$HOME/.agents/skills/**/SKILL.md`, unless `OPENCODE_DISABLE_EXTERNAL_SKILLS` is set;
   - every `.claude`/`.agents` directory from the instance directory up to the worktree, under the same flags;
   - every config directory's `{skill,skills}/**/SKILL.md`. The config directories are the XDG `opencode` config dir, the `.opencode` directories from the instance directory up to the worktree (unless `OPENCODE_DISABLE_PROJECT_CONFIG`), `$HOME/.opencode` and `OPENCODE_CONFIG_DIR`;
   - each `cfg.skills.paths` entry (`**/SKILL.md`, with `~/` expanded and relative paths resolved against the instance directory);
   - remote `cfg.skills.urls`.
3. A skill's frontmatter `name` keys it. The host tells the model that relative paths in a skill resolve against the skill's own directory.
4. Agent config entries override defaults field by field (`value ?? default`), and permissions merge. `disable: true` removes an agent. Config sources deep-merge, and later sources win per key.

## Non-Goals

1. CLI management (`toolu install --host opencode`, which edits `opencode.json`): OP-26 (#360).
2. Context and instruction delivery (`instructions`, system/prompt/compaction hooks): OP-07 (#341).
3. What each leaf skill does once loaded, and leaf helper behavior: OP-12 to OP-25.
4. The published package's file list and independent loadability: OP-27 (#361). Mandatory real-host CI: OP-28 (#362). The final install and migration guide: OP-29 (#363). This change updates the existing docs to match its behavior.
5. Detecting duplicates among remote `cfg.skills.urls` skills, which the host pulls only after toolu's hook has run.
6. Claude Code and Codex behavior. Only `tools/toolu-opencode`, its docs and the OpenCode live harness change.

## Architecture

**Decision: configure, do not materialize.** On the ready path, the plugin's `config` hook adds the selected plugins' contributions to the host's merged config. No file is written into any OpenCode discovery directory or the user's repository.

- Skills: one absolute `skills.paths` entry per selected skill, pointing at the skill's own directory inside the package's `generated/skills/<id>/`. That keeps its relative resource links valid.
- Agents and commands: `cfg.agent[id]` and `cfg.command[id]` entries built from the generated Markdown, exactly as the host's own loaders build them. An agent is `{...frontmatter, prompt: body.trim()}`, and a command is `{...frontmatter, template: body.trim()}`.

Why this approach (Jev `ask`, config hook 0.58 over the hybrid at 0.40):

- It is the only route that reaches agents and commands on the first start (fact 1).
- It keeps skill links intact (fact 3).
- It leaves nothing to orphan when the package is removed, and nothing for the user to commit or clean.

"Owned contributions" are therefore exactly the entries this instance adds, which are recomputed at every start. Enable, disable, update and remove take effect at the next start, and they cannot touch anything toolu did not add.

**Selection** (`src/inventory/selection.ts`):

1. When `<project>/.opencode/toolu/plugins.json` exists, it alone decides.
2. Otherwise, when the new global file `<config root>/toolu/plugins.json` exists, it decides. The config root is `opencodeConfigRoot`: `TOOLU_CONFIG_DIR`, then `TOOLU_OPENCODE_HOME`, then `$XDG_CONFIG_HOME/opencode`, then `~/.config/opencode`.
3. Otherwise every installed plugin is enabled, as today.

In every case, `skills.<name>: false` in the project `toolu.config.json` still removes a plugin, and manifest dependencies still close (`selectPluginsWithDependencies`).

- An explicit file that is unreadable, is not JSON, or fails `{version: 1, enabled: string[]}` makes toolu **not ready** (Jev 1.0).
- A name the catalog does not install becomes a startup note.

**Surface plan** (`src/surfaces/`, built in `prepareEnforcement` before readiness):

- `readSurfaceCatalog(generatedDir)` validates `generated/opencode.toolu.json` with Zod. It checks the shape, the ID rule `^[a-z0-9]+(-[a-z0-9]+)*$` (at most 64 characters), uniqueness per kind across the catalog, and that every path resolves inside `generatedDir` to an existing file. Each skill path must be `skills/<id>/SKILL.md`.
- `planSurfaces(catalog, selectedNames)` takes the selected plugins in catalog order and their surfaces in catalog order. It parses each agent and command file's frontmatter (`key: <JSON>` lines in the generator's canonical format) and validates them against the fields the generator emits. A selected plugin missing from the catalog becomes a note.
- Any catalog or file failure makes toolu not ready: `surfaces: <reason>`.

**Applying** (`src/surfaces/apply.ts`, called from the `config` hook):

- **Skills.** `existingSkillNames(scope)` mirrors the host's scan from fact 2: the same roots, flags, patterns and up-walk (from `directory` to `worktree`, inclusive, or to `/` when the worktree is `/`). It reads each `SKILL.md`'s frontmatter `name` with `Bun.YAML.parse`, retrying with the host's colon sanitizer, and maps each name to its first location. A toolu skill whose ID is already a name there is not added; that copy is the user's and stays in force. Otherwise its directory is appended to `cfg.skills.paths`, keeping the existing paths and every other `cfg.skills` key.
- **Agents and commands.** When an entry with the same ID already exists, toolu's entry becomes the lowest layer: `cfg.agent[id] = deepMerge(toolu, user)`. Every key the user set wins, and nested plain objects merge, so permissions merge key by key. Arrays and scalars come from the user's entry. Otherwise toolu's entry is added. Entries toolu does not own are never read or written (Jev: user-wins merge 0.93).
- **No throw.** The hook never throws: an unexpected error is caught and logged at error level. Each kind (skills, agents, commands) is computed first and assigned once, so each kind is applied whole or not at all.

**Reporting.** The host log gets one line per start:

- `toolu: surfaces: <s> skills, <a> agents, <c> commands from <p> plugins (<source>)`, where source is `project selection`, `global selection` or `all installed plugins`.
- When there are any, one bounded `toolu: surface notes: …` line, such as `skill toolu-debug kept from <path>` or `agent toolu-quick-task merged under your config`.

**Wiring:**

- `HostBinding` gains `worktree`, the host's raw value.
- `Enforcement` (ready) gains `surfaces: SurfacePlan`.
- `createTooluHooks` adds `config` beside `tool.execute.before` and `shell.env` only when ready. A not-ready instance and the duplicate instance (`claimInstance`) contribute no surfaces, so both routes together still yield one set.

## Interfaces / Schema

```ts
// src/inventory/selection.ts
export type SelectionSource = "project" | "global" | "default";
export function resolveEnabledPluginNames(
  pluginsRoot: string,
  projectRoot: string,
  globalConfigRoot?: string,            // omitted: no global file is read
):
  | { ok: true; enabled: Set<string>; source: SelectionSource; path?: string; unknown: string[] }
  | { ok: false; reason: string };

// src/select/resolve.ts — the success arm gains the selection facts
export type SelectResult =
  | { ok: true; plugins: PluginManifest[]; source?: SelectionSource; unknown?: string[] }
  | { ok: false; reason: string; missingDependency?: string };
export function selectPluginsWithDependencies(
  pluginsRoot: string, projectRoot: string, globalConfigRoot?: string): SelectResult;

// src/host/roots.ts
export function opencodeGlobalPluginSelectionPath(configRoot: string): string; // <root>/toolu/plugins.json

// src/surfaces/catalog.ts
export type CatalogEntry = { id: string; file: string /* absolute */ };
export type CatalogPlugin = { name: string; skills: CatalogEntry[]; agents: CatalogEntry[]; commands: CatalogEntry[] };
export function readSurfaceCatalog(generatedDir: string):
  { ok: true; plugins: CatalogPlugin[] } | { ok: false; reason: string };

// src/surfaces/plan.ts
export type SurfacePlan = {
  plugins: string[];                                  // selected plugins found in the catalog
  skills: Array<{ id: string; plugin: string; dir: string }>;
  agents: Array<{ id: string; plugin: string; entry: Record<string, unknown> }>;
  commands: Array<{ id: string; plugin: string; entry: Record<string, unknown> }>;
  notes: string[];
};
export function planSurfaces(generatedDir: string, selected: readonly string[]):
  { ok: true; plan: SurfacePlan } | { ok: false; reason: string };

// src/surfaces/existing-skills.ts
export type SkillScanScope = {
  directory: string; worktree: string; env: Record<string, string>; configuredPaths: readonly string[];
};
export function existingSkillNames(scope: SkillScanScope): Map<string, string>; // name -> first SKILL.md path

// src/surfaces/apply.ts
export type SurfaceReport = {
  skills: string[]; agents: string[]; commands: string[];       // ids toolu contributed
  kept: Array<{ id: string; location: string }>;                // user skills that won
  merged: Array<{ kind: "agent" | "command"; id: string }>;     // toolu entry under a user entry
  notes: string[];
};
export function applySurfaces(config: object, plan: SurfacePlan, scope: Omit<SkillScanScope, "configuredPaths">): SurfaceReport;
```

- **Global selection file:** `<config root>/toolu/plugins.json` with `{ "version": 1, "enabled": ["toolu", …] }`, the same strict schema as the project file.
- **Package export:** `"./surfaces": "./src/surfaces/index.ts"`.

## Failure modes and edge cases

| Input | Behavior |
|---|---|
| Project selection file invalid (bad JSON, wrong version, extra key, non-string name) | Not ready: `plugin selection: invalid <path>: <detail>`. Every tool call is refused, and no surfaces are added |
| Project file absent, global file invalid | Not ready, naming the global path |
| Project file present (even `enabled: []`) and global file present | The project file decides. The global file is not read, so a broken global file does not matter there |
| Enabled name not installed | Dropped, with the note `enabled plugin "<name>" in <path> is not installed` |
| `generated/opencode.toolu.json` missing, not JSON, or schema-invalid; duplicate ID; path outside `generated/`; missing file; bad agent or command frontmatter | Not ready: `surfaces: <reason>` |
| Selected plugin absent from the catalog (a `TOOLU_REPO_ROOT` newer than the package) | Note `no generated surface for plugin "<name>"`, other plugins still apply |
| A same-name skill exists in any scanned location (user copy, manual `skills.paths` to an older clone, `~/.claude/skills`) | toolu adds no path for it, and the note names the winning location. The host sees one copy, so precedence is deterministic |
| User `SKILL.md` with unparsable frontmatter or no string `name` | Not counted. The host also skips it, so toolu's copy is the only one |
| Unreadable or missing scan root | Skipped silently, as in the host's scoped scans |
| User `agent.<id>` or `command.<id>` (partial or full) | Deep-merged over toolu's entry with the user's keys winning; note `merged under your config`. `disable: true` removes the agent natively |
| `cfg.skills` present but not `{paths?: string[]}` | Skills are not added, with the note `skills config unreadable`; agents and commands still apply |
| Unexpected exception in the hook | Caught. Error log `toolu: surfaces not applied: <reason>`; enforcement is unaffected |
| npm route and local shim both loaded | Only the admitted instance has a `config` hook, so each surface appears once (and the existing gate dedup holds) |
| Not ready for any reason | No `config` hook, so no toolu surfaces |
| Two OpenCode processes on one project | Each changes only its own in-memory config. Nothing is written, so there is no race |
| Plugin disabled, package updated or package removed | The next start recomputes from the current selection and package. Nothing persists, so nothing is left behind |
| Paths with spaces | Absolute paths are passed unchanged |

## Acceptance criteria

- **AC-1:** Setup: a clean isolated profile with the packed `@toolu/opencode` tarball through the npm route, and the project selection `["pr-babysit"]`. Expected:
  - Of all catalog skill IDs, the pinned host's `debug skill` lists exactly the 7 of the closure {pr-babysit, toolu}, each located under the package's `generated/skills/<id>/`. No skill of an unselected plugin (for example `jev-jev`) is listed. The host's built-in skill is ignored.
  - `GET /agent` lists the 5 toolu agents, and `GET /command` lists the 3 commands of the closure.
  - A scripted session calls the native `skill` tool for `toolu-debug` and the tool result contains that skill's body. The recorded model request's `skill` tool description names `toolu-debug`.
  - The project's `.opencode/` gains no `skills`, `agents` or `commands` directory.
- **AC-2:** The same project is restarted with the selection changed to `["jev"]`. The host lists `jev-jev` and none of the AC-1 surfaces, and the project's `.opencode/` still has no surface files. With toolu removed from `opencode.json`, no toolu skill, agent or command is discovered.
- **AC-3:** A project defines its own `.opencode/skills/toolu-debug/SKILL.md`, which differs from toolu's, plus an unrelated `.opencode/skills/my-skill/SKILL.md`. It sets `agent.toolu-quick-task.description` and `command.toolu-commit-1e9b92d5.template` in `opencode.json`. Expected:
  - `debug skill` lists `toolu-debug` once, at the user's path, and `my-skill` unchanged.
  - The resolved agent has the user's description with toolu's prompt and permission. The resolved command has the user's template.
  - The user's files are byte-identical afterwards.
  - The host log notes the kept skill and both merges.
- **AC-4:** The npm route and a local `.opencode/plugins` shim are loaded together on one project. Every toolu skill appears exactly once in `debug skill`, the agents appear once, and the host log has one `toolu: surfaces:` line and one `toolu: duplicate load skipped` line.
- **AC-5:** Selection sources:
  - With only a global `<XDG config>/opencode/toolu/plugins.json` `["jev"]`, the host discovers `jev-jev` and the log says `global selection`.
  - Adding a project file `["toolu-review"]` makes it discover `toolu-review-review` and not `jev-jev`.
  - An invalid project file refuses a bash call with `toolu: not ready: plugin selection: invalid …/plugins.json`, and no toolu skill is listed.
- **AC-6:** Hermetic Bun tests over the committed catalog and real temp directories prove:
  - every catalog failure listed above makes `prepareEnforcement` not ready;
  - plan order follows the catalog;
  - every host scan root and flag in fact 2 is honored by `existingSkillNames`;
  - `applySurfaces` keeps unrelated config entries, existing `skills.paths` and user keys, and adds toolu's entries;
  - the `config` hook never throws on malformed config.
- **AC-7:** `bun run test` passes, which includes surface, bundle and pack drift, `check:opencode-host`, Claude and Codex suites, and guardrails. Claude Code and Codex code paths are unchanged.

## Acceptance evidence

| AC | Real input | Observable result | Boundary | Check |
|---|---|---|---|---|
| AC-1 | Packed tarball, isolated pinned host, `["pr-babysit"]` | `debug skill` names and locations; `/agent` and `/command`; skill tool output and recorded tool description; `.opencode` listing | An unselected plugin's skill is absent | `bun run smoke:opencode-entry surfaces.npm-clean` |
| AC-2 | Same project, selection changed, then the plugin removed | New set only; no files; no toolu surfaces without the plugin | Removal leaves nothing | `bun run smoke:opencode-entry surfaces.reselect` |
| AC-3 | User skill, unrelated skill, `opencode.json` agent and command overrides | One `toolu-debug` at the user's path; merged agent; user template; unchanged bytes; notes | Partial override keeps toolu's prompt | `bun run smoke:opencode-entry surfaces.precedence` |
| AC-4 | npm spec plus local shim | Each skill once; one surfaces line; one duplicate line | Two module copies | `bun run smoke:opencode-entry surfaces.both-routes` |
| AC-5 | Global file, then project file, then an invalid project file | Global, then project discovery; not-ready refusal | Invalid explicit file fails closed | `bun run smoke:opencode-entry surfaces.selection` |
| AC-6 | Committed `generated/`, temp catalogs, temp HOME, XDG and project trees | Unit assertions | Every failure row above | `bun test tools/toolu-opencode/src/surfaces tools/toolu-opencode/src/inventory tools/toolu-opencode/src/select tools/toolu-opencode/src/plugin` |
| AC-7 | The repository | Green gate | Claude and Codex regressions | `bun run test` |

## Documentation impact

- **`docs/opencode.md`.** Replace step 5's manual wiring with automatic discovery, the precedence rules and the notes. Document the global selection file and its order, the invalid-file failure, and how disable, update and remove behave. State that toolu writes no surface files.
- **`tools/toolu-opencode/README.md`.** The surface section says the plugin contributes the enabled surfaces through the `config` hook, and documents the global selection file.
- **Generator text.** `scripts/generate-surface.ts` writes `generated/GENERATED-NOTES.md`; its "handled separately by OP-11" lines point to the `config` hook instead, and the file is regenerated.
- **`docs/opencode-host-contract.md`.** The "Install through…" and "Surfaces can be registered…" host constraints now name the shipped mechanism, edited through `capability-matrix.json` and `--write-doc` where generated.
- **`AGENTS.md`.** Key files gain `tools/toolu-opencode/src/surfaces/`, if the docs-sync gate requires it.

## Open Questions

None blocking. Decisions recorded above, made from the issue, the epic and the pinned host source:

- config hook rather than materializing;
- user-wins deep merge for same-name agents and commands, and skip-and-keep for same-name skills;
- the project selection replaces the global one;
- an invalid explicit selection is not ready.

Mirroring the skill scan is pinned to `opencode-ai@1.18.34`. A pin bump re-runs `surfaces.precedence`, and the host-contract pin-bump procedure already requires re-probing.
