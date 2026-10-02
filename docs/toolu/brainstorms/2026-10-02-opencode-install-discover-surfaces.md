# Brainstorm: OpenCode install and discovery of generated surfaces (#345)

Delivery mode, Full path (public interface, user-visible precedence, selection semantics).

## Capsule

- **Outcome:** With `@toolu/opencode` loaded through the npm route or a local shim, the host discovers the enabled plugins' generated skills, agents and commands on the first start. Skills come through the native `skill` tool, and agents and commands through the host's own agent and command lists. Nobody copies or wires paths by hand. Enabling, disabling, updating or removing a plugin changes only toolu's contributions at the next start. User definitions keep native precedence, and loading toolu twice never duplicates a surface.
- **Material defaults/non-goal:** The plugin's `config` hook injects the surfaces. No file is written into any OpenCode discovery directory. Selection uses the project file, then a new global file, then every installed plugin, and dependencies close as before. Out of scope: CLI management (OP-26), context and instructions delivery (OP-07), leaf behavior inside the skills (OP-12 to OP-25), and the published package's file list (OP-27).
- **Repository evidence:** The host contract lists "Surfaces can be registered from the plugin config hook (commands, agents, skills.paths, instructions) instead of writing into user directories. Owner: OP-11" (probes `surface.config-hook` and `surface.files`). The generated catalog `tools/toolu-opencode/generated/opencode.toolu.json` lists every surface per plugin. `selectPluginsWithDependencies` already closes dependencies, and `claimInstance` (`src/plugin/once.ts`) already admits one instance per directory.
- **Risk:** Duplicate detection for skills mirrors the host's scan list. A host pin bump that changes that list makes the mirror stale, so the live smoke re-proves it. Remote `skills.urls` cannot be scanned before the host pulls them.
- **Handoff:** spec.

## Host evidence (`opencode-ai@1.18.34` source at tag `v1.18.34`, plus the live probes)

- **Config before plugins.** `config/config.ts` merges `opencode.json` and every config directory's `{command,commands}/**/*.md` and `{agent,agents}/**/*.md` before any plugin loads. `plugin/index.ts` then calls each loaded plugin's `config?.(cfg)` with that same object. A command or agent file that a plugin writes during init is not seen until the next start. The `config` hook is the only route that works on the first session.
- **Skills scan lazily.** `skill/index.ts` scans after plugin init. It looks in `~/.claude/skills` and `~/.agents/skills` (unless `OPENCODE_DISABLE_EXTERNAL_SKILLS`, or `OPENCODE_DISABLE_CLAUDE_CODE[_SKILLS]` for `.claude`), in the project `.claude`/`.agents` directories from the instance directory up to the worktree, in each config directory's `{skill,skills}/**/SKILL.md`, and in `cfg.skills.paths` (`**/SKILL.md`, relative paths resolved against the directory). `cfg.skills.urls` are pulled remotely.
- **Duplicate skills are nondeterministic.** Every match is parsed concurrently, and the last parse to finish overwrites the entry with only a `duplicate skill name` warning. Precedence between two same-name skills is therefore undefined.
- **Skill base directory.** The host tells the model that relative paths in a skill resolve against its base directory. Generated skills link `../../resources/…`, so a skill copied out of `generated/` loses its links.
- **Native agent and command precedence.** Config sources deep-merge, and later sources win per key. The agent service applies config entries over built-in agents field by field, merging permissions, and `disable: true` removes an agent. Command entries replace built-ins by name. A skill also becomes a command unless a command already has its name.
- **Runtime.** The host binary embeds Bun 1.3.14 (`Bun.YAML`, `Bun.Glob` present). Frontmatter goes through `gray-matter`, with a sanitizing retry for unquoted colons.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| Data and state | No installed files. Contributions are computed at each start from the package's `generated/` catalog for the selected plugins and applied to the in-memory config | Config-hook timing; the skill base-directory rule | None persisted, so there is nothing to migrate or clean up |
| Interface | Global selection file `<config root>/toolu/plugins.json`, same schema as the project file. The project file wins when present | Existing project file `.opencode/toolu/plugins.json` and the `opencodeConfigRoot` resolver (#343) | A new user-facing file, which must be documented |
| Precedence | Skills: a name defined anywhere the host scans keeps the user's copy, and toolu adds none. Agents and commands: toolu's entry is the lowest layer and the user's entry deep-merges over it | Nondeterministic duplicate skills; native field-level agent overrides | The skill mirror can drift on a host pin bump |
| Failure behavior | An invalid explicit selection file, or an unreadable or invalid generated catalog, makes toolu not ready (every tool call refused with the reason). An unknown enabled name and every overlap or skip are startup notes. The `config` hook itself never throws | `load.init-throw` (fail-open init) and the existing deny-all contract | Stricter than today's silent fallback for a bad selection file; the docs explain it |
| Integration | `prepareEnforcement` builds the surface plan, `createTooluHooks` adds a `config` hook only on the ready path, and the duplicate instance returns no hooks | `once.ts`, `hooks.ts`, `enforcement.ts` | Parallel OP-07 work touches `hooks.ts`; keep the edit small |

## Alternatives (Jev `ask` over the issue text and the host evidence above)

- Mechanism: **config-hook injection** 0.58, hybrid (config hook for agents and commands, plus skills materialized into a toolu-owned directory) 0.40, materializing into `.opencode/` or the global config directory 0.01. Materializing fails "before the host scans them" for agents and commands, breaks relative skill links, adds files to the user's repository, and orphans them when the package is removed. The hybrid only adds a ledger and symlinks around the same per-skill directories without a behavioral gain, so it is rejected for simplicity.
- Same-name agent or command: **deep-merge with the user's keys winning** 0.93, over skipping toolu's entry entirely (0.07). This matches how OpenCode layers config and overrides built-in agents. A user who wants none of toolu's agent sets `disable: true`, as for built-ins.
- Selection scope: **the project file replaces the global file** 0.96. Rejected: union (0.04) and project only (0).
- Invalid explicit selection: **not ready** 1.0. Rejected: falling back with a note, and today's silent enable-all.
