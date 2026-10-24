# Cursor as an epic worker host — Design

**Date:** 2026-10-23   **Status:** Approved   **Author:** Falconiere Barbosa   **Topic:** install, hooks, doctor and live smoke for Cursor (#589, re-scoped)

## Problem
`route` and `launch` accept `cursor`, but `toolu plugins` cannot install into Cursor, doctor says `plugin inventory is unknown for this host`, and nothing proves toolu's hooks run in a Cursor session. A Cursor worker cannot follow the delivery-flow brief. Refusing Cursor (first cut of this PR) is no longer acceptable: Cursor must be supported.

## Non-Goals
1. Muse host work (#603, #604, #605).
2. Changing the Cursor payload/encoder code in `toolu_protocol` (it exists).
3. Cursor-only plugin content (rules, `.cursor-plugin` bundles) beyond what hooks need.

## Architecture
Decisions, with reasons:
- **Hooks: prove, don't generate.** Verified live on cursor-agent 2026.10.01: with `plugins/toolu` in `~/.cursor/plugins/local/toolu` (and with `--plugin-dir`), a print-mode prompt to write `.env` is denied by toolu's `protectedFiles` gate. Cursor therefore runs the Claude-format `hooks/hooks.json` entries (documented: it maps `PreToolUse`, `PostToolUse`, `SessionStart`, `UserPromptSubmit`, `PreCompact` to camelCase events). So no `.cursor-plugin` manifests; `check-hooks` gains a Cursor-compat rule: each Claude event in every `hooks.json` must map to a Cursor event through `toolu_protocol::native`.
- **Install without a Cursor command.** `cursor-agent plugin` has only `marketplace {add,list,update,remove}`; plugin installs are interactive (`/plugin`) and account-side, with no local registry. Cursor's documented CLI-free path is `~/.cursor/plugins/local/<plugin>/` (a real directory, symlinks out are skipped), loaded without flags (verified). So `toolu plugins install|list|remove|update --host cursor` is a filesystem installer: clone toolu's `v<version>` tag (the Muse checkout design, separate dir `toolu/cursor-marketplace`), copy `plugins/<name>/` into the local directory in dependency order, and write a `.toolu-install.json` marker (tag, version) so list/update/remove touch only what toolu installed. A same-named directory without the marker is left alone (`skew`). `--scope` is refused as on Codex/Muse. `cursor-agent` is not needed to install and the keychain does not matter.
- **Doctor reads disk.** `doctor/cursor.rs` lists `<cursor>/plugins/local/*` (toolu's marker) and `<cursor>/plugins/cache/toolu/*/<rev>` (marketplace installs, only with `.cache-complete`), reads `.claude-plugin/plugin.json` for name/version, and reports `hooks/hooks.json` present or missing.
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
- **AC-6:** `toolu plugins install delivery-flow toolu --host cursor --catalog …` against a real local git origin carrying a `vX` tag copies the plugin directories (dependencies first) into `<cursor>/plugins/local` with markers; a second install reports `already`; `list` shows installed versions; a directory without a marker is left (`skew`); `remove --yes` deletes only marked directories; `update` moves to a newer tag. `--scope` and a missing git/tag fail with the CLI exit codes.
- **AC-5:** (live) `toolu plugins install delivery-flow --host cursor` (a temporary `CURSOR_CONFIG_DIR`/HOME) then `toolu --host cursor doctor` passes, and `list`, `update`, `remove` round-trip; `host-smoke --host cursor` passes; a Cursor epic launch records `running` with a captured session.

## Acceptance evidence
AC-1: `epic-orchestrator` unit tests and `cli` epic tests. AC-2: `doctor/tests/cursor_test.rs` over a fixture copied from this machine's real `~/.cursor/plugins/cache/toolu/ast-grep/<rev>` tree. AC-3: xtask fixture cases `fixtures/guardrails`-style plus the repo run. AC-4: xtask tests; the binary run with no `cursor-agent` on PATH. AC-6: `crates/plugins` unit tests and a `crates/cli` test that spawn the real binary and real git on a temp origin repo. AC-5: live commands, last in the ledger, need an unlocked keychain and a logged-in `cursor-agent`; their output is linked on the PR.

## Documentation impact
`docs/cli/` (regenerated), `plugins/epic-orchestrator/{README.md,skills/…/SKILL.md}` (Cursor install section restored with real commands), `docs/install.md` / `docs/cli/installer.md` for `--host cursor`, `docs/host-smoke` mention in AGENTS.md key files.

## Open Questions
- None blocking: Cursor's missing install command was resolved by the local-plugins design above.
- Whether Cursor expands `${CLAUDE_PLUGIN_ROOT}` for plugin hooks: answered by the live deny above (the launcher got a usable root); the smoke re-proves it.
