# @toolu/opencode

The [toolu](https://github.com/Falconiere/toolu) bridge for [OpenCode](https://opencode.ai) — it wires toolu's bash quality gates into OpenCode's `permission.evaluate`, so an edit that violates a gate is denied before bytes change.

## Install

```bash
opencode plugin add @toolu/opencode
```

Then choose which toolu plugins are active, in your project:

```jsonc
// .opencode/toolu/plugins.json
{ "version": 1, "enabled": ["toolu"] }
```

Restart OpenCode. That is the whole install — the package carries the bash `plugins/` tree, so there is no clone and no `TOOLU_REPO_ROOT` to export.

## What you get

The `toolu` plugin brings the core hook engine: protected-file blocking, bash command gating, and the post-edit quality checks. Adding `rust-quality`, `ts-quality` or `python-quality` to `enabled` turns on that language's post-edit rules — file and function size limits, banned escape hatches, colocated real-data tests.

Enforcement scope matches the fixture evidence in [#212](https://github.com/Falconiere/toolu/issues/212); it is not yet the full Claude Code and Codex hook surface.

## Requirements

- OpenCode `v2.0.12`
- Bash ≥ 5 and `jq`, for the assembled hooks
- macOS or Linux. Windows is not supported.

Per-gate tools (rustfmt, oxlint, ruff, …) are whatever the plugins you enable require.

## Overriding the plugin root

The bundled tree is used by default. To run against a checkout instead — when working on toolu itself — set `TOOLU_REPO_ROOT` to the clone, or pass `repoRoot` as a plugin option. Both take precedence over the bundled copy.

Full documentation: **[docs/opencode.md](https://github.com/Falconiere/toolu/blob/main/docs/opencode.md)**

MIT © Falconiere Barbosa
