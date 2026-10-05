# Generated surface notes

Do not edit by hand. Regenerate with `bun run generate:opencode-surface`.

The catalog covers all 12 plugin manifests. At runtime the plugin's `config` hook contributes the selected plugins' skills, agents and commands from this directory; nothing is copied into OpenCode's discovery directories (see `docs/opencode.md`).

## Catalog coverage

- `ast-grep`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `brainstorm`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `delivery-flow`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `epic-orchestrator`: generated; 1 skill(s), 0 agent(s), 1 command(s)
- `jev`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `pr-babysit`: generated; 1 skill(s), 0 agent(s), 1 command(s)
- `python-quality`: no-surface; 0 skill(s), 0 agent(s), 0 command(s) — Plugin provides hooks only; no skill, agent, or command source.
- `rust-quality`: no-surface; 0 skill(s), 0 agent(s), 0 command(s) — Plugin provides hooks only; no skill, agent, or command source.
- `statusline`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `toolu`: generated; 6 skill(s), 5 agent(s), 2 command(s)
- `toolu-review`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `ts-quality`: no-surface; 0 skill(s), 0 agent(s), 0 command(s) — Plugin provides hooks only; no skill, agent, or command source.

## Excluded host-specific surfaces

- `plugins/statusline/commands/setup.md`: Claude Code statusLine setting in settings.json; OpenCode has no statusline setting, so the persistent statusline is host-specific. Use the statusline-status skill. Owner: OP-25 (#359).

## Path rewrites

- Claude plugin-root tokens → the owning plugin's `${TOOLU_PLUGIN_ROOT_<PLUGIN>}`: 0.
- Claude config-root tokens → OpenCode config root: 1.
- Typed source paths → `${TOOLU_OPENCODE_ROOT}/generated/…` paths: 3.
- Explicit skill invocations → generated skill IDs: 2.

## Runtime environment

The OpenCode adapter's `shell.env` hook gives every bash call these variables (see `docs/opencode.md`):

- `TOOLU_PLUGIN_ROOT_<PLUGIN>`: each enabled plugin's directory, the name upper-cased with `-` as `_`.
- `TOOLU_PLUGIN_ROOT`: the toolu core plugin's directory.
- `TOOLU_OPENCODE_ROOT`: the `@toolu/opencode` package directory, which holds `generated/`.
- `TOOLU_CONFIG_DIR`: the project's data root, where plugin helpers are published.
- `TOOLU_USER_CONFIG_DIR`, `TOOLU_HOST_OVERRIDE`, `TOOLU_PROJECT_CONFIG_DIRNAME`, `TOOLU_SETTINGS_DIR` and `TOOLU_BUN`; `PATH` gains Bun's directory only when it has no `bun`.

## Stripped frontmatter

- (none)

## Literal `.claude` references (not rewritten)

- `~/.claude/epics/<owner>-<repo>-<n>/`, Codex: `$CODEX_HOME/toolu/epics/…`,
