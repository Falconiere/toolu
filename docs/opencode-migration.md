# Migrating from the V2-targeted OpenCode adapter

Up to 7.7.2, `@toolu/opencode` targeted OpenCode V2: `@opencode/cli@2.0.12` with `@opencode/plugin@2.0.12`, through `Plugin.define` and `permission.evaluate`. From 7.8.0 ([#366](https://github.com/Falconiere/toolu/issues/366)) the package implements the plugin API documented at <https://opencode.ai/docs/plugins/>, and it loads only on `opencode-ai@1.18.34` ([host contract](opencode-host-contract.md)). This guide moves an existing V2 setup onto that line and back. New installs follow [docs/opencode.md](opencode.md).

CI checks this guide. The `docs.migration` acceptance check seeds a profile with V2-era state and runs the [migrate](#migrate) and [roll back](#roll-back) blocks below word for word against the pinned host. The seeded state is:

- a commented global `opencode.jsonc` with an unpinned `@toolu/opencode` entry and another plugin;
- the clone shim;
- a `.opencode/package.json` with the V2 SDK dependency;
- a selection and a gate config.

After the migrate block, the check expects:

- toolu loads once and refuses a protected write;
- the backups are readable only by their owner;
- your selection and gate config are unchanged;
- your comments and the other plugin are kept.

After the rollback block, every seeded file must match its original byte for byte.

`docs.migration-refusals` runs the migrate block where `update` must fail:

- a clone-only setup with no entry, which must exit `1`;
- a package configured in both scopes, which must exit `2`.

In both cases the block must stop with every user file unchanged.

## What changes

| | V2-targeted (≤ 7.7.2) | Documented line (≥ 7.8.0) |
|---|---|---|
| Host | `@opencode/cli@2.0.12` | `opencode-ai@1.18.34` |
| SDK | `@opencode/plugin@2.0.12`, which you installed | `@opencode-ai/plugin@1.18.34`, which the host provisions |
| Install | `opencode plugin add @toolu/opencode` | `npx @toolu/plugins install --host opencode` |
| Update / remove | `opencode plugin check`, `update`, `remove` | `npx @toolu/plugins update` / `remove … --host opencode` |
| Enforcement | `permission.evaluate` on edit, write and bash | `tool.execute.before` on every covered tool, plus post-tool checks in `tool.execute.after` |
| Skills, agents, commands | Wired by hand (`skills.paths`, copied files) | Added by toolu's `config` hook at each start |
| Readiness | A marker file from an earlier start | The current startup run only |

## Who owns what

| Path | Owner | Migration | Rollback |
|---|---|---|---|
| The `@toolu/opencode` entry in the global `opencode.json`/`opencode.jsonc` `plugin` array (where V2's `plugin add` wrote it) | The `toolu` CLI | `update` rewrites it to the CLI's release, keeping comments, options and other entries | Restored from the backup |
| `toolu/plugins.json` (global or `.opencode/toolu/`) | You; named `install`/`remove` edit it | Unchanged: same schema and paths | Unchanged |
| `toolu.config.json` (global or `.opencode/`) | You | Unchanged: same schema | Unchanged |
| `.opencode/plugins/toolu.ts` (the V2 clone shim) | You | Deleted. Kept, it would load a second, skipped instance | Restored |
| `.opencode/package.json` dependencies `@opencode/plugin`, `@toolu/opencode`, `@toolu/core` | You | Deleted. The host adds its own SDK dependency at startup | Restored |
| `TOOLU_REPO_ROOT` / `TOOLU_ROOT` in your environment | You | Unset, unless you stay on the [contributor path](opencode.md#contributor-path-git-clone) | — |
| Hand-wired `skills.paths` to `generated/skills`, or generated skills, agents and commands copied into `.opencode/` or `~/.config/opencode/` | You | Remove them by hand (see [after migrating](#after-migrating)) | — |
| `.opencode/toolu/state/` (data root) | toolu | Rebuilt at each start. Safe to delete | Deleted; whichever toolu starts next rebuilds it |
| `.opencode/tmp/` (gate state) | toolu | Kept | Kept: the backup leaves it out |

## Before you start

1. Install the pinned host. Both `@opencode/cli` and `opencode-ai` install an `opencode` command, and npm will not overwrite one package's command with another's. Run `npm uninstall -g @opencode/cli` first, then `npm install -g opencode-ai@1.18.34`. Afterwards, `opencode --version` must print `1.18.34`.
2. Run the block from each project where you used toolu. The global config is backed up and migrated on the first run. Later runs keep that first backup and only back up and clean the project.
3. If the package is configured in both your global and project config, add `--scope user` or `--scope project` to `update`. Without it, the CLI exits `2` and writes nothing.

## Migrate

<!-- opencode-doc:migrate:start -->

```bash
test "$(opencode --version)" = 1.18.34
config="${XDG_CONFIG_HOME:-$HOME/.config}/opencode"
backup="$HOME/toolu-opencode-v2-backup"
project="$backup/projects/$(printf %s "$PWD" | tr '/ ' '__').tgz"
(umask 077 && mkdir -p "$backup/projects")
[ -e "$backup/global.tgz" ] || [ ! -d "$config" ] || (umask 077 && tar -czf "$backup/global.tgz" --exclude node_modules -C "$config" .)
[ -e "$project" ] || [ ! -d .opencode ] || (umask 077 && tar -czf "$project" --exclude node_modules --exclude .opencode/tmp --exclude .opencode/toolu/state .opencode)
npx @toolu/plugins update --host opencode
rm -f .opencode/plugins/toolu.ts
[ ! -f .opencode/package.json ] || (cd .opencode && npm pkg delete "dependencies.@opencode/plugin" "dependencies.@toolu/opencode" "dependencies.@toolu/core")
npx @toolu/plugins list --host opencode
```

<!-- opencode-doc:migrate:end -->

- **Backups.** They go under `~/toolu-opencode-v2-backup/`, readable only by you, because a global config can hold provider keys. A backup that already exists is never overwritten, so running the block again cannot replace the V2 state with migrated state. `node_modules`, toolu's data root and gate state are left out.
- **`update`.** It rewrites every `@toolu/opencode` entry, the V2 one included, to this release, and leaves your selection alone. When it fails, the block stops before changing anything else:
  - **Exit `1`, not configured:** only the clone shim loaded toolu, so there is no entry to update. Add one, then run the block again:
    - with a selection file, `npx @toolu/plugins install toolu --host opencode` keeps it (adding `toolu` if it is missing);
    - with no selection file, `npx @toolu/plugins install --host opencode` enables every plugin, as before.

    A bare `install` with an existing selection would enable every plugin in it.
  - **Exit `2`:** the package is configured in both your global and project config. Add `--scope user` or `--scope project` to the `update` line.
- **`list`** shows the entry OpenCode will load and the enabled plugins.
- **Restart OpenCode.** The host log (`opencode --print-logs`) shows one `toolu: ready (…)` line, and the [quick start](opencode.md#quick-start) check refuses the scratch `.env.toolu-check` write.

## After migrating

Remove any surface you wired by hand for V2: a `skills.paths` entry pointing at `generated/skills`, or skills, agents and commands copied into `.opencode/` or `~/.config/opencode/`. Your definitions win over toolu's, so a stale copy would hide the current one.

- `opencode debug skill` prints each skill's `location`. A toolu skill located outside the installed `@toolu/opencode` package is a copy.
- Each start's `toolu: surface notes:` log line names every user skill that hides one of toolu's.

Before #343, an override root (`TOOLU_CONFIG_DIR` or `TOOLU_OPENCODE_HOME`) was itself toolu's data root. Files toolu left there are no longer used; see [Roots and helper environment](opencode.md#roots-and-helper-environment).

## Roll back

Run this from the same project. It restores every file the backups hold and deletes toolu's data root. The migration creates no other toolu file. OpenCode's own startup files in its config directories (`package.json`, `node_modules`, `.gitignore`) stay where they are.

<!-- opencode-doc:rollback:start -->

```bash
config="${XDG_CONFIG_HOME:-$HOME/.config}/opencode"
backup="$HOME/toolu-opencode-v2-backup"
project="$backup/projects/$(printf %s "$PWD" | tr '/ ' '__').tgz"
[ ! -f "$backup/global.tgz" ] || tar -xzf "$backup/global.tgz" -C "$config"
[ ! -f "$project" ] || tar -xzf "$project"
rm -rf .opencode/toolu/state
```

<!-- opencode-doc:rollback:end -->

Then swap the hosts back: `npm uninstall -g opencode-ai`, then `npm install -g @opencode/cli@2.0.12`. If your restored entry is the unpinned `@toolu/opencode`, pin it to `@toolu/opencode@7.7.2`, the last V2-targeted release; no later release loads on V2. CI checks that every backed-up file is restored byte for byte. It does not test running the V2 host again.

To go back to an earlier release on the documented line instead, pin it with `TOOLU_OPENCODE_PACKAGE=@toolu/opencode@<X.Y.Z> npx @toolu/plugins update --host opencode` ([Update, roll back and remove](opencode.md#update-roll-back-and-remove)).

## What is not retained

- **No V2 host support.** The package entry is the documented `PluginModule`. In the required acceptance, `control.v2-entry` swaps in the V2 `Plugin.define`/`permission.hook("evaluate")` entry and expects the `entry.local-shim` check to fail on it.
- **Two internal V2-shaped exports remain.**
  - `@toolu/opencode/adapter/evaluate` exports `createPermissionEvaluateHandler`.
  - `@toolu/opencode/adapter/permission-map` keeps the `permission.evaluate` mapping.

  Only the historical `smoke:clean-install` lane ([#279](https://github.com/Falconiere/toolu/issues/279)) uses them, by driving the adapter directly against `@opencode/cli@2.0.12`. No supported host calls them, and they are not an install route.
