# Toolu Config

Runtime opt-out for individual toolu components. No config file is
required — every component defaults to enabled. Drop in a file only to
disable what you do not want.

## Locations

- User-global: `~/.claude/toolu.config.json` (override the root with `TOOLU_CONFIG_DIR`
  or `CLAUDE_CONFIG_DIR`)
- Project override: `$CLAUDE_PROJECT_DIR/.claude/toolu.config.json` (override the
  directory name with `TOOLU_PROJECT_CONFIG_DIRNAME`)

Both are optional. When both exist they are deep-merged via `jq '. * .'`;
project values win on conflict. Missing keys default to **enabled**.

Requires `jq` (already a hard dependency of every toolu hook). With `jq`
absent or the JSON malformed, the loader warns once on stderr and falls
back to "all enabled".

## Schema

```json
{
  "version": 1,
  "skills":  { "<name>": true | false },
  "hooks":   { "<name>": true | false },
  "mcp":     { "<server>": true | false },
  "lang":    { "ts":   { "maxFileLines": 300, "maxFnLines": 60 },
               "rust": { "maxFileLines": 500, "maxFnLines": 50, "maxImplLines": 200 } },
  "docsSync": { "surfaces": ["README.md", "docs/*.md", "*/SKILL.md"],
               "surfaceExcludes": ["docs/releases/*"],
               "codeSurfaces": ["*.ts", "*.rs", "*.sh", "*plugin.json"] }
}
```

`version` is reserved for future schema bumps; v1 is the current value.
See `plugins/toolu/settings/toolu.config.example.json` for a fully-populated example.

### Quality thresholds (`lang`)

The rust-quality and ts-quality gates' line limits are not hardcoded. Each threshold resolves
with this precedence (first hit wins, always a positive integer):

1. **Project / user override** — the `lang.<ts|rust>.<key>` value above.
2. **Native linter config** (TS `maxFileLines` only) — the `max-lines` rule from
   the *active* linter's JSON config: `.oxlintrc.json` when oxc is detected,
   `.eslintrc.json` when eslint is (detection precedence is biome > oxc > eslint;
   biome has no `max-lines`, so it falls through to the default). Only the active
   linter's file is read — a repo carrying both (e.g. mid-migration) does not
   chain between them. All encodings parse: `N`, `["error", N]`,
   `["error", {"max": N}]`. Flat config `eslint.config.{js,mjs,ts}` is JavaScript
   and not parsed — it falls through.
3. **Built-in default** — TS `maxFileLines` 300 / `maxFnLines` 60; Rust
   `maxFileLines` 500 / `maxFnLines` 50 / `maxImplLines` 200.

The file-size limit counts real code only: `count_code_lines`
(`plugins/toolu/hooks/lib/detect.sh`) excludes blank lines and `//` + `/* */`
comments. It is a lexical heuristic, not a parser — a `//` inside a string literal
(e.g. a `"https://…"` URL) is treated as a line-ending comment, so a file dense in
such literals can count slightly low. The gate deliberately fails *toward*
flagging: when the scan ends mid-`/* */` (an unterminated block, or a `/*` inside a
string), it falls back to the raw line count rather than risk under-counting an
oversized file.

A value of `0`, a negative, or `"off"` is treated as "no override" and falls
through to the next layer — it does not mean a limit of zero. A stringified
positive integer (`"maxFileLines": "120"`) is accepted and coerced to a number,
so configs copy-pasted from sources that quote numbers still work. The gate never
invokes biome/oxc/eslint/prettier/clippy/rustfmt; detecting them only tunes
advisory wording. Resolver: `plugins/toolu/hooks/lib/quality-config.sh`.

### Docs-sync surfaces (`docsSync`)

The docs-sync backstop (`plugins/toolu/hooks/pre-tools/modules/docs-sync.sh`)
fires an **advisory** on `git push` when the branch diff changes code but no
documentation surface — a nudge to keep user-facing docs in sync with behavior.
It never blocks the push and is silenced by a diff-`sha`-keyed attestation the
agent writes to `.claude/tmp/docs-sync/<branch-slug>.json`.

Three glob sets tune it; each resolves *project/user override → built-in
default* (resolver `plugins/toolu/hooks/lib/docs-sync-config.sh`):

| Key | Default | Meaning |
|-----|---------|---------|
| `docsSync.surfaces` | `README.md`, `*/README.md`, `docs/*.md`, `*/SKILL.md` | doc files whose change **satisfies** the check |
| `docsSync.surfaceExcludes` | `docs/releases/*`, `*/docs/releases/*` | doc paths carved back out (release notes are per-release, not per-task) |
| `docsSync.codeSurfaces` | `*.ts`, `*.rs`, `*.sh`, `*/commands/*`, `*plugin.json`, `*.config.json` | code files whose change **demands** a doc touch |

Globs are matched with bash `case` fnmatch where a single `*` **crosses `/`** —
so `docs/*.md` already covers nested paths (which is *why* `surfaceExcludes`
exists: without it, `docs/*.md` would swallow `docs/releases/*.md`). A diff path
counts as a doc touch when it matches `surfaces` **and not** `surfaceExcludes`.
Setting any key replaces (does not merge with) that list's default.

### Recognized names

