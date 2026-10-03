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
`<project>/.opencode/toolu/plugins.json`, or for every project with the global selection file (see step 4):

```json
{ "version": 1, "enabled": ["toolu"] }
```

Every plugin ships its skills, agents and commands in the package. OpenCode discovers the ones you enable automatically (step 5). For example, add `delivery-flow`, `brainstorm`, `pr-babysit` or `epic-orchestrator` to `enabled` when you want those workflows. Dependencies close automatically.

`npx @toolu/plugins install --host opencode` does not drive this yet — the CLI
has no OpenCode adapter. Until it does, run the two steps above.

The git-clone flow below remains the contributor path, and is still how you work
against an unreleased checkout.

A ready plugin appends startup instructions, prompt reminders, and compaction context onto the host's system, user, and compaction arrays. A vague-prompt block becomes another reminder and does not stop the turn.

Enforcement runs in `tool.execute.before`. Covered calls are `bash`, `read`, `grep`, `glob`, `edit`, `write`, `apply_patch`, `task`, and MCP tools named `<server>_<tool>` for servers listed in `opencode.json`. The nine native core gates run before selected `pre-tools.d` registry modules; files left by disabled plugins stay inert. MCP calls use the standalone `mcp__` blocker policy. Task calls run the native and registry gates plus the standalone agent-tier policy, which checks a supplied task model against the active plan step and records delegation telemetry. A task with no model inherits the step's tier. Any other tool is left to the host. A toolu refusal stops the call before it runs, and a toolu allow never overrides your own `permission` rules. OpenCode still applies its native `deny` and `ask` rules after toolu allows; a rejected prompt gives no approval to a retry. The host cannot open a native prompt for a toolu gate. A security guardrail's `ask` therefore blocks, while a judgement gate's `ask` allows the call and appends advice to that call's successful tool result through `tool.execute.after` ([#339](https://github.com/Falconiere/toolu/issues/339)). Host events without an OpenCode hook remain outside that scope; see the [host contract](opencode-host-contract.md).

After a completed tool call, `tool.execute.after` runs native gate-status and push-waiver checks, then selected `post-tools.d` modules. An edit, write or patch can record a per-file quality failure; a shell quality command with a confirmed nonzero exit records a failed global gate. The next commit or push is then checked against that state. Post-check diagnostics and matching pre-tool advice are appended to the original tool result once. A post-check message reports an action that already ran; it does not undo its side effects. A shell result without a confirmed exit, or marked interrupted, cannot clear a failure or promote a waiver. OpenCode does not call the after hook for a thrown tool error or a rejected permission prompt, so those errors reach the model through the host and leave prior gate state intact.

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

   The default export is the documented `PluginModule` ([#336](https://github.com/Falconiere/toolu/issues/336)). Its `server` runs preflight and bootstrap ([#211](https://github.com/Falconiere/toolu/issues/211)), then returns `tool.execute.before` and `tool.execute.after` hooks.

   - **Setup fails** (no repo root, missing git or Bun, bootstrap NotReady): every tool call is refused with `toolu: not ready: <reason>`, and the same line reaches the host log (`opencode --print-logs`).
   - **Healthy start:** logs `toolu: ready (<p> plugins, <n> startup artifacts)`, then one `toolu: startup notes:` line listing up to 20 notes, such as a kept user file or a removed contribution.
   - **Both routes configured** (the npm package and this shim): only the first load enforces, and the other logs `toolu: duplicate load skipped`.

   **Bootstrap** ([#342](https://github.com/Falconiere/toolu/issues/342)):

   - **Which entries run:** every enabled plugin runs every SessionStart entry its `hooks/hooks.json` declares for `startup`, with Bun. Each entry is the committed `hooks/dist/<entry>.js` bundle behind the generated launcher. For example, ts-quality runs both `register` and `check-toolu`.
   - **Order:** dependencies start first. A dependency cycle, a declared bundle that is missing, a hand-written command, or a hook that is not a command makes toolu not ready.
   - **Contributions:** each entry reports its registry modules and helpers to the bootstrap (`TOOLU_STARTUP_REPORT`). Each one is checked on disk: a module must be byte-equal to the plugin's bundle, and a helper must be a symlink to it. For example, context7 publishes its Bun bundle at the stable `context7/search.sh` path under the bootstrap data root.
   - **Readiness:** comes from this run only. A failed or partial registration, a missing helper source, non-JSON startup output, or an entry that exits non-zero or outlives its 120 s deadline names the plugin and entry in the reason. Files left on disk from an earlier session never make toolu ready, whoever wrote them. The whole startup has a 180 s budget.
   - **Disabled plugins:** when a plugin is no longer enabled, the next startup removes its `<name>@toolu__*` registry modules. It also removes the helper symlinks recorded in `.opencode/toolu/state/toolu/startup-ledger.json`, but only while each one is still that symlink and inside the data root. A file you put at a helper path is kept and reported, never deleted.
   - **Environment:** entries run with your own `HOME` and toolu's roots (see [Roots and helper environment](#roots-and-helper-environment)). Inherited Claude Code, Codex, Cursor and Hermes root variables (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA`, …) are removed first, so no other host's home is read or written ([#343](https://github.com/Falconiere/toolu/issues/343)). Bun runs every entry, at startup and for prompt and compaction context, with `--no-env-file`. A project `.env` therefore never adds variables, and a credential check sees only the environment OpenCode started with ([#350](https://github.com/Falconiere/toolu/issues/350)).
   - **Context:** each entry's SessionStart context (toolu's session protocol, Jev's mandate) is collected for delivery to the model.

4. **Enabled plugins** — project file `.opencode/toolu/plugins.json`:

   ```json
   { "version": 1, "enabled": ["toolu"] }
   ```

   Add names (for example `ts-quality`, `ast-grep`, `delivery-flow`, `pr-babysit`, `epic-orchestrator`) only when those directories exist under `$TOOLU_REPO_ROOT/plugins/` (or the npm package catalog) and you accept their extra prerequisites. Manifest dependencies are closed automatically (`@toolu/opencode/select`).

   **Global selection** ([#345](https://github.com/Falconiere/toolu/issues/345)):

   - The first source that exists decides:
     1. the project file;
     2. the global file `<global config root>/toolu/plugins.json`, which is `~/.config/opencode/toolu/plugins.json` by default (see [Roots](#roots-and-helper-environment)), with the same schema;
     3. otherwise, every bundled plugin.
   - A project file, even `"enabled": []`, replaces the global one. The two are never merged.
   - `skills.<plugin>: false` in the project `toolu.config.json` still turns a plugin off.
   - **An invalid selection file stops toolu.** That covers a file that is unreadable, not JSON, a version other than `1`, an unknown key, or a non-string name. Every tool call is then refused with `toolu: not ready: plugin selection: invalid <path>: …` until you fix it. toolu does not guess a selection you did not write.
   - A name that is not installed is skipped and reported in the `toolu: startup notes:` log line.

5. **Generated surface (automatic)** — skills, agents and commands for OpenCode live in the package's `generated/` directory (catalog `opencode.toolu.json`). In a clone that is `tools/toolu-opencode/generated/`, which `bun run generate:opencode-surface` regenerates. You wire nothing by hand.

   **How they get in.** When toolu is ready, its `config` hook ([#345](https://github.com/Falconiere/toolu/issues/345)) adds the enabled plugins' surfaces, dependencies included, to OpenCode's merged config:

   - each skill's directory goes into `skills.paths`, so the native `skill` tool lists it in the system prompt and loads it;
   - agents and commands become `agent.<id>` and `command.<id>` entries, which OpenCode lists like its own.

   **No files.** toolu writes nothing into `.opencode/`, `~/.config/opencode/` or any other discovery directory. Its contributions are recomputed at every start from the current package and selection.

   **Enable, disable, update and remove** take effect at the next start and touch only toolu's entries:

   - removing a plugin from the selection drops its surfaces;
   - updating the package replaces them;
   - removing the package leaves nothing behind.

   **Your definitions win:**

   - **Skills:** if a `SKILL.md` with the same `name` exists anywhere OpenCode looks, toolu does not add its own, and your copy is the only one OpenCode sees. The locations are `.opencode/skills/`, `.claude/skills/`, `.agents/skills/`, the global equivalents, `OPENCODE_CONFIG_DIR` and your own `skills.paths`. Without this, OpenCode would load two same-name skills in an unpredictable order. Remote `skills.urls` and paths that other plugins add after toolu cannot be checked.
   - **Agents and commands:** your `agent.<id>` or `command.<id>` keys, from `opencode.json` or a Markdown file, override toolu's key by key, the way your config overrides OpenCode's built-in agents. Setting only `agent.toolu-quick-task.model` keeps toolu's prompt.
   - **Complete Markdown agents:** a full agent file with a toolu ID still inherits any key it leaves out, such as `mode` or toolu's permission rules. Set `disable: true` to drop a toolu agent.

   **Shared procedures.** Some generated skills link files under `generated/resources/`, such as the commit workflow. Those files sit outside the skill's own directory, so OpenCode would ask for `external_directory` permission to read them.

   - When your `permission` config has no `external_directory` rule and no wildcard key, toolu adds `external_directory: { "<generated>/resources/*": "allow" }`. That is the same allowance OpenCode itself gives every skill directory.
   - When you do have such a rule, toolu adds nothing and your rule decides.
   - Like OpenCode's skill-directory allowance, this rule also lets edit tools touch that package directory without the directory prompt; your `edit` permission still applies.

   **Log.** Each ready start logs `toolu: surfaces (<project selection | global selection | all installed plugins>, <n> ms): skills …; agents …; commands …`. When any applies, it adds a `toolu: surface notes:` line naming each kept user skill, merged entry and permission decision.

   **Migrating from manual wiring.** If you followed the old instructions and pointed `skills.paths` at `generated/skills`, or copied generated files into `.opencode/`, remove them. A user copy always wins, so a stale copy would hide the current one. The surface notes list every skill kept from your files.

6. **Gate config** — optional `toolu.config.json` at `<project>/.opencode/toolu.config.json` and, for every project, in the global config root (`${XDG_CONFIG_HOME:-~/.config}/opencode/toolu.config.json`); the project file wins on conflict. Same schema as other hosts; OpenCode uses `.opencode` instead of `.claude` / `.codex`. Gate modes and presets: [`docs/config.md`](config.md#gate-modes-gates) and [`docs/portable-core.md`](portable-core.md). Example for a smoke test:

   ```json
   { "version": 1, "gates": { "protectedFiles": { "mode": "block" } } }
   ```

### Roots and helper environment

[#343](https://github.com/Falconiere/toolu/issues/343) fixes three roots. None of them is a Claude Code or Codex home, and toolu never changes `HOME`.

| Root | Where | Holds |
|---|---|---|
| Project | The OpenCode instance's worktree. A linked git worktree is its own project. A `TOOLU_PROJECT_DIR` in your environment does not replace it | `.opencode/toolu.config.json`, `.opencode/toolu/plugins.json`, gate state in `.opencode/tmp/` |
| Global config | `TOOLU_CONFIG_DIR`, else `TOOLU_OPENCODE_HOME`, else `$XDG_CONFIG_HOME/opencode`, else `~/.config/opencode` | The global `toolu.config.json`; toolu only reads it |
| Data | `<project>/.opencode/toolu/state/`. With `TOOLU_CONFIG_DIR` or `TOOLU_OPENCODE_HOME` set: `<override>/toolu/opencode/projects/<name>-<hash>/`, one directory per project | Registry modules, published helpers, startup ledger |

Projects and worktrees never share a data root, even under one override, so one project's startup cannot remove or relink another's modules and helpers. Before #343 an override was itself the shared data root. Those old files (`<override>/toolu/{pre,post}-tools.d/*@toolu__*`, `<override>/<plugin>/<helper>.sh`, `<override>/toolu/startup-ledger.json`) are no longer used. While that old ledger remains, each startup logs a `toolu: startup notes:` line naming it. Deleting the ledger, which only OpenCode writes, silences the note. Delete the old modules and helpers too when no Claude Code or Codex install shares that override.

Every bash call the agent makes gets these variables through the plugin's `shell.env` hook. They are added only when toolu is ready, and nothing secret is copied:

| Variable | Value |
|---|---|
| `TOOLU_CONFIG_DIR` | The project's data root, where helpers such as `context7/search.sh` are published (the generated skills name `"${TOOLU_CONFIG_DIR:-…}/<plugin>/<helper>"`) |
| `TOOLU_OPENCODE_DATA_ROOT` | The same path. An OpenCode started from this bash sees `TOOLU_CONFIG_DIR` equal to it and does not treat it as an override, so it keeps its own roots |
| `TOOLU_USER_CONFIG_DIR` | The global config root |
| `TOOLU_HOST_OVERRIDE`, `TOOLU_PROJECT_CONFIG_DIRNAME`, `TOOLU_SETTINGS_DIR` | `opencode`, `.opencode`, the toolu settings directory, so helpers resolve OpenCode state |
| `TOOLU_PLUGIN_ROOT_<PLUGIN>` | Each enabled plugin's directory, the name upper-cased with `-` as `_` (`TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR`). Generated surfaces use it wherever the source says `${CLAUDE_PLUGIN_ROOT}` |
| `TOOLU_PLUGIN_ROOT` | The toolu core plugin, for `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js"` and `verdict.js` |
| `TOOLU_OPENCODE_ROOT` | The `@toolu/opencode` package, which holds `generated/` |
| `TOOLU_BUN` | The Bun that startup resolved (`TOOLU_BUN`, `PATH`, then `~/.bun/bin/bun`) |
| `PATH` | Gains that Bun's directory at the end, only when `PATH` has no `bun`, so `#!/usr/bin/env bun` helpers run. This works when that executable is named `bun`, as the default `~/.bun/bin/bun` is |
| `TOOLU_PROJECT_DIR` | Set to empty when your environment exports one, so it cannot point every helper at a single project |

Because `TOOLU_PROJECT_DIR` stays empty, a helper run inside another repository uses that repository's state. Every process the agent starts from bash inherits these variables. A nested `opencode` keeps its own roots through `TOOLU_OPENCODE_DATA_ROOT`. Unset the `TOOLU_*` variables before starting `claude` or `codex` from an OpenCode session.

### ast-grep

With `ast-grep` selected, its two registry modules run on OpenCode's own tools ([#347](https://github.com/Falconiere/toolu/issues/347)):

- **search-nudge** judges `grep` (its `include` counts as the glob) and `bash`, with the same rules and text as other hosts. The host has no pre-tool advisory channel, so the nudge is appended to that call's result as `[toolu advisory]`. A call the host refuses never gets one.
- **byte-savings** appends one line per completed `read`, `grep`, `glob` and bash `ast-grep`/`sg` run to `$TOOLU_CONFIG_DIR/toolu/byte-savings/<session>.jsonl` (the project's data root). It measures the host's result text before toolu appends anything. An interrupted call, or a shell call without a confirmed exit, is not recorded. After each `ast-grep` run, the result ends with the session's report under `[toolu post-check after execution]`; `read`, `grep` and `glob` results get no extra text. OpenCode's `read` output includes line numbers and tags, so a read's `returned` can exceed the file's `full` size. The same report on demand, from the session's bash:

  ```bash
  bun "$TOOLU_PLUGIN_ROOT_AST_GREP/hooks/dist/byte-savings-report.js" "$TOOLU_CONFIG_DIR"/toolu/byte-savings/*.jsonl
  ```

The `ast-grep-ast-grep` skill names only the `ast-grep` CLI, so its examples run as written in bash.

## Verify a real gate

Hermetic proof (matches CI):

```bash
cd /path/to/toolu
bun run test:conformance
```

Live entry smoke on the pinned host: the npm route, a local shim, both at once, and a failing config entry. It also covers full startup and paths: helpers resolve from paths with spaces with no `bun` on `PATH`, and a linked worktree keeps its own data root. The first run needs the network. Pass scenario ids to run only some of them:

```bash
cd /path/to/toolu
bun run smoke:opencode-entry                       # every scenario
bun run smoke:opencode-entry entry.helper-env entry.worktree-state
```

The same runner proves surface install and discovery ([#345](https://github.com/Falconiere/toolu/issues/345)):

- `surfaces.npm-clean`: a clean npm install exposes the selection's skills, agents and commands. The native `skill` tool loads one, and its linked shared procedure is readable.
- `surfaces.lifecycle`: reselect, update and remove.
- `surfaces.precedence`: user skills, agents, commands and permission rules win.
- `surfaces.skill-roots`: a user skill in each OpenCode skill root leaves one copy.
- `surfaces.both-routes`: the npm route and a local shim together add each surface once.
- `surfaces.selection`: global, then project selection; an invalid file fails closed.

```bash
bun run smoke:opencode-entry surfaces.npm-clean surfaces.lifecycle surfaces.precedence surfaces.skill-roots surfaces.both-routes surfaces.selection
```

Live pre-tool smoke on the pinned host checks protected edits, writes and patches, unsafe shell, commit and push gates, MCP and task denials, plus an allowed shell call:

```bash
bun run smoke:opencode-pretool
```

Live permission smoke checks native deny, repeated native ask rejection, guardrail and judgement gate fallback, model-visible registry advice, and denial by a second plugin in both load orders:

```bash
bun run smoke:opencode-permissions
```

The pinned post-tool smoke uses isolated git projects and a scripted loopback provider to check edited file state, model-visible diagnostics and later commit/push denial:

```bash
bun run smoke:opencode-posttool
```

The ast-grep smoke enables `toolu` and `ast-grep` in one isolated project. It loads the `ast-grep-ast-grep` skill with the native `skill` tool and runs its search example from bash. It checks each nudge, or its absence, in the model's tool messages, the savings report after the ast-grep run, the session ledger, and the report CLI. It needs `ast-grep` on `PATH`:

```bash
bun run smoke:opencode-ast-grep
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

- **Disable enforcement** — remove or rename `.opencode/plugins/toolu.ts`, then restart OpenCode. User config under `.opencode/toolu.config.json` is left intact. Clearing `enabled` in `.opencode/toolu/plugins.json` only removes the plugins' startup contributions and their skills, agents and commands at the next start; the core gates still run.
- **Remove toolu** — npm install: `opencode plugin remove @toolu/opencode`, then delete `.opencode/toolu/` and optional `.opencode/toolu.config.json`. Contributor clone: delete `.opencode/plugins/toolu.ts`, `.opencode/package.json` (if only used for toolu), `.opencode/toolu/`, and optional `.opencode/toolu.config.json`, remove `TOOLU_REPO_ROOT` from your environment, and delete the clone separately.
- **Scoped cleanup** — registry, helpers, startup ledger and state under the project's data root (`.opencode/toolu/state/`, or `<override>/toolu/opencode/projects/<name>-<hash>/`) can be deleted to force a fresh bootstrap; it does not remove Claude/Codex settings.

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
