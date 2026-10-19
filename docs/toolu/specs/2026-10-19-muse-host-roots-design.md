# Muse host detection and roots — Design

**Date:** 2026-10-19  **Status:** Approved  **Author:** Codex  **Topic:** Falconiere/toolu#542

## Problem

Muse Code sets `PLUGIN_ROOT` as a compatibility alias. Current detection takes
that as Codex, so Muse hooks read the wrong config and state paths. The host
must be explicit before later Muse payload, encoder and installer work can use
the shared host abstractions.

## Non-Goals

1. Muse payload parsing, output encoding, tool-name normalization and gates
   belong to #543–#545.
2. Muse manifests and installer adapter belong to #546–#549. These roots
   return host-native command strings only.
3. Broad user documentation belongs to #551.

## Architecture

Add `Host::Muse` after Hermes in `Host::ALL`; existing CLI host validation and
help derive from the array. Reuse the protocol PascalCase event table for Muse.
Keep Muse-only events (`Notification`, `Stop`, `SubagentStop`,
`PostToolUseFailure`) unmapped. Runtime detection order is valid override,
host-unique event, Cursor variables, `MUSE_PLUGIN_ID` or `MUSE_SESSION_ID`,
`PLUGIN_ROOT`, then Claude. An invalid override preserves its warning and
falls through. This makes shared event names ambiguous-safe.

Until #543/#544 supply the distinct Muse payload and output encoder, parsing
uses the shared Claude-shaped hook payload and Muse decision encoding returns
an explicit error. `run_hook` converts that error to stderr plus exit 2 on
enforcing events, preserving the epic's fail-closed rule during staged delivery.
It never emits Claude's incompatible decision JSON for Muse.

Extend `Roots` matches rather than branching callers. Keep `TOOLU_CONFIG_DIR`
and `TOOLU_PROJECT_DIR` as highest-priority generic overrides. Muse project
dirname is `.agents`, except when `TOOLU_PROJECT_CONFIG_DIRNAME` overrides it.
Muse-specific plugin root/data variables take priority over the existing
Claude compatibility fallback. The slash invocation form follows current
Claude-shaped hosts and Meta's Muse Code extending guide, which documents
slash shortcuts for skills. A live plugin command check will verify namespace
spelling.

## Interfaces / Schema

- `Host::Muse`, `Host::name() == "muse"`, and `Host::parse("muse")`.
- `native_event(Host::Muse, event)` returns the PascalCase shared name for
  every `HostEvent`; extra Muse names return `None` from `canonical_event`.
- `Roots::config_root()` for Muse: `TOOLU_CONFIG_DIR`, then
  `MUSE_CONFIG_DIR`, then `$XDG_CONFIG_HOME/muse`, then `$HOME/.config/muse`.
- `Roots::project_root()` for Muse: `TOOLU_PROJECT_DIR`, then git toplevel;
  `project_dirname()` is `.agents` by default.
- `Roots::plugin_root()` prefers `MUSE_PLUGIN_ROOT`;
  `plugin_data()` prefers `MUSE_PLUGIN_DATA_DIR`; existing Claude aliases
  remain fallback values.
- `Roots::invocation("toolu", "setup")` returns `/toolu:setup`.
- `plugin_install_command(spec)` returns `muse plugins install <spec>`;
  `plugin_uninstall_command(name)` returns `muse plugins remove <name>` because
  Muse removes by its native plugin ID, as its CLI and installed-plugin list show.

## Failure modes and edge cases

Empty environment values count as unset, as `Env::get` currently defines.
An invalid host override warns once and still detects Muse from its variables.
Without a Muse variable or override, `PreToolUse` alone does not select Muse.
Without `TOOLU_PROJECT_DIR` or a git toplevel, project paths remain `None`.
Empty command names preserve the existing `CallerError`. `--host mus` remains
an unknown-host usage error. No `HOOK_PROTOCOL` bump is needed for an additive
host and event mapping.
Before #544 lands, a Muse decision fails with a named encoder error: enforcing
events exit 2 with a nonempty blocked stderr; context events report a
`systemMessage` with exit 0.

## Acceptance criteria

- **AC-1:** `toolu --host muse commands --json` succeeds; `--host mus` fails
  with the known-hosts error, and the command snapshot adds Muse.
- **AC-2:** Muse variables select Muse ahead of `PLUGIN_ROOT`, while
  `PLUGIN_ROOT` alone still selects Codex; valid and invalid overrides behave
  as specified.
- **AC-3:** Muse roots resolve the config, project, plugin and data paths,
  invocation, and install/uninstall commands above, including XDG fallback
  and a real git worktree project root.
- **AC-4:** Shared `PreToolUse` maps to Muse and the existing hosts;
  Muse-only `Stop` remains unmapped, and a shared event alone does not
  identify Muse.
- **AC-5:** `cargo xtask check-cli-compat` and the full `cargo xtask gate`
  pass with the additive surface.

## Acceptance evidence

| AC | Real input and expected result | Boundary / runnable check |
|---|---|---|
| AC-1 | Native CLI with `--host muse` exports JSON; `mus` names valid hosts | `cargo test -p toolu-cli --test contract`; CLI invocation |
| AC-2 | `MUSE_SESSION_ID` or `MUSE_PLUGIN_ID` plus `PLUGIN_ROOT` returns Muse; `PLUGIN_ROOT` alone returns Codex | `cargo test -p toolu-runtime host::detect`; invalid override test |
| AC-3 | Environment pairs and an on-disk git worktree return exact Muse paths and commands | `cargo test -p toolu-runtime host::roots`; `fixtures/host/root.json` replay; Muse 1.4.10 CLI help for command grammar and Muse slash syntax |
| AC-4 | `PreToolUse` owner list includes Muse; `Stop` has no mapping | `cargo test -p toolu-protocol native`; shared-name fallback test |
| AC-5 | Current documented CLI contract and all Rust quality checks remain green | `toolu epic job -- cargo xtask check-cli-compat`; `toolu epic job -- cargo xtask gate` |

## Documentation impact

This design and the generated `commands --json` snapshot document this
issue's additive CLI surface. User guides are owned by epic issue #551.

## Open Questions

None. The exact plugin namespace spelling will be checked against live Muse
behavior before completion.
