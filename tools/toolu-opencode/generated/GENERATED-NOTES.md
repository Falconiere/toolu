# Generated surface notes

Do not edit by hand. Regenerate with `bun run generate:opencode-surface`.

The catalog covers all 16 plugin manifests. Runtime installation and enabled-plugin selection are handled separately by OP-11.

## Catalog coverage

- `agent-browser`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `ast-grep`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `brainstorm`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `context7`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `delivery-flow`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `epic-orchestrator`: generated; 1 skill(s), 0 agent(s), 1 command(s)
- `exa-search`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `jev`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `jira`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `pr-babysit`: generated; 1 skill(s), 0 agent(s), 1 command(s)
- `python-quality`: no-surface; 0 skill(s), 0 agent(s), 0 command(s) — Plugin provides hooks only; no skill, agent, or command source.
- `rust-quality`: no-surface; 0 skill(s), 0 agent(s), 0 command(s) — Plugin provides hooks only; no skill, agent, or command source.
- `statusline`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `toolu`: generated; 6 skill(s), 5 agent(s), 2 command(s)
- `toolu-review`: generated; 1 skill(s), 0 agent(s), 0 command(s)
- `ts-quality`: no-surface; 0 skill(s), 0 agent(s), 0 command(s) — Plugin provides hooks only; no skill, agent, or command source.

## Excluded host-specific surfaces

- `plugins/statusline/commands/setup.md`: Claude Code settings.json statusLine command has no OpenCode implementation in OP-10. Owner: OP-25 (#359).

## Path rewrites

- Claude plugin-root tokens → `${TOOLU_PLUGIN_ROOT}`: 1.
- Claude config-root tokens → OpenCode config root: 7.
- Typed source paths → generated paths: 3.
- Explicit skill invocations → generated skill IDs: 6.
- The OpenCode adapter must set `TOOLU_PLUGIN_ROOT` to the installed `@toolu/opencode` package root.

## Stripped frontmatter

- (none)

## Literal `.claude` references (not rewritten)

- This writes a ledger below the active host's `<repo>/.claude/tmp/plan-ledger/`
- `.claude/tmp/jira/plans/<KEY>.md` or `.codex/tmp/jira/plans/<KEY>.md`.
- `~/.claude/epics/<owner>-<repo>-<n>/`, Codex: `$CODEX_HOME/toolu/epics/…`,
- the active host's `<repo root>/.claude/tmp/push-review/` or
