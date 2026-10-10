# OpenCode install

**Supported host:** `opencode-ai@1.18.34` with `@opencode-ai/plugin@1.18.34`. That is the plugin API documented at <https://opencode.ai/docs/plugins/>, pinned and probed in the [host contract](opencode-host-contract.md). Runs on macOS and Linux; Windows is **N/A**.
**Evidence:** the required OpenCode acceptance (`bun run test:opencode`) runs on Linux and macOS in CI and checks every support claim on this page. The [quick start](#quick-start) and the [update and removal steps](#update-roll-back-and-remove) run there exactly as written, and [Plugin support](#plugin-support) lists the checks each plugin must pass.

toolu ships to OpenCode as one npm package, `@toolu/opencode`, which carries all 12 plugins. OpenCode has no `plugin add`, `list`, `update` or `remove` command of its own. The `toolu` CLI therefore adds the package to OpenCode's documented `plugin` config and writes which plugins are enabled ([docs/cli/installer.md § OpenCode](cli/installer.md#opencode)). It keeps your comments and other entries.

If you installed the V2-targeted adapter (toolu 7.7.2 or earlier on `opencode` 2.x), follow the [migration guide](opencode-migration.md) instead.

## Prerequisites

| Requirement  | Pin / note                                                                                                     |
| ------------ | -------------------------------------------------------------------------------------------------------------- |
| OpenCode CLI | `opencode-ai@1.18.34` ([host contract](opencode-host-contract.md)); `opencode --version` prints `1.18.34`      |
| Plugin SDK   | `@opencode-ai/plugin@1.18.34`. The host provisions it into each config directory; toolu imports its types only |
| Bun          | `1.4.x` (workspace `>=1.4.0 <1.5.0`; CI/docs baseline `1.4.2`); see the [runtime contract](runtime.md)          |
| Node.js      | For `npx @toolu/plugins`                                                                                        |
| git          | Project and gate context                                                                                       |
| Platform     | macOS and Linux; Windows **N/A**                                                                               |

Install the host with `npm install -g opencode-ai@1.18.34`, or any route that makes `opencode --version` print `1.18.34`. That is the only verified version. The first start in a fresh profile needs registry access, because the host installs its SDK. Plugins can have prerequisites of their own; see [Plugin guides](#plugin-guides) and [`docs/config.md`](config.md).

## Quick start

Run this in the project you want guarded. It installs toolu in your global OpenCode config with the core `toolu` plugin and one leaf plugin, `ast-grep`. It then checks that OpenCode discovers the leaf plugin's skill and asks your agent to overwrite a scratch secrets file, `.env.toolu-check`. The block does not touch your `.env` or your gate config, and it removes the scratch file at the end, even when a step fails:

<!-- opencode-doc:quickstart:start -->

```bash
test "$(opencode --version)" = 1.18.34
npx @toolu/plugins install toolu ast-grep --host opencode
opencode debug skill | grep ast-grep-ast-grep
test ! -e .env.toolu-check
trap 'rm -f .env.toolu-check' EXIT
printf 'SECRET=1\n' > .env.toolu-check
opencode run "Replace the contents of .env.toolu-check with PWNED"
grep -qx 'SECRET=1' .env.toolu-check && echo "toolu refused the write: .env.toolu-check is unchanged"
rm -f .env.toolu-check
```

<!-- opencode-doc:quickstart:end -->

- **What `install` writes.** It adds `@toolu/opencode@<CLI version>` to the `plugin` array of `~/.config/opencode/opencode.json` (or `$XDG_CONFIG_HOME/opencode`). It writes `{ "version": 1, "enabled": ["toolu", "ast-grep"] }` to `toolu/plugins.json` under the global config root. That root is the same directory unless `TOOLU_CONFIG_DIR` or `TOOLU_OPENCODE_HOME` overrides it (see [Roots](#roots-and-helper-environment)). Add `--scope project` to write this project's `opencode.json` and `.opencode/toolu/plugins.json` instead.
- **What the session shows.**
  - `opencode debug skill` lists the skills OpenCode discovered, and `opencode run` uses your configured model.
  - Paths matching `.env` and `.env.*` are protected. With the default gate config, `protectedFiles` asks, and on OpenCode a security guardrail's ask refuses the call.
  - toolu refuses the write in `tool.execute.before`, before the file changes. The model receives `… a protected path (matches ".env.*") …` as the tool error.
  - If your `toolu.config.json` sets `protectedFiles` to `advise` or `off`, the write goes through and the last check prints nothing.
- **How CI runs it.** The acceptance runs this block word for word (`docs.quickstart`) in an isolated profile. It redirects only `npx @toolu/plugins` to the checkout's CLI and `opencode` to the pinned binary, and it replaces your model with a scripted one that attempts the write.

## Plugin support

Each plugin's status comes from the [capability matrix](opencode-host-contract.md#capability-matrix). Its CI checks are the actual-host acceptance checks dedicated to it: `bun run test:opencode` fails unless every one of them passes on Linux and macOS. The limitations below are host-specific: each one names what OpenCode lacks and the alternative toolu uses. `bun run check:opencode-docs` regenerates this section from the matrix and the acceptance registry, and fails when it drifts.

<!-- opencode-support:start -->
| Plugin | Status on OpenCode | Dedicated CI checks |
|---|---|---|
| ast-grep | Supported with limitations | `entry.worktree-state`, `docs.quickstart`, `ast-grep.session` |
| brainstorm | Supported | `live.delivery-workflows` |
| delivery-flow | Supported | `live.delivery-workflows` |
| epic-orchestrator | Supported | `live.epic-worker` |
| jev | Supported | `entry.helper-env`, `live.jev` |
| pr-babysit | Supported with limitations | `babysit.*` (3) |
| python-quality | Supported | `pyquality.*` (3) |
| rust-quality | Supported | `rsquality.*` (3) |
| statusline | Supported with limitations | `status.*` (2) |
| toolu | Supported with limitations | `entry.*` (6), `surfaces.*` (6), `cli.*` (2), `docs.*` (3), `pretool.*` (7), `permissions.*` (7), `posttool.*` (3), `live.*` (2), `concurrent.sessions`, `budget.overhead` |
| toolu-review | Supported | `live.core-workflows` |
| ts-quality | Supported | `tsquality.*` (3) |

**Host-specific limitations**

- **ast-grep** — tools (unsupported): search-nudge advises ast-grep before Grep or bash text search, without blocking. Alternative: Deliver the nudge with the tool result through tool.execute.after.
- **pr-babysit** — Tick scheduling: the documented plugin API has no cron or loop primitive, so the OpenCode controller runs bounded ticks in the invoking turn (bash sleep of backoff.waitSeconds, at most 60 s) and resumes from the slot state file when invoked again.
- **statusline** — A persistent statusline is Claude Code-only (its statusLine setting). The pinned server plugin API has no status bar, and @opencode-ai/plugin/tui slots (home_footer, sidebar_footer) load only in the interactive TUI, which headless probes cannot verify. Alternative: the statusline-status skill reports toolu readiness from <data root>/toolu/opencode-status.json, and each startup sends one structured toolu: status client.app.log entry.
- **toolu** — permission (partial): Gate ask decisions open a native prompt, and the user's permission rules stay authoritative. Alternative: Degrade ask with the @toolu/core/host class rules: security guardrails ask becomes a tool.execute.before deny, judgement gates ask becomes advice appended by tool.execute.after; the user's own ask/deny rules still apply after toolu allows.
- **toolu** — postTool (partial): gate-status, push-waiver and post-tools.d quality feedback after edits and bash. Alternative: A thrown tool error has no side effect to check; it reaches the model as the tool error and the event bus as message.part.updated.
<!-- opencode-support:end -->

## Choose plugins

`npx @toolu/plugins install <name...> --host opencode` enables the named plugins plus their manifest dependencies. `npx @toolu/plugins remove <name...> --host opencode --yes` disables them, and is refused while an enabled plugin depends on one of them. You can also write the selection file yourself:

```json
{ "version": 1, "enabled": ["toolu"] }
```

Selection files ([#345](https://github.com/Falconiere/toolu/issues/345)):

- The first source that exists decides:
  1. the project file `.opencode/toolu/plugins.json`;
  2. the global file `<global config root>/toolu/plugins.json`, which is `~/.config/opencode/toolu/plugins.json` by default (see [Roots](#roots-and-helper-environment)), with the same schema;
  3. otherwise, every bundled plugin.
- A project file, even `"enabled": []`, replaces the global one. The two are never merged.
- Manifest dependencies are closed automatically (`@toolu/opencode/select`).
- `skills.<plugin>: false` in the project `toolu.config.json` still turns a plugin off.
- **An invalid selection file stops toolu.** That covers a file that is unreadable, not JSON, a version other than `1`, an unknown key, or a non-string name. Every tool call is then refused with `toolu: not ready: plugin selection: invalid <path>: …` until you fix it. toolu does not guess a selection you did not write.
- A name that is not installed is skipped and reported in the `toolu: startup notes:` log line.

Changes take effect at the next OpenCode start.

## Update, roll back and remove

<!-- opencode-doc:manage:start -->

```bash
npx @toolu/plugins update --host opencode
npx @toolu/plugins remove ast-grep --host opencode --yes
npx @toolu/plugins remove toolu --host opencode --yes
```

<!-- opencode-doc:manage:end -->

- **Update.** `update` rewrites every `@toolu/opencode` entry in scope to the CLI's own release, and a `[spec, options]` tuple keeps its options. Run it with the newest CLI, then restart OpenCode. When the package is configured in both your global and project config, add `--scope user` or `--scope project`.
- **Roll back.** `TOOLU_OPENCODE_PACKAGE=@toolu/opencode@<X.Y.Z> npx @toolu/plugins update --host opencode` pins an earlier release. `TOOLU_OPENCODE_PACKAGE` overrides the spec the CLI writes, and `cli.lifecycle` checks this same `update` path by moving the entry to a second packed release. Selection and gate config are untouched. For the V2-targeted adapter, see the [migration guide's rollback](opencode-migration.md#roll-back).
- **Disable one plugin.** `remove <name>` takes it out of the selection. Its skills, agents, commands, startup context and toolu-owned helper symlinks disappear at the next start.
- **Remove toolu.** `remove toolu` deletes the package entry and keeps the selection file, so a reinstall restores it. Delete `toolu/plugins.json` and `.opencode/toolu.config.json` yourself if you want them gone.
- **Clean state.** The project's data root (`.opencode/toolu/state/`, or `<override>/toolu/opencode/projects/<name>-<hash>/`) holds registry modules, helpers and the startup ledger. Deleting it forces a fresh bootstrap and touches no Claude Code or Codex settings. Gate state lives in `.opencode/tmp/`.

CI runs this block word for word after the quick start (`docs.quickstart`). It checks that the entry stays at the tested release, that `ast-grep-ast-grep` disappears, and that removing toolu leaves no toolu entry or skill.

## How toolu runs inside OpenCode

The package's default entry (`exports["."]` / `main` → `./src/plugin/toolu.ts`) is a `PluginModule` `{ id: "toolu", server }`, so the host loads it from the `plugin` config entry with no local shim. The package carries the plugin manifests, settings and committed Bun bundles, so the adapter resolves its plugin root to its own package directory: no clone, no `TOOLU_REPO_ROOT`. Every helper, link, import and export it reaches ships in the same tarball or comes from a declared registry dependency (`bun run test:pack`). `@toolu/core` is declared as `^<this release>`, and `@opencode-ai/plugin` and `@opencode-ai/sdk` are peer dependencies at the pinned SDK version.

- **Setup fails** (missing git or Bun, an invalid selection, bootstrap NotReady): every tool call is refused with `toolu: not ready: <reason>`, and the same line reaches the host log (`opencode --print-logs`). toolu never throws from init, because the host would then run tools unguarded.
- **Healthy start:** the log shows `toolu: ready (<p> plugins, <n> startup artifacts)`, then one `toolu: startup notes:` line listing up to 20 notes, such as a kept user file or a removed contribution.
- **Both routes configured** (the npm entry and a [contributor shim](#contributor-path-git-clone)): only the first load enforces, and the other logs `toolu: duplicate load skipped`.

**Context.** A ready plugin appends startup instructions, prompt reminders and compaction context to the host's system, user and compaction arrays. A vague-prompt block becomes another reminder and does not stop the turn.

**Enforcement before a tool runs.**

- **Hook and covered calls.** Enforcement runs in `tool.execute.before`. It covers `bash`, `read`, `grep`, `glob`, `edit`, `write`, `apply_patch`, `task`, and MCP tools named `<server>_<tool>` for servers listed in `opencode.json`. Any other tool is left to the host.
- **Order.** The nine native core gates run before the selected `pre-tools.d` registry modules. Files left by disabled plugins stay inert.
- **MCP calls** use the standalone `mcp__` blocker policy.
- **Task calls** run the native and registry gates plus the standalone agent-tier policy. That policy checks a supplied task model against the active plan step and records delegation telemetry; a task with no model inherits the step's tier.
- **Refusals and permissions.** A toolu refusal stops the call before it runs. A toolu allow never overrides your own `permission` rules: OpenCode still applies its native `deny` and `ask` rules afterwards, and a rejected prompt gives a retry no approval.
- **`ask` decisions.** The host cannot open a native prompt for a toolu gate. A security guardrail's `ask` therefore blocks. A judgement gate's `ask` allows the call and appends advice to that call's successful result through `tool.execute.after` ([#339](https://github.com/Falconiere/toolu/issues/339)).

**Checks after a tool runs.**

- **What runs.** After a completed tool call, `tool.execute.after` runs the native gate-status and push-waiver checks, then the selected `post-tools.d` modules.
- **Gate state.**
  - An edit, write or patch can record a per-file quality failure.
  - A shell quality command with a confirmed nonzero exit records a failed global gate.
  - The next commit or push is checked against that state.
- **Delivery to the model.** Post-check diagnostics and matching pre-tool advice are appended to the original tool result once. A post-check message reports an action that already ran; it does not undo its side effects.
- **Unconfirmed results.** A shell result without a confirmed exit, or one marked interrupted, cannot clear a failure or promote a waiver.
- **No after hook.** OpenCode does not call it for a thrown tool error or a rejected permission prompt. Those errors reach the model through the host, and prior gate state stays intact.

Host events without an OpenCode hook remain outside this scope; see the [host contract](opencode-host-contract.md).

**Bootstrap** ([#342](https://github.com/Falconiere/toolu/issues/342)):

- **Which entries run:** every enabled plugin runs every SessionStart entry its `hooks/hooks.json` declares for `startup`. OpenCode accepts the generated Bun and native launcher forms. A native entry runs the declared POSIX command, which selects a compatible `toolu` binary or uses its committed Bun bundle during the transition. The bundle remains the entry's identity. For example, ts-quality runs both `register` and `check-toolu`.
- **Order:** dependencies start first. A dependency cycle, a declared bundle that is missing, a hand-written command, or a hook that is not a command makes toolu not ready.
- **Contributions:**
  - Each entry reports its registry modules and helpers to the bootstrap (`TOOLU_STARTUP_REPORT`).
  - Each one is checked on disk: a module must be byte-equal to the plugin's bundle, and a helper must be a symlink to it.
  - For example, jev publishes its Bun bundle at the stable `jev/jev.sh` path under the bootstrap data root.
- **Readiness:** comes from this run only.
  - Any of these makes toolu not ready, with the plugin and entry named in the reason: a failed or partial registration, a missing helper source, non-JSON startup output, or an entry that exits non-zero or outlives its 120 s deadline.
  - Files left on disk from an earlier session never make toolu ready, whoever wrote them.
  - The whole startup has a 180 s budget.
- **Disabled plugins:**
  - When a plugin is no longer enabled, the next startup removes its `<name>@toolu__*` registry modules.
  - It also removes the helper symlinks recorded in `.opencode/toolu/state/toolu/startup-ledger.json`, but only while each one is still that symlink and inside the data root.
  - A file you put at a helper path is kept and reported, never deleted.
- **Environment:**
  - Entries run with your own `HOME` and toolu's roots (see [Roots and helper environment](#roots-and-helper-environment)).
  - Inherited Claude Code, Codex, Cursor and Hermes root variables (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `PLUGIN_ROOT`, `CLAUDE_PLUGIN_DATA`, …) are removed first, so no other host's home is read or written ([#343](https://github.com/Falconiere/toolu/issues/343)).
  - OpenCode passes the same environment and deadline to Bun and native entries. Direct Bun entries and the native launcher's transitional Bun fallback run with `--no-env-file`, at startup and for prompt and compaction context. A project `.env` therefore never adds variables to those entries, and a credential check sees only the environment OpenCode started with ([#350](https://github.com/Falconiere/toolu/issues/350)).
- **Context:**
  - Each entry's SessionStart context is collected for delivery to the model: toolu's session protocol and Jev's mandate. If `TOOLU_BIN` points to a missing executable, a native launcher sends both install commands as session context and OpenCode still starts. With no override, the transitional Bun fallback remains available. Bun entries keep their existing startup notices in the host log.

### Skills, agents and commands

The skills, agents and commands for OpenCode live in the package's `generated/` directory (catalog `opencode.toolu.json`). In a clone that is `tools/toolu-opencode/generated/`, which `bun run generate:opencode-surface` regenerates. You wire nothing by hand.

**How they get in.** When toolu is ready, its `config` hook ([#345](https://github.com/Falconiere/toolu/issues/345)) adds the enabled plugins' surfaces, dependencies included, to OpenCode's merged config:

- each skill's directory goes into `skills.paths`, so the native `skill` tool lists it in the system prompt and loads it;
- agents and commands become `agent.<id>` and `command.<id>` entries, which OpenCode lists like its own.

**No files.** toolu writes nothing into `.opencode/`, `~/.config/opencode/` or any other discovery directory. Its contributions are recomputed at every start from the current package and selection.

**Enable, disable, update and remove** take effect at the next start and touch only toolu's entries:

- removing a plugin from the selection drops its surfaces;
- updating the package replaces them;
- removing the package leaves nothing behind.

**Your definitions win:**

- **Skills.** If a `SKILL.md` with the same `name` exists anywhere OpenCode looks, toolu does not add its own, and your copy is the only one OpenCode sees.
  - OpenCode looks in `.opencode/skills/`, `.claude/skills/`, `.agents/skills/`, their global equivalents, `OPENCODE_CONFIG_DIR` and your own `skills.paths`.
  - Without this rule, OpenCode would load two same-name skills in an unpredictable order.
  - Remote `skills.urls`, and paths that other plugins add after toolu, cannot be checked.
- **Agents and commands:** your `agent.<id>` or `command.<id>` keys, from `opencode.json` or a Markdown file, override toolu's key by key, the way your config overrides OpenCode's built-in agents. Setting only `agent.toolu-quick-task.model` keeps toolu's prompt.
- **Complete Markdown agents:** a full agent file with a toolu ID still inherits any key it leaves out, such as `mode` or toolu's permission rules. Set `disable: true` to drop a toolu agent.

**Shared procedures.** Some generated skills link files under `generated/resources/`, such as the commit workflow. Those files sit outside the skill's own directory, so OpenCode would ask for `external_directory` permission to read them.

- When your `permission` config has no `external_directory` rule and no wildcard key, toolu adds `external_directory: { "<generated>/resources/*": "allow" }`. That is the same allowance OpenCode itself gives every skill directory.
- When you do have such a rule, toolu adds nothing and your rule decides.
- Like OpenCode's skill-directory allowance, this rule also lets edit tools touch that package directory without the directory prompt; your `edit` permission still applies.

**Log.** Each ready start logs `toolu: surfaces (<project selection | global selection | all installed plugins>, <n> ms): skills …; agents …; commands …`. When any applies, it adds a `toolu: surface notes:` line naming each kept user skill, merged entry and permission decision.

### Gate config

`toolu.config.json` is optional. Put it at `<project>/.opencode/toolu.config.json` and, for every project, in the global config root (`${XDG_CONFIG_HOME:-~/.config}/opencode/toolu.config.json`). The project file wins on conflict. The schema is the same as on other hosts; OpenCode uses `.opencode` instead of `.claude` / `.codex`. Gate modes and presets: [`docs/config.md`](config.md#gate-modes-gates) and [`docs/portable-core.md`](portable-core.md).

## Roots and helper environment

[#343](https://github.com/Falconiere/toolu/issues/343) fixes three roots. None of them is a Claude Code or Codex home, and toolu never changes `HOME`.

| Root          | Where                                                                                                                                                                    | Holds                                                                                         |
| ------------- | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------ | --------------------------------------------------------------------------------------------- |
| Project       | The OpenCode instance's worktree. A linked git worktree is its own project. A `TOOLU_PROJECT_DIR` in your environment does not replace it                                | `.opencode/toolu.config.json`, `.opencode/toolu/plugins.json`, gate state in `.opencode/tmp/` |
| Global config | `TOOLU_CONFIG_DIR`, else `TOOLU_OPENCODE_HOME`, else `$XDG_CONFIG_HOME/opencode`, else `~/.config/opencode`                                                              | The global `toolu.config.json`; toolu only reads it                                           |
| Data          | `<project>/.opencode/toolu/state/`. With `TOOLU_CONFIG_DIR` or `TOOLU_OPENCODE_HOME` set: `<override>/toolu/opencode/projects/<name>-<hash>/`, one directory per project | Registry modules, published helpers, startup ledger                                           |

Projects and worktrees never share a data root, even under one override, so one project's startup cannot remove or relink another's modules and helpers. Before #343 an override was itself the shared data root. Those old files (`<override>/toolu/{pre,post}-tools.d/*@toolu__*`, `<override>/<plugin>/<helper>.sh`, `<override>/toolu/startup-ledger.json`) are no longer used. While that old ledger remains, each startup logs a `toolu: startup notes:` line naming it. Deleting the ledger, which only OpenCode writes, silences the note. Delete the old modules and helpers too when no Claude Code or Codex install shares that override.

Every bash call the agent makes gets these variables through the plugin's `shell.env` hook. They are added only when toolu is ready, and nothing secret is copied:

| Variable                                                                    | Value                                                                                                                                                                                         |
| --------------------------------------------------------------------------- | --------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| `TOOLU_CONFIG_DIR`                                                          | The project's data root, where helpers such as `jev/jev.sh` are published (the generated skills name `"${TOOLU_CONFIG_DIR:-…}/<plugin>/<helper>"`)                                    |
| `TOOLU_OPENCODE_DATA_ROOT`                                                  | The same path. An OpenCode started from this bash sees `TOOLU_CONFIG_DIR` equal to it and does not treat it as an override, so it keeps its own roots                                         |
| `TOOLU_USER_CONFIG_DIR`                                                     | The global config root                                                                                                                                                                        |
| `TOOLU_HOST_OVERRIDE`, `TOOLU_PROJECT_CONFIG_DIRNAME`, `TOOLU_SETTINGS_DIR` | `opencode`, `.opencode`, the toolu settings directory, so helpers resolve OpenCode state                                                                                                      |
| `TOOLU_PLUGIN_ROOT_<PLUGIN>`                                                | Each enabled plugin's directory, the name upper-cased with `-` as `_` (`TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR`). Generated surfaces use it wherever the source says `${CLAUDE_PLUGIN_ROOT}`     |
| `TOOLU_PLUGIN_ROOT`                                                         | The toolu core plugin, for generated skills that run its bundles, such as `toolu-debug`                                                                                                       |
| `TOOLU_OPENCODE_ROOT`                                                       | The `@toolu/opencode` package, which holds `generated/`                                                                                                                                       |
| `TOOLU_BUN`                                                                 | The Bun that startup resolved (`TOOLU_BUN`, `PATH`, then `~/.bun/bin/bun`)                                                                                                                    |
| `PATH`                                                                      | Gains that Bun's directory at the end, only when `PATH` has no `bun`, so `#!/usr/bin/env bun` helpers run. This works when that executable is named `bun`, as the default `~/.bun/bin/bun` is |
| `TOOLU_PROJECT_DIR`                                                         | Set to empty when your environment exports one, so it cannot point every helper at a single project                                                                                           |

Because `TOOLU_PROJECT_DIR` stays empty, a helper run inside another repository uses that repository's state. Every process the agent starts from bash inherits these variables. A nested `opencode` keeps its own roots through `TOOLU_OPENCODE_DATA_ROOT`. Unset the `TOOLU_*` variables before starting `claude` or `codex` from an OpenCode session.

## Plugin guides

One section per plugin with host-specific setup. Enable a plugin with `npx @toolu/plugins install <name> --host opencode`, or add it to `enabled` in the selection file, then restart OpenCode.

### TypeScript post-edit quality

Add `ts-quality` to `.opencode/toolu/plugins.json` alongside `toolu`, then restart OpenCode. It requires a git-tracked `tsconfig*.json`, a lock file for an available Bun, pnpm, yarn, or npm executable, and runs per-file rules on completed `.ts` and `.tsx` writes, edits, and patches. If `ast-grep` is installed, structural checks use the TSX parser for `.tsx`; without it, those structural rules are skipped. A patch checks every changed TypeScript destination and clears gate entries for deleted files and moved sources. Disabled ts-quality and other file types do not invoke the module. The existing linked-worktree rule skips these checks in linked worktrees.

A violation appears in the completed tool result and in the project's `.opencode/tmp/quality-gate-status.json`; it blocks later commit and push attempts until a clean edit or removal clears it. The edit itself has already happened. Run `bun run smoke:opencode-ts-quality` in the toolu checkout to replay native write/edit recovery, multi-file patch, and disabled-plugin scenarios on the pinned host.

### Python post-edit quality

Add `python-quality` to `.opencode/toolu/plugins.json` alongside `toolu`, then restart OpenCode. It requires a Python marker (`pyproject.toml`, `setup.py`, `setup.cfg` or `requirements.txt`) at the git toplevel and `python3` on `PATH`, and runs static per-file rules on completed `.py` writes, edits, and patches; it never invokes `ruff`, `pylint`, or `mypy`. If `ast-grep` is installed, the no-mocks rule scans test files with its Python parser; without it, that rule is skipped. A patch checks every changed Python destination and clears gate entries for deleted files and moved sources. Disabled python-quality and other file types do not invoke the module. Unlike ts-quality, linked worktrees are checked, each against its own gate file.

A violation appears in the completed tool result and in the project's `.opencode/tmp/quality-gate-status.json`; it blocks later commit and push attempts until a clean edit or removal clears it. The edit itself has already happened. Run `bun run smoke:opencode-python-quality` in the toolu checkout to replay native write/edit recovery, multi-file patch, and disabled-plugin scenarios on the pinned host.

### Rust post-edit quality

Add `rust-quality` to `.opencode/toolu/plugins.json` alongside `toolu`, then restart OpenCode. It requires `Cargo.toml` at the git toplevel and `cargo` on `PATH`, and runs static per-file rules on completed `.rs` writes, edits, and patches; it never invokes `cargo check`, `clippy`, or `rustfmt`. If `ast-grep` is installed, its Rust parser runs the error-handling rule (`.unwrap()`, `.expect()`, `panic!` and friends in `src/`) and the no-mocks rule; without it, those rules are skipped. Path-dependent rules match `/src/` and `/tests/` in the edited path as the tool gave it, as on other hosts: a relative `src/lib.rs` skips the error-handling, mock-definition, inline-test and docs checks, and a relative `tests/` file with a test attribute is reported as outside `tests/`. A patch checks every changed Rust destination and clears gate entries for deleted files and moved sources. Disabled rust-quality and other file types do not invoke the module. Like python-quality, linked worktrees are checked, each against its own gate file.

A violation appears in the completed tool result and in the project's `.opencode/tmp/quality-gate-status.json`; it blocks later commit and push attempts until a clean edit or removal clears it. The edit itself has already happened. Run `bun run smoke:opencode-rust-quality` in the toolu checkout to replay native write/edit recovery, multi-file patch, and disabled-plugin scenarios on the pinned host.

### ast-grep

With `ast-grep` selected, its two registry modules run on OpenCode's own tools ([#347](https://github.com/Falconiere/toolu/issues/347)):

- **search-nudge** judges `grep` (its `include` counts as the glob) and `bash`, with the same rules and text as other hosts. The host has no pre-tool advisory channel, so the nudge is appended to that call's result as `[toolu advisory]`. A call the host refuses never gets one.
- **byte-savings** appends one line per completed `read`, `grep`, `glob` and bash `ast-grep`/`sg` run to `$TOOLU_CONFIG_DIR/toolu/byte-savings/<session>.jsonl` (the project's data root). It measures the host's result text before toolu appends anything. An interrupted call, or a shell call without a confirmed exit, is not recorded. After each `ast-grep` run, the result ends with the session's report under `[toolu post-check after execution]`; `read`, `grep` and `glob` results get no extra text. OpenCode's `read` output includes line numbers and tags, so a read's `returned` can exceed the file's `full` size. The same report on demand, from the session's bash. The data root keeps one ledger per session, and the most recently written one is the current session's:

  ```bash
  bun "$TOOLU_PLUGIN_ROOT_AST_GREP/hooks/dist/byte-savings-report.js" "$(ls -t "$TOOLU_CONFIG_DIR"/toolu/byte-savings/*.jsonl | head -n 1)"
  ```

The `ast-grep-ast-grep` skill names only the `ast-grep` CLI, so its examples run as written in bash.

### Core workflows

[#358](https://github.com/Falconiere/toolu/issues/358) ports the toolu and toolu-review skills, agents and commands to OpenCode's own tools. The generator applies an exact-match port table (`tools/toolu-opencode/scripts/lib/opencode-port.ts`). When a source edit moves one of its anchors, generation fails and names the file, so Claude Code or Codex text never reaches OpenCode unnoticed. The shared host mapping (`plugins/toolu/workflows/host-mapping.md`) has an OpenCode column.

- **Agents.** The `config` hook registers `toolu-quick-task`, `toolu-deep-explore`, `toolu-research-agent`, `toolu-implementer` and `toolu-architect`. Delegate with `task` and `subagent_type`. The `task` tool takes no model argument, so each agent runs `agent.<id>.model` from your `opencode.json`, else the session's model. Set `agent.<id>.disable: true` to drop one. OpenCode's default `subagent_depth` of 1 stops a subagent from delegating again. With `models` enabled, the SessionStart context routes by agent (`plugins/toolu/hooks/docs/model-routing-opencode.md`) and does not tell the model to pass `model:`.
- **Setup.** There are no Codex profiles to install. The `toolu-setup` skill explains the overrides above, and its `setup.ts` exits 2 with that message when `TOOLU_HOST_OVERRIDE=opencode`.
- **Questions.** Workflow skills use OpenCode's `question` tool when the client offers it, and otherwise ask one concise question.
- **Review.** `toolu-review-review` runs `TOOLU_HOST_OVERRIDE=opencode toolu review write-state --findings-count 0` after reviewing the committed diff. That writes `<project>/.opencode/tmp/push-review/<branch>.json`, the file the push-review gate reads.
- **Debug.** `toolu-debug` runs its collectors as `bun "$TOOLU_PLUGIN_ROOT_TOOLU/scripts/debug-testfail.ts"` (also `debug-stack.ts` and `debug-log.ts`). `@toolu/opencode` ships them. MCP tools are named `<server>_<tool>`; there is no tool search.

`tools/toolu-opencode/src/plugin/__tests__/core-workflows.test.ts` proves the review, commit-gate and debug paths hermetically. `TOOLU_LIVE_OPENCODE=1 bun test tools/toolu-opencode/src/plugin/__tests__/core-workflows.live.test.ts` repeats them on the pinned host with a scripted provider.

### Delivery workflows

[#355](https://github.com/Falconiere/toolu/issues/355) ports brainstorm and delivery-flow through the same port table. Add `delivery-flow` to `enabled`; its `toolu`, `toolu-review`, `pr-babysit` and `brainstorm` dependencies are selected with it.

- **Skills.** Load `delivery-flow-delivery-flow` to deliver a task, or `brainstorm-brainstorm` on its own. Delivery loads brainstorm, `toolu-review-review` and `pr-babysit-babysit-73c340c6` by those ids. Each skill reads its installed references from its own directory.
- **Ledger and verdict.** The skill runs `toolu ledger …` and `toolu ledger verdict …` in bash, so the native `toolu` must be installed ([#421](https://github.com/Falconiere/toolu/issues/421)); toolu's session start reports when it is missing. `shell.env` sets `TOOLU_HOST_OVERRIDE=opencode` in every bash call, so the ledger is `<project>/.opencode/tmp/plan-ledger/<branch-slug>.json` (`/` becomes `_`), the file the plan-ledger push gate reads. A preflight refused on OpenCode names the skill: `preflight: plan not approved (Status: Draft) — load skill({ name: "delivery-flow-delivery-flow" }) (plan review phase)`.
- **Model tiers.** A plan step's `model` label picks an agent: `haiku` runs `toolu-quick-task`, `sonnet` runs `toolu-implementer`, `opus` and `fable` run `toolu-architect`, and `inherit` runs `general`. The model is that agent's `agent.<id>.model`.
- **Quality checks.** The post-edit checks run in `tool.execute.after` and append a violation to the edit's result. The quality gate then blocks the next commit and push until it is fixed.

`tools/toolu-opencode/src/plugin/__tests__/delivery-workflows.test.ts` proves the preflight, ledger, verdict and push-gate transitions hermetically. `TOOLU_LIVE_OPENCODE=1 bun test tools/toolu-opencode/src/plugin/__tests__/delivery-workflows.live.test.ts` loads both skills and their references on the pinned host and delivers to a bare remote.

### Epic orchestrator

[#356](https://github.com/Falconiere/toolu/issues/356) ports epic-orchestrator. Add `epic-orchestrator` to `enabled`; delivery-flow and its dependencies are selected with it.

- **Skill.** Load `epic-orchestrator-epic-orchestrator`, or run the `epic-orchestrator-epic` command. It runs its scripts from `$TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR` and stops with `epic-orchestrator is not enabled in this OpenCode session` when the plugin is off.
- **Watcher.** OpenCode has no background shell, so the orchestrator runs `epic-watch.ts --max-wait 480` in the foreground with a 600000 ms bash timeout and runs it again until no issue is active.
- **Workers.** An OpenCode worker is `opencode --auto --model provider/model` in its own herdr worktree. Launch refuses an `opencode` whose `--version` is not 1.x, and adds `/.opencode/toolu/state/` and `/.opencode/tmp/` to the repository's shared `info/exclude` so worker state stays out of `git status`, `refs/epic-wip/*` snapshots and worktree removal. The worker loads `delivery-flow-delivery-flow` by that id, and its native `task` calls reach agent-tier and delegation telemetry. A worktree reads its committed `.opencode/toolu/plugins.json`, else the global selection, else every installed plugin.

`tools/toolu-opencode/src/plugin/__tests__/epic-workflows.test.ts` proves the selection, script root and agent-tier paths hermetically. `TOOLU_LIVE_OPENCODE=1 bun test tools/toolu-opencode/src/plugin/__tests__/epic-worker.live.test.ts` runs a worker on the pinned host through a kill, checkpoint, `session list` capture and `--session` resume to a ready report.

### PR babysitting

Add `pr-babysit` to the `enabled` list and restart OpenCode. Run the `pr-babysit-babysit-ff6e5a3d` command (or load `pr-babysit-babysit-73c340c6`) on a branch with an open pull request. [#357](https://github.com/Falconiere/toolu/issues/357) ports its controller and fixers:

- **Controller.** The plugin API has no cron or goal, so the invoking turn runs a tick, sleeps `backoff.waitSeconds` (at most 60 s) in bash, and ticks again until CI, review threads and the review-bot verdict are clear, or a human-only blocker escalates. State is `<repo>/.opencode/tmp/pr-babysit/<slot>.json`; invoking the command again resumes from it. Helpers run as `"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_PR_BABYSIT/hooks/dist/<helper>.js"`, so a project `.env` (a `GH_TOKEN`, say) never reaches `gh`.
- **Fixers.** `babysit-route-fix.js --host opencode` reads `prBabysit` from `$TOOLU_USER_CONFIG_DIR/toolu.config.json` and `.opencode/toolu.config.json` ([config](config.md#pr-babysit-fixers-prbabysit)) and Jev only from `$TOOLU_CONFIG_DIR/jev/jev.sh`. An `opencode` group runs as a detached `opencode run --agent pr-babysit-fixer` in a native worktree beside the state file, with no herdr. The dispatcher adds that agent to `OPENCODE_CONFIG_CONTENT`, denying `task` and the plain `git push` and `gh` forms on top of your own rules; `--auto` approves asks but keeps every deny. Whatever form a command takes, the fixer gets no GitHub token, a `GH_CONFIG_DIR` with no login, and `GIT_ALLOW_PROTOCOL=file`, so git refuses every https and ssh remote while local remotes (a test's bare repository) still work. The fixer edits, tests, commits and reports; the controller verifies, pushes, replies and resolves.
- **Inline fixes** use `task` with `toolu-quick-task`, `toolu-implementer` or `toolu-architect` in a detached worktree at `.opencode/tmp/pr-babysit/<slot>.inline`.
- **Cancel.** `stop` ends a running fixer's whole process group, removes the clean worktrees and marks the state `cancelled`.

`bun run smoke:opencode-entry babysit.fixer babysit.no-report babysit.cancel` proves this on the pinned host with the scripted provider: an OpenCode controller's bash runs the shipped helpers, and an OpenCode fixer fixes a seeded failing test in an isolated repository while its push is denied.

### Status

Add `statusline` to the `enabled` list and restart OpenCode. The native `statusline-status` skill runs `"$TOOLU_BUN" --no-env-file "$TOOLU_PLUGIN_ROOT_STATUSLINE/hooks/dist/status.js"` and reports toolu's readiness first: `toolu: ready — N plugins (<selection>), M startup artifacts`, then a `Plugins:` line with each plugin's startup entries, any startup notes and the record's path and time. The repository, branch, working tree, `.opencode/tmp/quality-gate-status.json` gate and Jev readiness follow. Every toolu startup writes the record to `<data root>/toolu/opencode-status.json` and sends one structured `toolu: status` host-log entry (`opencode --print-logs`) with `status`, `plugins`, `selection`, `artifacts`, `record` and, when not ready, `reason`; no environment value is copied into either. A missing or unreadable record prints a line naming the next step.

The persistent statusline is Claude Code-only. OpenCode has no `statusLine` setting, and the pinned SDK's TUI slots (`@opencode-ai/plugin/tui`) load only in the interactive TUI, which the headless contract cannot verify. So toolu ships no OpenCode statusline UI or `/statusline:setup` command, and writes no Claude Code setting into OpenCode configuration. `bun run smoke:opencode-entry status.enabled status.disabled` proves the skill, the log entry and the clean configuration on the pinned host.

## Contributor path (git clone)

Use this only when working on toolu itself, or against an unreleased checkout. A user install needs neither a clone nor `TOOLU_REPO_ROOT`.

```bash
git clone https://github.com/Falconiere/toolu.git
cd toolu
bun install --frozen-lockfile
bun run check:opencode-surface   # optional sanity: generated mirror matches sources
```

In your application repo (not inside the clone):

1. **Environment.** Point toolu at the clone. The package directory in a checkout does not carry `plugins/`, so the adapter reads manifests, bundles and settings from there:

   ```bash
   export TOOLU_REPO_ROOT=/absolute/path/to/toolu
   ```

   `TOOLU_ROOT` is an alias, and the plugin option `repoRoot` does the same. Either takes precedence over the bundled copy.

2. **Dependencies.** In `.opencode/package.json`, add `file:` dependencies into the clone. Adjust the relative paths to your layout; OpenCode installs `.opencode/` dependencies at startup.

   ```json
   {
     "dependencies": {
       "@toolu/opencode": "file:../toolu/tools/toolu-opencode",
       "@toolu/core": "file:../toolu/packages/toolu-core"
     }
   }
   ```

3. **Local plugin.** OpenCode loads files under `.opencode/plugins/` ([OpenCode plugins](https://opencode.ai/docs/plugins/)). Create `.opencode/plugins/toolu.ts`:

   ```ts
   export { default } from "@toolu/opencode";
   ```

   Keep only one route. With both this shim and an npm `plugin` entry configured, one of them logs `toolu: duplicate load skipped`.

Selection, gate config and everything else work as described above. To leave the contributor path, delete `.opencode/plugins/toolu.ts`, the `file:` dependencies and `TOOLU_REPO_ROOT`.

## Verify on the pinned host

These commands run from a toolu checkout. Hermetic proof (matches CI):

```bash
bun run test:conformance
bun run check:opencode-docs   # this page's support section and stale-claim guard
```

`bun run smoke:opencode-entry` runs the live scenarios on the pinned host. They cover the npm route, a local shim, both at once, and a failing config entry. They also cover full startup and paths: helpers resolve from paths with spaces with no `bun` on `PATH`, and a linked worktree keeps its own data root. The first run needs the network. Pass scenario ids to run only some of them:

```bash
bun run smoke:opencode-entry                       # every scenario
bun run smoke:opencode-entry docs.quickstart docs.migration docs.migration-refusals   # this page and the migration guide, verbatim
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

The runner packs the tarball with `npm pack` through the package's own `prepack`, the way a release publishes it.

`package.clean-install` ([#361](https://github.com/Falconiere/toolu/issues/361)) loads that tarball with all 12 plugins in a sandbox where `TOOLU_REPO_ROOT` and `TOOLU_ROOT` are blank. It checks that:

- every plugin becomes ready;
- the installed tree equals the tarball and sits outside the checkout;
- helpers and published symlinks run from it;
- every export imports.

`cli.install` and `cli.lifecycle` drive the CLI against a commented global `opencode.jsonc`.

```bash
bun run smoke:opencode-entry package.clean-install cli.install cli.lifecycle
```

The pre-tool smoke checks protected edits, writes and patches, unsafe shell, the commit and push gates, MCP and task denials, and an allowed shell call:

```bash
bun run smoke:opencode-pretool
```

The permission smoke checks:

- native deny and repeated native ask rejection;
- the guardrail and judgement gate fallbacks;
- registry advice reaching the model;
- denial by a second plugin, in both load orders.

```bash
bun run smoke:opencode-permissions
```

The post-tool smoke uses isolated git projects and a scripted loopback provider. It checks edited file state, diagnostics the model sees, and the later commit and push denial:

```bash
bun run smoke:opencode-posttool
```

The ast-grep smoke enables `toolu` and `ast-grep` in one isolated project. It needs `ast-grep` on `PATH`. It loads the `ast-grep-ast-grep` skill with the native `skill` tool, runs the skill's search example from bash, and checks:

- each nudge, or its absence, in the model's tool messages;
- the savings report after the ast-grep run;
- the session ledger;
- the report CLI.

```bash
bun run smoke:opencode-ast-grep
```

All of the above runs as one required acceptance on Linux and macOS in CI, together with the host probes, the `*.live.test.ts` files, concurrency, budgets and regression controls. It needs `ast-grep` on `PATH`, and it fails, never skips, when one is missing:

```bash
bun run test:opencode
```

Full matrix and limitations: [`docs/conformance-report.md`](conformance-report.md) and the [host contract](opencode-host-contract.md).

## Host comparison

| Host        | Install doc                              | Runtime                          |
| ----------- | ---------------------------------------- | -------------------------------- |
| Claude Code | [README § Install](../README.md#install) | Bun hook bundles                 |
| Codex       | [README § Install](../README.md#install) | Bun hook bundles                 |
| OpenCode    | This file                                | In-process TypeScript dispatcher |

## Related

- [Migrating from the V2-targeted adapter](opencode-migration.md)
- [Plugin catalog](plugins/index.md)
- [OpenCode host contract](opencode-host-contract.md)
- [Portable core contracts](portable-core.md)
- [Configuration](config.md)
- [Conformance report](conformance-report.md)
- [CLI](cli/installer.md#opencode)
