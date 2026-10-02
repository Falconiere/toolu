# OpenCode install

**Issue:** [#207](https://github.com/Falconiere/toolu/issues/207) (epic [#203](https://github.com/Falconiere/toolu/issues/203))  
**Status:** `@toolu/opencode` publishes to npm from v6.8.0. Install it with OpenCode's own plugin CLI — no clone, no `TOOLU_REPO_ROOT`:

```bash
opencode plugin add @toolu/opencode
```

> **Documented contract.** The plugin API at <https://opencode.ai/docs/plugins/> is pinned and probed in [opencode-host-contract.md](opencode-host-contract.md): `opencode-ai@1.18.34` with `@opencode-ai/plugin@1.18.34` ([#335](https://github.com/Falconiere/toolu/issues/335)). Since [#336](https://github.com/Falconiere/toolu/issues/336) the package entry is the documented plugin function. [#363](https://github.com/Falconiere/toolu/issues/363) replaces this install guide.

The package carries plugin manifests, settings, and committed Bun bundles, so the
adapter resolves its plugin root to its own package directory. Its default entry
(`exports["."]` / `main` → `./src/plugin/toolu.ts`) is a `PluginModule` `{ id: "toolu", server }`, so the host loads it
through an `opencode.json` `plugin` entry with no local shim. Choose which plugins are active with
`<project>/.opencode/toolu/plugins.json`:

```json
{ "version": 1, "enabled": ["toolu"] }
```

`delivery-flow`, `brainstorm`, `pr-babysit`, and `epic-orchestrator` ship in the committed OpenCode surface —
add them to `enabled` when you want those workflows (dependencies close
automatically).

`npx @toolu/plugins install --host opencode` does not drive this yet — the CLI
has no OpenCode adapter. Until it does, run the two steps above.

The git-clone flow below remains the contributor path, and is still how you work
against an unreleased checkout.

Enforcement runs in `tool.execute.before` through the native dispatcher. Covered calls are `bash`, `edit` and `write`; [#337](https://github.com/Falconiere/toolu/issues/337) extends that to every tool. A toolu refusal stops the call before it runs, and a toolu allow never overrides your own `permission` rules. Gate `ask` decisions deny for now, because the host has no ask channel ([#339](https://github.com/Falconiere/toolu/issues/339)). Host events without an OpenCode hook remain outside that scope; see the [host contract](opencode-host-contract.md).

Bun 1.4.x is a prerequisite on every host, Claude Code and Codex included; see the [runtime contract](runtime.md). Claude Code and Codex keep their marketplace installs. OpenCode calls the TypeScript core dispatcher in process. Its npm package ships committed bundles and their runtime data. Bootstrap reports NotReady when a selected plugin lacks a required Bun registration bundle.

## Prerequisites

| Requirement | Pin / note |
|-------------|------------|
| OpenCode CLI | `opencode-ai@1.18.34` ([host contract](opencode-host-contract.md)); `opencode --version` |
| Plugin SDK | `@opencode-ai/plugin@1.18.34`. The host provisions it into each config directory; toolu imports its types only |
| Bun | `1.4.x` (workspace `>=1.4.0 <1.5.0`; CI/docs baseline `1.4.2`) |
| git | Project and gate context |
| Platform | macOS and Linux ([#212](./conformance-report.md)); Windows **N/A** |

Do **not** target the legacy `opencode-ai@1.18.31` (V1) line. Pins and contracts: [`docs/portable-core.md`](portable-core.md).

Per-gate tool dependencies remain specific to the plugins you enable; see each plugin’s README and [`docs/config.md`](config.md).

## First install (git clone)

The pasteable prompt in the root [README § Install everything → OpenCode](../README.md#install-everything) (`<!-- install-everything:opencode -->`, mirrored in [`docs/plugins/index.md`](plugins/index.md)) uses the npm package above. The steps below are the contributor path against a checkout.

Install from a **release tag**, not `main`, unless you are developing toolu itself.

```bash
git clone https://github.com/Falconiere/toolu.git
cd toolu
git checkout vX.Y.Z   # latest from https://github.com/Falconiere/toolu/releases

bun install --frozen-lockfile
bun run check:opencode-surface   # optional sanity: generated mirror matches sources
```

Record the clone path — the OpenCode plugin must reach `plugins/` for manifests, bundles and settings, plus `tools/toolu-opencode/generated/`.

### Project wiring

In **your application repo** (not inside the toolu clone):

1. **Environment** — point toolu at the clone (required for gate enforcement):

   ```bash
   export TOOLU_REPO_ROOT=/absolute/path/to/toolu
   ```

   `TOOLU_ROOT` is an alias. You can set the same variable in your shell profile or OpenCode’s environment so every session sees it.

2. **OpenCode plugin loader** — local plugin under `.opencode/plugins/` (see [OpenCode plugins](https://opencode.ai/docs/plugins/)). Example `.opencode/package.json` using **file** dependencies into the clone (no global TS install):

   ```json
   {
     "dependencies": {
       "@toolu/opencode": "file:../toolu/tools/toolu-opencode",
       "@toolu/core": "file:../toolu/packages/toolu-core"
     }
   }
   ```

   Adjust relative paths to your layout. OpenCode runs `bun install` in `.opencode/` at startup.

3. **Plugin entry** — `.opencode/plugins/toolu.ts`:

   ```ts
   export { default } from "@toolu/opencode/plugin";
   ```

   The default export is the documented `PluginModule` ([#336](https://github.com/Falconiere/toolu/issues/336)). Its `server` runs preflight and bootstrap ([#211](https://github.com/Falconiere/toolu/issues/211)), then returns a `tool.execute.before` hook. When setup fails (no repo root, missing git or Bun, bootstrap NotReady), every tool call is refused with `toolu: not ready: <reason>`, and the same line reaches the host log (`opencode --print-logs`). A healthy start logs `toolu: ready (<n> bootstrap artifacts)`. If the npm package and this shim are both configured, only the first load enforces and the other logs `toolu: duplicate load skipped`. Bootstrap runs each enabled plugin's committed `hooks/dist/register.js` bundle, else its `hooks/dist/session-start.js` bundle, with Bun. For example, context7 publishes the executable Bun bundle at the stable `context7/search.sh` path under the bootstrap data root, and toolu's bundle writes the `toolu/.session-start-ready` readiness marker ([#263](https://github.com/Falconiere/toolu/issues/263)).

4. **Enabled plugins** — project file `.opencode/toolu/plugins.json`:

   ```json
   { "version": 1, "enabled": ["toolu"] }

   ```

   Add names (for example `ts-quality`, `ast-grep`, `delivery-flow`, `pr-babysit`, `epic-orchestrator`) only when those directories exist under `$TOOLU_REPO_ROOT/plugins/` (or the npm package catalog) and you accept their extra prerequisites. Manifest dependencies are closed automatically (`@toolu/opencode/select`).

5. **Generated surface** — skills, agents, and commands for OpenCode live under `tools/toolu-opencode/generated/` (catalog: `opencode.toolu.json`). Regenerate after changing upstream skills with `bun run generate:opencode-surface` in the toolu clone. Wire OpenCode to those paths using your OpenCode project config; the committed tree is the canonical mirror from [#206](https://github.com/Falconiere/toolu/issues/206).

6. **Gate config** — optional `toolu.config.json` at `<project>/.opencode/toolu.config.json` (same schema as other hosts; OpenCode uses `.opencode` instead of `.claude` / `.codex`). Gate modes and presets: [`docs/config.md`](config.md#gate-modes-gates) and [`docs/portable-core.md`](portable-core.md). Example for a smoke test:

   ```json
   { "version": 1, "gates": { "protectedFiles": { "mode": "block" } } }
   ```

State and registry artifacts write under `<project>/.opencode/toolu/state/` unless overridden by `TOOLU_CONFIG_DIR` or `TOOLU_OPENCODE_HOME` (see `@toolu/opencode/host` in [`portable-core.md`](portable-core.md)).

## Verify a real gate

Hermetic proof (matches CI):

```bash
cd /path/to/toolu
bun run test:conformance
```

Live entry smoke on the pinned host (npm route, local shim, both at once, and a failing config entry; the first run needs the network):

```bash
cd /path/to/toolu
bun run smoke:opencode-entry
```

Optional live CLI probe:

```bash
export TOOLU_LIVE_OPENCODE=1
# optional: export OPENCODE_BIN=/path/to/opencode
bun run test:conformance
```

Full matrix and limitations: [`docs/conformance-report.md`](conformance-report.md). Do not advertise gates that [#212](https://github.com/Falconiere/toolu/issues/212) has not exercised.

In OpenCode, a blocked edit to a protected file (for example `.env` with `protectedFiles` in `block` mode) should **deny** before bytes change — same class as the `protected-files` and `permission-evaluate` suites.

## Update

npm install — OpenCode's plugin CLI checks and moves the package:

```bash
opencode plugin check @toolu/opencode    # is a newer release published?
opencode plugin update @toolu/opencode   # move to it; omit the name to update every outdated plugin
```

Contributor clone:

```bash
cd /path/to/toolu
git fetch --tags
git checkout vX.Y.Z.new
bun install --frozen-lockfile
```

Restart OpenCode so `.opencode/` dependencies reload. Bump the tag you track; release-please keeps root `package.json`, every `plugin.json`, and the Bun workspace packages on one `vX.Y.Z`.

## Rollback

Check out the last known-good tag in the toolu clone and run `bun install --frozen-lockfile` again. Your project’s `.opencode/toolu.config.json` and `.opencode/toolu/plugins.json` are preserved unless you remove them.

## Disable / uninstall

- **Disable enforcement** — remove or rename `.opencode/plugins/toolu.ts`, or clear `enabled` in `.opencode/toolu/plugins.json`, then restart OpenCode. User config under `.opencode/toolu.config.json` is left intact.
- **Remove toolu** — npm install: `opencode plugin remove @toolu/opencode`, then delete `.opencode/toolu/` and optional `.opencode/toolu.config.json`. Contributor clone: delete `.opencode/plugins/toolu.ts`, `.opencode/package.json` (if only used for toolu), `.opencode/toolu/`, and optional `.opencode/toolu.config.json`, remove `TOOLU_REPO_ROOT` from your environment, and delete the clone separately.
- **Scoped cleanup** — registry/state under `.opencode/toolu/state/` can be deleted to force a fresh bootstrap; it does not remove Claude/Codex settings.

## Host comparison

| Host | Install doc | Runtime |
|------|-------------|---------|
| Claude Code | [README § Install](../README.md#install) | Bun hook bundles |
| Codex | [README § Install](../README.md#install) | Bun hook bundles |
| OpenCode | This file | In-process TypeScript dispatcher |

## Related

- [Portable core contracts](portable-core.md)
- [Configuration](config.md)
- [Conformance report](conformance-report.md)
