# <NAME>

<ONE-LINE-SUMMARY — lift the shared Claude/Codex plugin.json `description`.>

## Install

```
/plugin install <NAME>@toolu
```

**Prerequisite:** [Bun](https://bun.sh) 1.4.x on `PATH`. See [docs/runtime.md](../../docs/runtime.md).

Hooks and registry contributions run as bundled TypeScript on Bun. Document the executable bundle and its host wiring when this plugin has a hook.

In skills, commands and agents, write plain `toolu <namespace> <verb>` for a
ported CLI verb. SessionStart supplies the native binary's absolute path for
that session when the agent command shell cannot resolve plain `toolu`; never
put a fixed binary path in static Markdown. Keep the current Bun invocation
until the namespace's port supplies that verb. See [docs/install.md](../../docs/install.md#agent-command-shell).

<DEPENDENCY-NOTE — if the Claude plugin.json lists a dependency, e.g. "Requires the `toolu` plugin." Otherwise: "Standalone, no dependencies.">

Keep `name`, `version`, and `description` identical in the plugin's `.claude-plugin/plugin.json` and `.codex-plugin/plugin.json`. Declare `./skills/` and `./hooks/hooks.json` in the Codex manifest when those directories exist, then add matching entries to both marketplaces.

## Crate

`crates/<NAME>` is this plugin's Rust library. It contributes the `toolu <namespace>` commands that this plugin's skills, commands, agents and `hooks.json` run; the generated reference is [docs/cli/](../../docs/cli/README.md). <NAMESPACE-NOTE — name the namespace and its verbs. Before the port: "Not ported yet; `toolu <namespace> planned` names the planned verbs." A Markdown-only plugin has a guide namespace with no verbs.>

## What it provides

- <SKILL/COMMAND/HOOK by name — what the user actually invokes or what fires.>
- <…one bullet per real surface; do not pad.>

<WIRING-OR-TOOL-NOTE — for external-tool wrappers, name the underlying binary/API
and how to get it (brew/curl/CLI/env var). For hook-only plugins, note how it
registers into the toolu engine. Omit this section if there is nothing to say.>
