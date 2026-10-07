# Management surface: doctor, config, status, setup agents — Brainstorm

**Date:** 2026-10-07  **Issue:** #445  **Mode:** Delivery, full

## Outcome

`toolu doctor` reports every installation fact on one page, with ok, warn or fail per check, a fix hint, exit 1 on any fail and the full report under `--json`. `toolu config get|set|validate` reads and writes `toolu.config.json` through the fail-closed loader. `toolu status` prints the repository and gate state the statusline renders, and `crates/toolu` implements `StatusSnapshot` with it. `toolu setup agents` replaces the setup skill's Bun script. `/toolu:doctor` and `/toolu:status` run the CLI, and the setup skill calls it.

## Evidence and decisions

- `crates/toolu` already has `doctor` (the #443 reachability check), and placeholders for `config`, `status` and `setup`. `toolu-runtime` supplies most of the parts: `config::load` (fail-closed envelope and its messages), `config::epic::parse` (no credentials in `epic`), `config::secrets::{load, redact_text, redact_json}` (#463), `manifest::read` and `skew::assess` (#411), `install::upgrade_command`, `registry::{event_dir, parse_name}` and `registry::manifest::read_manifest`, `process::commands::{native_toolu_on_path, codex_plugin_list}`. The engine adds `registry::list_dir` and `registry::gate::plugin_presence`, and `toolu-state` the gate-file reader. The doctor composes them and adds no new loader.
- **Templates for `setup agents`.** They are compiled into the binary from `plugins/toolu/assets/agents/*.toml` with `include_str!`, and `TOOLU_AGENT_TEMPLATE_DIR` overrides them as it overrides the script. On Codex, `PLUGIN_ROOT` never reaches a skill's shell, so a path the binary has to discover would be fragile (Jev 1.00).
- **Codex plugin inventory.** `codex plugin list --json` gives the version and `source.path`. The SessionStart snapshot holds only ids, and the skew check needs each plugin's `plugin.json` (Jev 1.00). The doctor is on demand, so spawning the CLI is fine. Without the CLI the check warns.
- **Claude plugin inventory.** `installed_plugins.json` lists entries per scope. The entries that apply are the user scope, and the project or local scope whose `projectPath` is the current project. Each `installPath` gives the `plugin.json`.
- **OpenCode plugin inventory.** OpenCode keeps no `plugin.json` the binary can read. Its plugins run through the TypeScript adapter, not the native hook protocol, until #437. The doctor lists the plugin names from the adapter's startup record, `opencode-status.json`. The skew check is reported as not applicable there.
- **`config set` target.** The user file by default, `--project` for the project file (Jev 0.63 over "project when inside a repository", 0.35). The merged view is what `get` shows; writing the user file by default never changes what a shared repository file says behind the user's back.
- **`status` scope.** Repository facts (root, branch, ahead and behind, working-tree counts) plus gate state (quality-gate slots, push-review state, push waivers) (Jev 0.62 over gate-only, 0.37). Jev readiness, the comemory count and the OpenCode startup record stay with the statusline port (#431), which reads them from its own plugin's state.

## Alternatives rejected

- A `--templates <dir>` flag that the skill fills from its base directory. It works, but every caller has to plumb a path that the binary can carry itself.
- Reading only the Codex snapshot. It has no versions, so skew and `hookProtocol` cannot be judged.
- Writing the project file when inside a repository. The rule is the same as git's, but the file is checked in, and an agent that runs `config set` inside a worktree would change it for everyone.
- Putting everything the statusline shows into the snapshot. Jev and comemory readiness are statusline concerns, and porting them here widens #445 into #431.

## Risks

- The doctor's JSON document changes shape. `$defs/doctor` in `commands.schema.json` and the existing #443 tests are updated with it. The `reachable` and `path` fields keep their meaning inside the `reachability` check.
- Installed plugins at 7.11.0 have no `hookProtocol`, so on today's machines `doctor` fails the skew check. That is the #411 rule: an unreadable protocol is a mismatch.
- File and function size limits (300 and 50 code lines) force the doctor into a module tree under `crates/toolu/src/doctor/`, with one test file per module.

## Handoff

Spec next: `docs/toolu/specs/2026-10-07-management-surface-design.md`.
