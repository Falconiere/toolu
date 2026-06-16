# Toolu on opencode

toolu ships an opencode adapter (`opencode/extensions/toolu.ts`) that reuses
the same shell hook engine the Claude Code and pi installs use. The shell
scripts (quality gate, protected files, bash-commands, ts-quality,
rust-quality, comemory scope, ast-grep nudge) are unchanged — the adapter is
the only new code.

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│  opencode session                                           │
│                                                             │
│   session.created   ──►  adapter: run all register.sh       │
│   tool.execute.before  ──►  adapter: spawn pre-tools/mod.sh │
│   tool.execute.after   ──►  adapter: spawn post-tools/mod.sh│
│   session.compacting   ──►  adapter: spawn pre-compact.sh   │
│                                                             │
│   skills.paths:  ./plugins   (config wired by installer)    │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
              plugins/toolu/hooks/{pre,post}-tools/mod.sh
                              │  (existing — runs every
                              │   modules/*.sh + the
                              │   runtime registry at
                              │   ~/.config/opencode/toolu/)
                              ▼
       ┌──────────┬──────────┬──────────┬──────────┐
       │ ast-grep │ comemory │ ts-      │ rust-    │  ...
       │ @toolu   │ @toolu   │ quality  │ quality  │
       └──────────┴──────────┴──────────┴──────────┘
```

The adapter's responsibilities mirror the pi extension one-for-one:

| Step | opencode event | What runs |
|------|---------------|-----------|
| 1. Registry sync | `session.created` | Each plugin's `register.sh` mirrors its hook modules into `~/.config/opencode/toolu/{pre,post}-tools.d/`. Idempotent, content-hash skipping, atomic `cp + mv`. |
| 2. Pre-tool gate | `tool.execute.before` | Builds the Claude-Code-shaped `{tool_name, tool_input}` payload, spawns `pre-tools/mod.sh`, maps the response back: `permissionDecision: "deny"` → `throw`, advisory → metadata. Args are threaded through `output.metadata.tooluArgs` so the after hook can rebuild the payload. |
| 3. Post-tool gate | `tool.execute.after` | Rebuilds the payload, spawns `post-tools/mod.sh`, appends `additionalContext` to the tool's result text, surfaces the current gate status inline. |
| 4. Pre-compact | `experimental.session.compacting` | Spawns `pre-compact.sh`, pushes its reminder into `output.context`. |
| 5. Statusline | `/toolu-status` slash command | opencode has no statusline bar. The closest equivalent is a markdown command that reads `.opencode/tmp/quality-gate-status.json` and prints its contents. |

## Install

```bash
# Project install — installs into the current directory
git clone https://github.com/Falconiere/toolu
bash toolu/tooling/install-opencode.sh

# User install — installs into ~/.config/opencode/
bash toolu/tooling/install-opencode.sh --global

# Both are re-runnable; second runs report "up to date".
```

The installer:

1. Drops the adapter at `<target>/.opencode/plugins/toolu.ts`.
2. Copies every opencode-format agent from `opencode/agents/*.md` into `<target>/.opencode/agents/`.
3. Copies every opencode-format command from `opencode/commands/*.md` into `<target>/.opencode/commands/`.
4. Adds `skills.paths: ["<repo>/plugins"]` to `<target>/opencode.json` (creates the file if absent, merges into it if present, never clobbers user fields).
5. Runs every plugin's `register.sh` once with `TOOLU_CONFIG_DIR=~/.config/opencode` so the runtime registry is populated before the first session.

> **Restart opencode after the install.** Config and the plugin are loaded at
> session start, not hot-reloaded. The first `session.created` re-syncs the
> registry from the toolu checkout, so subsequent code changes in
> `plugins/*/hooks/` are picked up automatically without re-running the
> installer.

## Per-plugin behaviors

### Core (`toolu`)

The full gate runs identically: bash-command denylist, protected-file guard,
quality gate on every edit, commit gate, push review, plan ledger. The
adapter just translates opencode's tool names to the ones the shell engine
expects (`bash` → `Bash`, `edit` → `Edit`, `write` → `Write`, etc.) and
renames the `filePath` arg to `file_path` so the existing modules don't need
to know opencode exists.

### `ast-grep`, `comemory`, `ts-quality`, `rust-quality`, `git-better`

These are **modules-only plugins** on opencode — they contribute hook
modules to the core engine via their `register.sh`, but they don't need
their own adapter. The toolu adapter's `session.created` runs every
`register.sh`, populating the registry at `~/.config/opencode/toolu/`.

`comemory`'s setup wizard (`/toolu-comemory-setup`) invokes the published
wrapper at `${OPENCODE_CONFIG_DIR:-$HOME/.config/opencode}/comemory/setup.sh`,
which the register.sh symlinks in. Same pattern for `statusline` and `stats`.

### Skills

All 17 skills live under `plugins/*/skills/*/SKILL.md`. opencode discovers
them via the `skills.paths` config the installer adds — no per-skill
shipping needed. Frontmatter is already opencode-compliant (lowercase
hyphenated `name`, 1-1024 char `description`).

### Agents

`deep-explore` and `research-agent` live at `opencode/agents/*.md`. The
adapter file imports `@opencode-ai/plugin` only as a documentation
reference; the runtime types are defined locally to keep the dep surface
narrow.

### Commands

Slash commands live at `opencode/commands/*.md` and become `/toolu-commit`,
`/toolu-babysit`, `/toolu-stats`, etc. in the TUI. They use the
`agent: build` (default primary agent) and the opencode-native
`$ARGUMENTS` placeholder.

## Smoke test

After `install-opencode.sh`, verify the bundle landed correctly:

```bash
# 1. Adapter file
ls .opencode/plugins/toolu.ts

# 2. Agents + commands
ls .opencode/agents/ .opencode/commands/

# 3. opencode.json has skills.paths
cat opencode.json | jq '.skills.paths'

# 4. Runtime registry is populated
ls ~/.config/opencode/toolu/pre-tools.d/
ls ~/.config/opencode/toolu/post-tools.d/
```

Then start opencode and exercise the gate:

1. Type `run a deliberate lint error in src/foo.ts` — opencode should
   run the pre-tools gate, which will pass (no protected file, no bash).
2. When the agent writes the file, the post-tools gate should detect the
   lint error and append `[toolu-gate] gate: failing — ...` to the result.
3. Type `/toolu-status` — you should see the failing status with the
   reason.

If the pre/post hooks never fire, the adapter isn't loading — check the
opencode log for plugin load errors and the file `.opencode/plugins/toolu.ts`
exists at the expected path.

## Troubleshooting

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| Adapter file present but no hooks fire | `opencode.json` not picked up — opencode requires a project-level config when `.opencode/` exists | Run installer (creates `opencode.json` with `$schema`); restart opencode |
| Skills missing from `/skill` autocomplete | `skills.paths` not set or the path is wrong | Re-run installer; verify `jq .skills.paths opencode.json` includes an entry ending in `/plugins` |
| Pre-tool hook blocks everything | A module registered with a malformed `plugin-spec__<name>.sh` namespace; the dispatcher fails closed | `ls ~/.config/opencode/toolu/pre-tools.d/` — every file should match `<spec>__<name>.sh`; re-run each plugin's `register.sh` |
| `command not found: jq` at hook time | `jq` not on PATH | Install `jq` — it's a hard dep of every toolu hook |
| Post-tool gate appends advice to the wrong tool | opencode fires `tool.execute.after` for all tools; the adapter filters to bash/edit/write/patch | No action needed — non-target tools are no-ops |
| `permissionDecision: deny` doesn't block | A pre-tools module emitted `systemMessage` but not `permissionDecision` | Check the toolu hook output JSON; the adapter only treats `deny` (and exit 2) as blocking |
| Comemory wrapper symlink not at the expected path | `register.sh` symlink not run on session start | Restart opencode to retrigger `session.created`; or run the symlink manually: `ln -sf <plugin-root>/skills/agent-memory/scripts/comemory.sh ~/.config/opencode/comemory/comemory.sh` |

## What is NOT ported yet

- **User-prompt-submit / session-start / session-end / pre-compact-with-save** — the opencode plugin API doesn't have a 1:1 equivalent of Claude Code's `UserPromptSubmit` or `Stop`. The `pre-compact.sh` reminder hook is wired up (it fires on `experimental.session.compacting`); the other three Claude-Code-only event scripts (`session-start.sh`, `user-prompt-submit.sh`, `session-end.sh`) reference hardcoded `.claude/...` paths and need a port before they work on opencode. Follow-up work.
- **Statusline** — opencode has no statusline bar. `/toolu-status` is the closest inline equivalent.
- **Per-plugin `tools:` → `permission:` translation** — only the toolu agents (`deep-explore`, `research-agent`) have opencode-format mirrors in `opencode/agents/`. If a new Claude Code agent is added, it needs a matching opencode mirror at `opencode/agents/<name>.md`.
