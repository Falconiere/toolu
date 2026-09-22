# Portable frontmatter (Claude/Codex → OpenCode surface)

**Issue:** [#206](https://github.com/Falconiere/toolu/issues/206)  
**Generator:** `tools/toolu-opencode/scripts/generate-surface.ts`

OpenCode discovery uses generated Markdown under `tools/toolu-opencode/generated/`. This table documents how YAML frontmatter on source skills, agents, and commands is treated when emitting that surface.

| Field | Treatment | Rationale |
|-------|-----------|-----------|
| `name` | **Map** → surface local id (before `plugin--` prefix) | Canonical slug; must match `^[a-z0-9-]+$` when present. Falls back to directory/basename when omitted. |
| `description` | **Preserve** | Required for skills/agents in marketplace packaging; copied verbatim into generated frontmatter. |
| `tools` | **Preserve** | Agent tool allowlists are host-specific strings; OpenCode agents keep the same list until a host map exists ([#212](https://github.com/Falconiere/toolu/issues/212)). |
| `model` | **Preserve** | Routing hint (`haiku`, `sonnet`, `opus`, …); OpenCode may ignore or map later. |
| `argument-hint` | **Preserve** | Slash-command UX; harmless when unused. |
| `disable-model-invocation` | **Reject** (strip) | Claude Code skill flag with no OpenCode equivalent in phase 1; dropping avoids silent “disabled” semantics. |
| `user-invocable` | **Preserve** | Codex/Claude command visibility hint. |
| `allowed-tools` | **Map** → `tools` | Alias normalized to `tools` when `tools` is absent. |
| Unknown keys | **Reject** (strip + catalog) | Listed in `generated/GENERATED-NOTES.md` so authors can promote fields to **preserve** explicitly. |

## Body path rewrites

| Pattern | Treatment |
|---------|-----------|
| `` `${CLAUDE_PLUGIN_ROOT}` `` | **Map** → `` `${TOOLU_PLUGIN_ROOT}` `` | Lifecycle-only plugin root; OpenCode bootstrap must set `TOOLU_PLUGIN_ROOT` to the installed `plugins/<name>/` tree (workflows, hooks), not the generated skill mirror. |
| `` `${CODEX_HOME}` `` / `` `${CLAUDE_CONFIG_DIR}` `` | **Preserve** | Documented host paths; cataloged when `.claude` appears literally. |
| Literal `.claude/` or `.claude` path segments | **Catalog** | Not rewritten automatically; see `generated/GENERATED-NOTES.md`. |

## Collision policy

Surface ids are `` `<plugin>--<localId>` ``. When the same local id appears on multiple artifact kinds (e.g. skill `commit` and command `commit`), the first kind in stable order **agent → command → skill** keeps the bare id; later kinds receive `` `--<kind>` `` (e.g. `toolu--commit--skill`).