| Category | Names                                                                              |
|----------|------------------------------------------------------------------------------------|
| `skills` | `comemory`, `ast-grep` (the only skill keys any hook reads)                        |
| `hooks`  | `session-start`, `user-prompt-submit`, `pre-tools`, `post-tools`, `pre-compact`, `session-end` |
| `mcp`    | any MCP server name — e.g. `canva`, `figma`                                        |

Unknown names are silently ignored (forward compatible).

## Effects

- `skills.<name> = false`
  - The hooks that reference the skill behave as if its CLI is not
    installed AND they suppress the "not installed" warning. Skill files
    themselves stay on disk.
  - Concretely: `skills.comemory = false` silences the `MANDATORY: recall`
    hint in `UserPromptSubmit`, the comemory entry in the `SessionStart`
    "missing tools" warning, and the comemory reminder in `PreCompact` and
    `SessionEnd`. `skills.ast-grep = false` removes the ast-grep STOP /
    install-hint advisories in `search-nudge` (a registry module shipped
    by the ast-grep plugin); the generic `grep/rg → Grep tool` advisory
    still fires.

- `hooks.<name> = false`
  - The named hook exits early and emits nothing. Its stdin is drained
    first so Claude Code's IPC does not stall.
  - **Exception — `session-end` reminder is opt-IN**: the end-of-session
    comemory "save your learnings" reminder is OFF by default (the agent-memory
    protocol already saves proactively, so the Stop-time nag is redundant
    noise). It emits only when you set `hooks.session-end: true`. Every other
    hook is opt-out (on unless set to `false`).
  - **`session-end` also drives autonomous comemory maintenance** (a once-per-day
    `mine`/`prune`/`gc` pass, local and token-free) — this is opt-OUT, ON by
    default, independent of the opt-IN reminder. Setting `hooks.session-end: false`
    disables BOTH the reminder and the maintenance, keeping the "exits early,
    mutates nothing" contract. (`skills.comemory: false` also disables it.)

- `mcp.<name> = false`
  - Any `mcp__<name>__*` tool invocation is blocked at `PreToolUse` with
    `permissionDecision: "deny"`. The deny reason names the source
    (`see plugins/toolu/settings/mcp-blocklist.txt` for file entries, or
    `disabled via toolu config (mcp.<name>=false …)` for config
    entries) so users know where to undo it.
  - A `mcp-blocklist.txt` line may add an optional `-> <text>` redirect hint
    after the server prefix; when that server is denied the text is appended to
    the deny reason (e.g. to steer the user to a replacement skill). See
    `plugins/toolu/settings/README.md`.
  - Matcher wiring lives in `plugins/toolu/hooks/hooks.json`, which routes
    the `mcp__` prefix through `plugins/toolu/hooks/pre-tools/modules/mcp-blocker.sh`.

Agents and commands are loaded by Claude Code from the plugin manifest at
session start, so they cannot be toggled at runtime. A future `toolu
sync` command may rewrite the manifest from config; until then, install or
uninstall the plugin to control them.

## comemory version

toolu targets **comemory ≥ 0.8.0** (pinned as `COMEMORY_MIN_VERSION` in
`plugins/toolu/hooks/lib/detect.sh`). The wrapper uses comemory's full verb
surface — the retrieval-quality loop (`feedback`/`mine`/`tune`/`eval`/`prune`/
`gc`/`rebuild`) and **comemory** (`search-code`/`index-code`/`graph`). An older
binary lacks some of these and will error on them, so session start emits a
non-fatal upgrade WARN when it detects one. Basics (`search`/`save`/`list`)
still work. Upgrade with `brew upgrade Falconiere/tap/comemory` (comemory is not published to crates.io; the Homebrew tap or the curl installer are the canonical paths). Run `/comemory:setup` to verify and wire it.

## Examples

Disable comemory completely (no recall hint, no install nag):

```json
{ "version": 1, "skills": { "comemory": false } }
```

Disable a single hook only in this project:

```json
{ "version": 1, "hooks": { "user-prompt-submit": false } }
```

Block several MCP servers without touching the blocklist file:

```json
{ "version": 1, "mcp": { "canva": false, "figma": false } }
```

User-global disables, project-local re-enable:

```jsonc
// ~/.claude/toolu.config.json
{ "version": 1, "skills": { "ast-grep": false } }

// <repo>/.claude/toolu.config.json
{ "version": 1, "skills": { "ast-grep": true } }
```

## Shrink session footprint

Every enabled plugin's skill `description` fields and the Session Protocol load
into the model's context at session start — recurring cost that shrinks your
usable context window. To reclaim it:

- **Disable a whole plugin you don't use** — via Claude Code's `/plugin` UI or
  the `enabledPlugins` setting. This is what actually drops its skill
  descriptions from session load. Note the toolu `skills.<name>` config
  does **not** do this: it only gates hook behavior for `comemory`/`ast-grep`
  (see *Effects* above — `SKILL.md` files stay on disk and their descriptions
  still load).
- **MCP tools defer natively** — Claude Code loads their schemas on demand via
  tool search, so an idle connector costs little. Use `mcp.<server>: false` to
  block a noisy one entirely.

The harness caps its *own* injected footprint with
`plugins/toolu/scripts/context-budget.sh` (run in CI): word ceilings on the
Session Protocol + per-language docs and on every skill `description`, so the
baseline cannot silently regrow.
