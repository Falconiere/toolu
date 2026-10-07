# Pre-tool gates A in Rust — Design

**Date:** 2026-10-07  **Status:** Approved  **Author:** Codex  **Topic:** #419 native pre-tool gates

## Problem

`toolu hook pre-tools` still has an empty Rust built-in table. The TypeScript hook protects sensitive writes, blocks listed MCP servers, and advises which rules apply to an edit. The Rust hook must make those decisions before #425 switches hosts to the binary.

## Non-Goals

1. The standalone `mcp-tools` hook and `agent-tier` are #422. The 20 fixture rows whose entry is `mcp-tools` continue through their existing bundle when the #409 seam selects only `toolu/pre-tools`.
2. Ledger, waivers, resources, management, and configuration changes owned by #421 and #445.
3. Changing gate policy, host encoding, or the shared runtime settings schema.

## Architecture

Add `code-edit-rules`, `mcp-blocker`, and `protected-files` to `toolu_engine::builtins::PRE_TOOL` in TypeScript's `NATIVE_MODULES` byte order. They implement the existing `Gate` trait. Reuse `toolu_runtime` for settings location, list entries, loaded config, modes, and guardrail decisions; use `toolu_shell::writes::write_targets` for shell writes; use the existing engine dispatch walk for multi-path edits, precedence, and encoding. A small engine-local Bash-style matcher serves both path gates without a subprocess. The code-edit gate reads JSON leniently, matching the TypeScript module's per-field behavior; the existing strict runtime loader remains untouched. Only context plumbing needed to honor an explicit CLI plugin root may extend a shared type.

## Interfaces / Schema

- No new CLI verb or `hooks.json` entry. `toolu hook pre-tools` takes the same host payload and emits the same stdout, stderr, and exit code.
- Settings keep their current names: `protected-files.txt`, `mcp-blocklist.txt`, and `code-edit-rules.json`. Existing `TOOLU_SETTINGS_DIR`, host plugin root, and explicit `--plugin-root` precedence applies.
- Modes are `gates.protectedFiles.mode` and `gates.mcpBlocker.mode`; `ask`, `block`, `advise`, and `off` follow `gate_mode` with Codex ask degradation.
- Gate outputs use `Decision::Allow`, `Ask`, `Deny`, or `Advisory` and the exact TypeScript reason strings. Their names in the table match the TypeScript `NATIVE_MODULES` keys for trace and stderr parity.

## Failure modes and edge cases

- An absent or empty settings file yields allow. A missing, malformed, or partial edit-rule JSON document yields no advice, while a first matching rule with no docs stops later rules, as TypeScript does.
- Shell targets include redirects, write commands, nested shell/eval, dynamic target text, and existing paths matched by an unquoted pathname pattern; they are deduplicated in source order. Failed directory reads leave the literal pattern candidate.
- An invalid config envelope makes a matched security gate block. MCP `mcp.<server>: false` remains effective even if that envelope is invalid; file entries win over config entries. Non-MCP tool names and a missing blocklist/config are silent.
- Absolute paths under the project root become repo relative; outside paths stay absolute. Pattern order and the first matching write target determine the message. Empty or non-string `file_path` is ignored.
- A gate read or decision error follows the existing built-in module failure path and stderr. The pre-tool dispatcher itself retains its fail-closed malformed-payload path.

## Acceptance criteria

- **AC-1:** Every `pre-tools` row of `fixtures/gates/pre-tool-modules-a.json` and all 22 selected corpus captures yields the golden decision, message, stderr, and exit code through `TOOLU_IMPL=rust:toolu/pre-tools`. The full 109-row fixture stays green, with its 20 standalone `mcp-tools` rows still using their existing entry. A direct A/B run compares TypeScript and Rust decision-message bytes and exit codes for those 89 native rows.
- **AC-2:** A real Bash payload `echo SECRET=1 >.env` asks on Claude Code and denies on Codex; shell target patterns, edit paths, absolute paths, and mode overrides match the golden captures.
- **AC-3:** A listed MCP server is denied or asked with the exact file/config reason and redirect hint; unrelated servers and non-MCP tools stay silent through the native `pre-tools` hook.
- **AC-4:** An edit matching `code-edit-rules.json` emits the exact first-rule advice, including optional extra docs; malformed and partial JSON keep TypeScript's no-op or partial-rule behavior.
- **AC-5:** The gate coverage inventory names #419 and the native seam check for all three gates while `hostMechanism` truthfully remains `bun-bundle` until #425, and the Rust quality gate passes without suppressions or exemptions.
- **AC-6:** The portable-core protected-files fixture produces the same guarded outcome through the native `pre-tools` hook as the TypeScript fixture.

## Acceptance evidence

| AC | Real input and expected output | Boundary and runnable check |
|---|---|---|
| AC-1 | 89 `pre-tools` golden rows, 22 corpus rows: captured outputs; paired TypeScript/Rust messages equal byte for byte | 20 `mcp-tools` rows stay on their entry; `TOOLU_IMPL=rust:toolu/pre-tools bun test plugins/toolu/hooks/src/__tests__/pre-tool-modules-a*.test.ts` and a direct paired test |
| AC-2 | `echo SECRET=1 >.env`: Claude ask, Codex deny, exact reason | `.en[v]`, `cp -t`, absolute path, dynamic target, mode `off`; same fixture suite and focused Rust tests |
| AC-3 | `mcp__exampleblocked__search` with `exampleblocked` in blocklist: guardrail reason | config false, invalid envelope, redirect, no source; native CLI integration test and fixture suite |
| AC-4 | `Edit` on a matching `.rs`: `File: …\nApply these rules: …` | malformed/extra/partial JSON and first match without docs; same fixture suite and focused Rust tests |
| AC-5 | Three inventory rows cite #419 and the native seam check without changing discovered host mechanism | `bun run tooling/src/gate-coverage-inventory.ts check`, `cargo xtask gate --base origin/main`, and `bun run test` through the epic job lease |
| AC-6 | `fixtures/portable-core/protected-files-pre.json` with `/repo/.env`: guarded edit | Run the portable-core conformance fixture and an equivalent native hook payload through the harness seam |

## Documentation impact

Update the coverage inventory and the Rust engine/fixture documentation to name the three native pre-tool gates. No host-facing configuration key or command changes.

## Open Questions

None. The #419/#422 split follows the epic ownership and the #409 entry-specific seam; Jev selected the gate-only interpretation with probability 0.90.
