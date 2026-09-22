# Generated surface notes

Do not edit by hand. Regenerate with `bun run generate:opencode-surface`.

## Path rewrites

- `${TOOLU_PLUGIN_ROOT}` replaces Claude `${CLAUDE_PLUGIN_ROOT}` (4 substitution(s) in phase-1 `toolu` set).
- Bootstrap must set `TOOLU_PLUGIN_ROOT` to the installed plugin directory (workflows, hooks).

## Stripped frontmatter

- (none)

## Literal `.claude` references (not rewritten)

- Codex or `${TOOLU_CONFIG_DIR:-${CLAUDE_CONFIG_DIR:-$HOME/.claude}}` on Claude
