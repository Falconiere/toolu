# Brainstorm: OpenCode paths, helper environment and concurrent state (#343)

Delivery mode, Full path (cross-cutting roots, a new public environment contract, persisted state).

## Capsule

- **Outcome:** On OpenCode, toolu reads a real global `toolu.config.json` from OpenCode's config directory and a project `.opencode/toolu.config.json`. It writes registry modules, helpers and its ledger to a per-project data root that no other project or worktree shares, even when an explicit override points several projects at one directory. Startup entries and gates keep the user's real `HOME` and never resolve Claude/Codex homes. Every bash call gets the non-secret toolu environment through `shell.env`, so published helpers and per-plugin CLIs resolve, including from paths with spaces and with Bun absent from `PATH`.
- **Material defaults/non-goal:** The default data root stays `<project>/.opencode/toolu/state` (OP-08). Out of scope: delivering context (OP-07), installing surfaces (OP-11), and leaf scripts that still hard-code `~/.claude` or `~/.codex` (epic-orchestrator, pr-babysit, statusline settings: OP-22, OP-23, OP-25). Claude Code and Codex behavior stays unchanged.
- **Repository evidence:** `docs/opencode.md` says "Isolating shared roots is tracked in #343". The OP-08 spec non-goal 2 defers HOME isolation, `TOOLU_PLUGIN_ROOT`, `shell.env` and shared data roots to #343. `bootstrapEnv` sets `HOME` to the data root. The adapter forces core `TOOLU_CONFIG_DIR` to the data root, so the user-level config is read from the data root. Generated skills name helpers as `${TOOLU_CONFIG_DIR:-${XDG_CONFIG_HOME:-$HOME/.config}/opencode}/<plugin>/<helper>`, which resolves nowhere without `shell.env`.
- **Risk:** A nested OpenCode launched from a toolu-on-OpenCode bash inherits the injected `TOOLU_CONFIG_DIR`, so it treats the parent's data root as an explicit override and keys its own data root under it. That is isolated, but surprising. Helpers that leaf ports have not touched may still read Claude homes when run by hand.
- **Handoff:** spec.

## Live host evidence (pinned `opencode-ai@1.18.34`)

A scratch plugin was run under the probe harness in a linked git worktree whose path contains a space:

- `PluginInput.directory` = `PluginInput.worktree` = `project.worktree` = the linked worktree root. A linked worktree is its own project root, so the per-project roots are per worktree.
- `shell.env` receives `{cwd, sessionID, callID}` and an empty `output.env`. Keys set there reach the bash tool (`TOOLU_WT_PROBE=set`).
- The bash tool environment carries `OPENCODE=1`, `AGENT=1` and `OPENCODE_PID`, plus the host process environment. It has no `TOOLU_*` variables.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| Data and state | Global root (read-only to toolu) = `TOOLU_CONFIG_DIR` → `TOOLU_OPENCODE_HOME` → `${XDG_CONFIG_HOME:-$HOME/.config}/opencode`. Data root = `<project>/.opencode/toolu/state`, or `<override>/toolu/opencode/projects/<slug>-<hash>` when an override is set | Core `configRoot({host:"opencode"})` already resolves the global root this way. OP-08 fixed the project-local default | Users of the OP-08 shared-override layout get a fresh keyed data root. The old files stay inert, and the docs say how to remove them |
| Interface | New `TOOLU_USER_CONFIG_DIR` in core (directory of the user-level `toolu.config.json`, default the config root). `TOOLU_PLUGIN_ROOT_<NAME>` per selected plugin in `shell.env`. `TOOLU_PLUGIN_ROOT` keeps its core meaning (the toolu core plugin root) | `configFiles` is the only user-config reader. `pluginRoot()` and delivery-flow already mean the core root | A new core variable must be documented in `docs/config.md` |
| Failure behavior | `shell.env` never throws. It is installed only when enforcement is ready, so a not-ready instance denies bash before any environment matters | Probe `load.init-throw`; the deny-all path from #336 | — |
| Integration | One `opencodeRuntimeEnv` builds the environment for startup children, for in-process gates and for `shell.env`. Foreign host roots (`CLAUDE_CONFIG_DIR`, `CODEX_HOME`, `PLUGIN_ROOT`, …) are removed from the child and gate environments only. The agent's own shell keeps them | `settingsDir` falls back to `~/.claude/settings` unless `TOOLU_SETTINGS_DIR` is set | Removing variables from child environments could hide a variable a leaf startup needs. The catalog startup test covers all 16 plugins |
| Constraints | `PATH` gains the resolved Bun's directory only when `bun` is not already on it | `#!/usr/bin/env bun` helper bundles | Prepending would shadow the user's own tools |

## Alternatives (Jev-assisted, `ask` over the issue text and code evidence)

- "Project/global overrides work" means both the project/global config files and the explicit path overrides: `both` 0.76, `env_paths` 0.18, `config_files` 0.06.
- Shared override design: **keyed per project under the override** 0.98. Rejected: `project_local_only` (0.02), which drops the existing override's relocation, and `shared_as_is` (0), which leaves the corruption in place.
- Plugin roots: **one variable per selected plugin** 1.0. Rejected: a single catalog-directory variable, which resolves unselected plugins' CLIs, and `TOOLU_PLUGIN_ROOT` as the package root, which conflicts with core's `pluginRoot()` and with delivery-flow.
- Bun: append the resolved Bun's directory when `bun` is missing, chosen over prepending a toolu-owned `bun` shim directory (which changes the user's `bun` in the agent shell) and over rewriting every helper call to `"$TOOLU_BUN" <helper>` (which changes skill text across leaf ports).
