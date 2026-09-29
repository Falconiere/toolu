# Hook module registry

Domain plugins (ast-grep and the language-quality plugins) add rules to toolu's core `PreToolUse` and `PostToolUse` dispatchers through a runtime registry in the host's config directory. `@toolu/core/registry` ([#257](https://github.com/Falconiere/toolu/issues/257)) is the TypeScript version of it: each contribution is one bundled ESM module, and the dispatcher imports it in-process instead of spawning `bash` per module. Bash modules keep working through the bash dispatcher until their plugin is ported (#258, #259, #265 to #268).

## Layout

```
<config root>/toolu/pre-tools.d/<spec>__<name>.js    # tool/pre, also shell/pre
<config root>/toolu/post-tools.d/<spec>__<name>.js   # tool/post
```

`<config root>` is `TOOLU_CONFIG_DIR`, else the host's own root (`CLAUDE_CONFIG_DIR` or `~/.claude`, `CODEX_HOME` or `~/.codex`, …). `<spec>` is the owning plugin's `name@marketplace`. It may contain `.` but never `__`, `/` or whitespace. The directories are the same ones the bash `*.sh` modules use.

## Module contract

A module's default export:

```ts
import { defineRegistryModule } from "@toolu/core/registry";

export default defineRegistryModule({
  spec: "ts-quality@toolu",   // must match the file name
  name: "ts-quality",          // must match the file name
  event: "tool/post",          // must match the directory
  async run(event, ctx) {      // event: the NormalizedEvent (tool/pre, shell/pre or tool/post)
    return { kind: "allow" };  // a Decision from @toolu/core/decision
  },
});
```

`ctx` carries `host`, `env`, `configRoot`, `projectRoot`, the host's `raw` payload and, for one path of a split multi-file patch, `edit`. It is an interface, so later layers add optional fields (the parsed shell command from #284) without breaking modules. A module owns its side effects: a quality module records or clears its own gate entry, once per event, as its bash predecessor did.

## Registering

The plugin's SessionStart entry is a bundle of its own (`hooks/src/register.ts` → `hooks/dist/register.js`, wired with the generated launcher):

```ts
import { join } from "node:path";
import { runRegisterHook } from "@toolu/core/registry";

await runRegisterHook("ts-quality@toolu", [
  { name: "ts-quality", event: "tool/post", bundle: join(import.meta.dir, "post-tool-use.js") },
]);
```

`registerModules` copies each bundle to its target through `<target>.tmp.<pid>` and `rename`, and only when the bytes differ. It then removes every other `<spec>__*.js` and `<spec>__*.sh` in both directories: a plugin's pre-port `.sh` disappears the first time its TypeScript version registers. It also removes `<spec>__*.{js,sh}.tmp.*` residue older than a minute. Other plugins' entries are never touched. A missing bundle keeps the existing registry copy (stale enforcement beats none). The hook drains stdin, prints nothing on stdout, reports each failure on one stderr line and always exits 0.

On Codex, `pruneInactiveModules` removes `.js` and `.sh` modules of plugins that the ready plugin snapshot lists as absent. It never removes a symlink, and a missing or stale snapshot prunes nothing. The core SessionStart hook runs it once per session once #263 lands; until then the bash prune covers `.sh` modules, and dispatch-time gating skips an absent plugin's `.js` modules.

## Dispatching

`runRegistry(event, ctx, { fallback?, warn? })` reads `<ctx.configRoot>/toolu/<dir>.d` and returns one outcome per module it reached, in byte order of file names (bash glob order under `LC_ALL=C`):

| Situation | Outcome |
|---|---|
| Plugin definitively not installed (Claude `installed_plugins.json`, Codex snapshot) | `skipped: inactive`. An unreadable record fails open, and Cursor, Hermes and OpenCode count as installed |
| `.sh` module whose spec also has a `.js` module | `skipped: shadowed`, so one plugin never writes the gate twice per event |
| Other `.sh` module | passed to `fallback` (the bash bridge, #258), else `skipped: bash` |
| `.js` module | imported (fresh when its bytes change), checked against its file name, `run`, decision validated: `decision` |
| Import error, throw, rejection, contract mismatch, non-decision | `error`, plus `toolu-registry: module <file> failed: …; output skipped` on stderr; the walk continues |
| `deny` on a pre-tool event, `post_block` on a post-tool event | recorded, then the walk stops, as in `dispatch.sh` |
| File not named `<spec>__<name>.{js,sh}` | never run; one stderr warning |

Merging outcomes (deny over ask over advisory) and encoding them for the host belong to the dispatcher.

## Import cost

Measured by `packages/toolu-core/src/registry/__tests__/registry-import-cost.test.ts` on an Apple M2 Max, macOS 26.6.2, Bun 1.4.2. The test runs 20 modules, each a separate 112.8 KB bundle that inlines the state layer and zod, through `runRegistry` in a fresh `bun` process, over 15 measured runs of the whole set.

| Measure | p50 | p95 |
|---|---|---|
| One module, import + validate + run, inside the process | 4.8 ms | 9.4 ms |
| Whole-process marginal cost per module (20 modules vs none) | 5.6 ms | — |
| One bash registry module under the bash dispatcher (one `bash` spawn), same run | 4.8 ms | — |

A bundled module costs about what the bash module it replaces cost. Most of it is parsing that bundle's own copies of `@toolu/core` and zod, so each registry module should import only the core entries it uses.
