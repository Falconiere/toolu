# <NAME>

<ONE-LINE-SUMMARY — lift the shared Claude/Codex plugin.json `description`.>

## Install

```
/plugin install <NAME>@toolu
```

<DEPENDENCY-NOTE — if the Claude plugin.json lists a dependency, e.g. "Requires the `toolu` plugin." Otherwise: "Standalone, no dependencies.">

Keep `name`, `version`, and `description` identical in the plugin's `.claude-plugin/plugin.json` and `.codex-plugin/plugin.json`. Declare `./skills/` and `./hooks/hooks.json` in the Codex manifest when those directories exist, then add matching entries to both marketplaces.

## What it provides

- <SKILL/COMMAND/HOOK by name — what the user actually invokes or what fires.>
- <…one bullet per real surface; do not pad.>

<WIRING-OR-TOOL-NOTE — for external-tool wrappers, name the underlying binary/API
and how to get it (brew/curl/CLI/env var). For hook-only plugins, note how it
registers into the toolu engine. Omit this section if there is nothing to say.>
