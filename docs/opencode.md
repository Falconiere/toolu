# OpenCode install

**Issue:** [#207](https://github.com/Falconiere/toolu/issues/207) (epic [#203](https://github.com/Falconiere/toolu/issues/203))  
**Status:** `@toolu/opencode` publishes to npm from v6.7.0. Install it with OpenCode's own plugin CLI — no clone, no `TOOLU_REPO_ROOT`:

```bash
opencode plugin add @toolu/opencode
```

The package carries the bash `plugins/` tree, so the bridge resolves its plugin
root to its own package directory. Choose which bash plugins are active with
`<project>/.opencode/toolu/plugins.json`:

```json
{ "version": 1, "enabled": ["toolu"] }
```

`npx toolu plugins install --host opencode` does not drive this yet — the CLI
has no OpenCode adapter. Until it does, run the two steps above.

The git-clone flow below remains the contributor path, and is still how you work
against an unreleased checkout.

**Previous status:** Git-clone install only (no npm publish). Enforcement scope matches [#212](https://github.com/Falconiere/toolu/issues/212) fixture evidence — not every bash gate is wired yet.

Claude Code and Codex keep their existing marketplace installs and **bash-only** runtime; they do **not** require Bun. OpenCode uses the Bun/TS packages in this repo plus the same bash hook tree under `plugins/`.

## Prerequisites

| Requirement | Pin / note |
|-------------|------------|
| OpenCode CLI | `v2.0.12` — `$OPENCODE_BIN` or `command -v opencode`; `opencode --version` |
| Plugin SDK | `@opencode/plugin@2.0.12` (resolved via the clone’s `bun.lock`) |
| Bun | `1.4.x` (workspace `>=1.4.0 <1.5.0`; CI/docs baseline `1.4.2`) |
| Bash | ≥ 5 (bridge and assembled hooks) |
| jq | Required for config merge and hook scripts |
| git | Bootstrap and bridge context |
| Platform | macOS and Linux ([#212](./conformance-report.md)); Windows **N/A** |

Do **not** target the legacy `opencode-ai@1.18.31` (V1) line. Pins and contracts: [`docs/portable-core.md`](portable-core.md).

Per-gate tool dependencies (Rust, TypeScript, Python linters, etc.) are unchanged from the bash plugins you enable — see each plugin’s README and [`docs/config.md`](config.md).

## First install (git clone)

A pasteable agent prompt lives in the root [README § Install everything → OpenCode](../README.md#install-everything) (`<!-- install-everything:opencode -->`) and is mirrored in [`docs/plugins/index.md`](plugins/index.md). The steps below are the canonical detail that prompt summarizes.

Install from a **release tag**, not `main`, unless you are developing toolu itself.

```bash
git clone https://github.com/Falconiere/toolu.git
cd toolu
git checkout vX.Y.Z   # latest from https://github.com/Falconiere/toolu/releases

bun install --frozen-lockfile
bun run check:opencode-surface   # optional sanity: generated mirror matches sources
```

Record the clone path — the OpenCode plugin must reach the full repo (bash `plugins/`, `plugins/toolu/settings/`, and `tools/toolu-opencode/generated/`).

### Project wiring

In **your application repo** (not inside the toolu clone):

1. **Environment** — point toolu at the clone (required for gate enforcement):

   ```bash
   export TOOLU_REPO_ROOT=/absolute/path/to/toolu
   ```

   `TOOLU_ROOT` is an alias. You can set the same variable in your shell profile or OpenCode’s environment so every session sees it.

2. **OpenCode plugin loader** — local plugin under `.opencode/plugins/` (see [OpenCode plugins](https://opencode.ai/v2/docs/build/plugins)). Example `.opencode/package.json` using **file** dependencies into the clone (no global TS install):

   ```json
   {
     "dependencies": {
       "@toolu/opencode": "file:../toolu/tools/toolu-opencode",
       "@toolu/core": "file:../toolu/packages/toolu-core",
       "@opencode/plugin": "2.0.12"
     }
   }
   ```

   Adjust relative paths to your layout. OpenCode runs `bun install` in `.opencode/` at startup.

3. **Plugin entry** — `.opencode/plugins/toolu.ts`:

   ```ts
   export { default } from "@toolu/opencode/plugin";
   ```

   The default export registers `permission.evaluate` and runs preflight + bootstrap ([#204](https://github.com/Falconiere/toolu/issues/204), [#211](https://github.com/Falconiere/toolu/issues/211)).

4. **Enabled bash plugins** — project file `.opencode/toolu/plugins.json`:

   ```json
   { "version": 1, "enabled": ["toolu"] }
   ```

   Add names (for example `ts-quality`) only when those directories exist under `$TOOLU_REPO_ROOT/plugins/` and you accept their extra prerequisites. Manifest dependencies are closed automatically (`@toolu/opencode/select`).

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

Optional live CLI probe:

```bash
export TOOLU_LIVE_OPENCODE=1
# optional: export OPENCODE_BIN=/path/to/opencode
bun run test:conformance
```

Full matrix and limitations: [`docs/conformance-report.md`](conformance-report.md). Do not advertise gates that [#212](https://github.com/Falconiere/toolu/issues/212) has not exercised.

In OpenCode, a blocked edit to a protected file (for example `.env` with `protectedFiles` in `block` mode) should **deny** before bytes change — same class as the `protected-files` and `permission-evaluate` suites.

## Update

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
- **Remove toolu** — delete `.opencode/plugins/toolu.ts`, `.opencode/package.json` (if only used for toolu), `.opencode/toolu/`, and optional `.opencode/toolu.config.json`. Remove `TOOLU_REPO_ROOT` from your environment. The git clone can be deleted separately.
- **Scoped cleanup** — registry/state under `.opencode/toolu/state/` can be deleted to force a fresh bootstrap; it does not remove Claude/Codex settings.

## Host comparison

| Host | Install doc | Runtime |
|------|-------------|---------|
| Claude Code | [README § Install](../README.md#install) | Bash hooks only |
| Codex | [README § Install](../README.md#install) | Bash hooks only |
| OpenCode | This file | Bun plugin + bash bridge |

## Related

- [Portable core contracts](portable-core.md)
- [Configuration](config.md)
- [Conformance report](conformance-report.md)
