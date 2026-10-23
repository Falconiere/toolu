# Cursor as an epic worker host — Design

**Date:** 2026-10-23   **Status:** Draft   **Author:** Falconiere Barbosa   **Topic:** install, hooks, doctor and live smoke for Cursor (#589, re-scoped)

## Problem
`route` and `launch` accept `cursor`, but `toolu plugins` cannot install into Cursor, doctor says `plugin inventory is unknown for this host`, and nothing proves toolu's hooks run in a Cursor session. A Cursor worker cannot follow the delivery-flow brief. Refusing Cursor (first cut of this PR) is no longer acceptable: Cursor must be supported.

## Non-Goals
1. Muse host work (#603, #604, #605).
2. Changing the Cursor payload/encoder code in `toolu_protocol` (it exists).
3. Cursor-only plugin content (rules, `.cursor-plugin` bundles) beyond what hooks need.

## Architecture
Decisions, with reasons:
- **Hooks: prove, don't generate.** Cursor's docs (hooks, third-party hooks; CLI changelog 2026-08-11) say plugin hooks in Claude format run and map `PreToolUse`, `PostToolUse`, `SessionStart`, `UserPromptSubmit`, `PreCompact` to Cursor's camelCase events; `${CLAUDE_PLUGIN_ROOT}` is expanded by Cursor. Every toolu entry uses only those events. So no `.cursor-plugin` manifests; instead `check-hooks` gains a Cursor-compat rule: each Claude event in every `hooks.json` must map to a Cursor event through `toolu_protocol::native`. A live host-smoke run is the proof; if it fails, generate Cursor entries (follow-up inside this PR).
- **Doctor reads disk.** Cursor caches a marketplace plugin at `~/.cursor/plugins/cache/<marketplace>/<plugin>/<rev>/` with `.cache-complete` and the Claude-format `.claude-plugin/plugin.json`. A new `doctor/cursor.rs` lists `cache/toolu/*` (newest complete rev per plugin), reads name/version, and checks each plugin's `hooks/hooks.json` is present (hooks effective).
- **Installer through `cursor-agent`.** `Target::Cursor` drives `cursor-agent plugin marketplace add <git-url> --git-ref <ref>` (documented, scriptable). The per-plugin install/list/remove/update commands are **not documented**; their argv and JSON shapes are captured from the live CLI into `fixtures/plugins/cursor.json` (as Muse's are) and implemented last.
- **Epic.** `HostKind::Cursor` stays accepted: the #600 refusal is reverted.
- **Smoke.** `cargo xtask host-smoke --host cursor` follows `host_smoke_muse.rs`.

## Interfaces / Schema
`Target::Cursor` (`--host cursor`, binary `cursor-agent`); `doctor::cursor::collect(&Roots) -> Inventory`; `check-hooks` rule `cursor-events`; `host-smoke --host cursor|all`. No flag removed.

## Failure modes and edge cases
- Cursor not installed / keychain locked: `cursor-agent` exits non-zero; installer reports its stderr as a `Fail`, exit follows the CLI contract. Doctor needs no CLI and still reads disk.
- Empty cache dir or incomplete rev (no `.cache-complete`): plugin not listed.
- A cached plugin with no `hooks/hooks.json`: doctor warns "hooks missing".
- A hook event with no Cursor mapping: `check-hooks` fails naming plugin and event.
- Unparseable `plugin.json`: doctor warns and skips that plugin; never panics.

## Acceptance criteria
- **AC-1:** `route --hosts claude,cursor:2` and `launch --kind cursor` are accepted again; `unknown host gemini` still fails.
- **AC-2:** `toolu --host cursor doctor` against a real Cursor cache tree lists the cached toolu plugins with versions and no longer says `plugin inventory is unknown`.
- **AC-3:** `cargo xtask check-hooks` passes on the repository and fails on a `hooks.json` fixture with an event Cursor lacks.
- **AC-4:** `cargo xtask host-smoke --host cursor` is wired (argument parsing, project setup, command construction) and refuses cleanly when `cursor-agent` is absent.
- **AC-5:** (live) `toolu plugins install delivery-flow --host cursor` then `toolu --host cursor doctor` passes; `host-smoke --host cursor` passes; a Cursor epic launch records `running` with a captured session.

## Acceptance evidence
AC-1: `epic-orchestrator` unit tests and `cli` epic tests. AC-2: `doctor/tests/cursor_test.rs` over a fixture copied from this machine's real `~/.cursor/plugins/cache/toolu/ast-grep/<rev>` tree. AC-3: xtask fixture cases `fixtures/guardrails`-style plus the repo run. AC-4: xtask tests; the binary run with no `cursor-agent` on PATH. AC-5: live commands, last in the ledger, need an unlocked keychain and a logged-in `cursor-agent`; their output is linked on the PR.

## Documentation impact
`docs/cli/` (regenerated), `plugins/epic-orchestrator/{README.md,skills/…/SKILL.md}` (Cursor install section restored with real commands), `docs/install.md` / `docs/cli/installer.md` for `--host cursor`, `docs/host-smoke` mention in AGENTS.md key files.

## Open Questions
- Cursor's plugin install/list/remove/update argv and JSON: owner worker, blocks only the live installer step (AC-5), resolved by capture.
- Whether Cursor expands `${CLAUDE_PLUGIN_ROOT}` for plugin hooks: owner worker, resolved by the live smoke; falls back to generated entries.
