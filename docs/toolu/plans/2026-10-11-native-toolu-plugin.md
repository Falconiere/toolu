# Native toolu plugin switch — Plan

**Date:** 2026-10-11   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-11-native-toolu-plugin-design.md   **Topic:** #425. Native host hooks and debug commands, with one-release Bun fallback.

## Evidence and approach

The approved spec follows issue #425 and epic #402. `plugins/toolu/hooks/hooks.json` has eight Bun entries. The native binary already handles seven names through `crates/cli/src/hook.rs`; `check-binary` still uses `packages/toolu-core/src/startup/native-toolu.ts`, whose Rust equivalent is `toolu_runtime::startup::native_toolu_advice`. `crates/core/protocol/src/launcher.rs` already generates native commands with a missing-binary Bun fallback, but has no `TOOLU_RUNTIME=bun` selector. `crates/toolu/src/debug.rs` is a placeholder, while the real captured inputs and Bun helpers are under `plugins/toolu/scripts/`. `tools/toolu-opencode/src/adapter/evaluate.ts` and `tool-post.ts` still import TypeScript gates. Preserve those consumers and the eight transition bundles; use knip and the pack gate to remove only proven dead TypeScript files. `package.json` has `test:unit` and `test:conformance` but no `test` alias.

The first notice commit (#483) is on main but is not contained in `v7.11.0`. The PR may be prepared now; the orchestrator must sequence its merge after a notice release. This does not require changing hook behavior.

## Workstream summary

Launcher and startup handler → generated hook inventory and conformance → native debug CLI → dead-code cleanup → docs and real host smoke → whole-repository verification and delivery audit.

## Steps (machine-readable)

```json
[
  {
    "id": "launcher",
    "title": "Add native check-binary handler and explicit Bun rollback selection to the generated launcher",
    "check": "cargo test -p toolu-cli --test launcher --test tool_hooks && cargo test -p toolu-protocol --lib launcher",
    "ac_refs": ["AC-1", "AC-3"],
    "paths": ["crates/cli/src/hook.rs", "crates/cli/src/hook/tests", "crates/cli/tests/launcher.rs", "crates/cli/tests/helpers", "crates/core/protocol/src/launcher.rs", "crates/core/protocol/src/tests", "crates/toolu/src/lifecycle", "crates/core/runtime/src/startup/native.rs", "plugins/toolu/hooks/src/check-binary.ts", "tooling/fixtures/native-launcher", "tools/toolu-opencode/src/bootstrap/native-launcher.txt"],
    "input": "real /bin/sh launcher with installed toolu and committed check-binary bundle; TOOLU_RUNTIME=bun with real Bun; invalid TOOLU_BIN, absent Bun or bundle, and both enforcing/context events assert exit and advice"
  },
  {
    "id": "hook-switch",
    "title": "Generate all eight toolu hooks.json entries and require their native conformance suites",
    "check": "cargo xtask check-hooks && cargo test -p toolu-cli --test native_toolu_manifest --test tool_hooks --test pre_tool_gates --test post_tool_gates && bun run test:rust-conformance",
    "ac_refs": ["AC-1", "AC-4"],
    "depends_on": ["launcher"],
    "paths": ["plugins/toolu/hooks/hooks.json", "plugins/toolu/hooks/src", "plugins/toolu/hooks/dist", "plugins/epic-orchestrator/hooks/dist/check-binary.js", "plugins/pr-babysit/hooks/dist/check-binary.js", "fixtures/rust-ported.json", "fixtures/gate-coverage/inventory.json", "docs/gate-coverage-matrix.md", "tools/toolu-conformance", "tooling/src/rust-conformance.ts", "tooling/src/check-hooks-json.ts", "tooling/src/__tests__/check-hooks-json.test.ts", "tooling/src/__tests__/gate-coverage-inventory.test.ts", "tooling/fixtures/native-launcher", "tools/toolu-opencode/src/bootstrap", "tools/toolu-opencode/src/plugin/__tests__/context-delivery.test.ts", "tools/toolu-opencode/generated", "packages/toolu-core/src/startup", "plugins/epic-orchestrator/scripts/__tests__/launch-issue.test.ts", "crates/cli/tests/native_toolu_manifest.rs", "crates/cli/tests", "crates/toolu/src"],
    "input": "the existing eight matchers and shared real hook payload fixtures; a missing required binary, bad generated command or removed transition bundle fails the check"
  },
  {
    "id": "debug-cli",
    "title": "Port debug io, log, stack and testfail to the native CLI against captured transcripts",
    "check": "cargo test -p toolu-cli --test debug_cli && cargo test -p toolu-hub --lib debug",
    "ac_refs": ["AC-6"],
    "depends_on": ["launcher"],
    "paths": ["crates/toolu/src/debug.rs", "crates/toolu/src/debug", "crates/toolu/src/tests/debug_test.rs", "crates/cli/tests/debug_cli.rs", "crates/cli/src/dispatch.rs", "plugins/toolu/scripts/__tests__/fixtures/debug", "plugins/toolu/scripts/debug-io.ts", "plugins/toolu/scripts/debug-log.ts", "plugins/toolu/scripts/debug-stack.ts", "plugins/toolu/scripts/debug-testfail.ts"],
    "input": "captured big.log, Node and Rust stack traces, and Bun and Cargo failing test transcripts; empty stdin, missing --file, low caps and unrecognized text exercise failure and boundary behavior"
  },
  {
    "id": "cleanup",
    "title": "Remove only dead TypeScript toolu entries and tests, retain OpenCode gates and eight fallback bundles, and add bun run test alias",
    "check": "cargo xtask gate --only final-removal --only dist --only pack && bun run test:unit && bun run test:conformance",
    "ac_refs": ["AC-4", "AC-8"],
    "depends_on": ["hook-switch", "debug-cli"],
    "paths": ["plugins/toolu/hooks/src", "plugins/toolu/hooks/dist", "plugins/toolu/scripts", "packages/toolu-core/src", "tools/toolu-opencode", "tooling/src", "package.json", "fixtures/rust-ported.json"],
    "input": "knip and pack closure over the actual workspace imports and published plugin files; the unit and conformance suites run against remaining TypeScript/OpenCode consumers"
  },
  {
    "id": "docs",
    "title": "Regenerate the CLI reference and align toolu Markdown, runtime, registry, install and agent instructions",
    "check": "cargo xtask gate --only docs-cli --only context-budget --only packaging && cargo xtask check-markdown-cli && bun test tooling/src/__tests__/native-toolu-markdown.test.ts",
    "ac_refs": ["AC-7"],
    "depends_on": ["hook-switch", "debug-cli", "cleanup"],
    "paths": ["docs/cli", "docs/runtime.md", "docs/registry.md", "docs/install.md", "plugins/toolu/README.md", "plugins/toolu/skills", "plugins/toolu/commands", "plugins/toolu/agents", "tooling/src/__tests__/native-toolu-markdown.test.ts", "AGENTS.md", "crates/toolu/src/debug.rs", "crates/cli/src/commands.schema.json"],
    "input": "the generated command tree and every toolu skill, command and agent Markdown; a stale CLI verb fails the drift check and a Bun or script invocation fails the native-Markdown test"
  },
  {
    "id": "live-hosts",
    "title": "Exercise native deny, quality state and commit gate in actual Claude Code and Codex sessions on macOS",
    "check": "bun run tooling/src/native-toolu-host-smoke.ts",
    "ac_refs": ["AC-2", "AC-3"],
    "depends_on": ["docs"],
    "paths": ["tooling/src/native-toolu-host-smoke.ts", "plugins/toolu/hooks/hooks.json", "plugins/toolu/.claude-plugin/plugin.json", "crates/cli/src/hook.rs", "crates/toolu/src/tool_hook.rs", "crates/toolu/src/lifecycle", "packages/toolu-core/src/state"],
    "input": "temporary installed host profiles and real macOS repository, with a denied Bash command, a source edit that records a quality gate failure, a blocked commit, and a clean allowed control; capture host transcript and gate file"
  },
  {
    "id": "full-gate",
    "title": "Run the Bun product suites, pinned OpenCode acceptance, native hook benchmark and complete Rust gate",
    "check": "bun run test && bun run test:opencode && bun run bench:hooks && cargo xtask gate --base origin/main --title 'feat(toolu): switch plugin to native binary (#425)'",
    "ac_refs": ["AC-5", "AC-8"],
    "depends_on": ["cleanup", "docs", "live-hosts"],
    "paths": ["crates", "plugins", "packages", "tools", "tooling", "fixtures", "benchmarks", "docs", "AGENTS.md", "Cargo.toml", "Cargo.lock", "package.json", "bun.lock"],
    "input": "the committed branch diff, real benchmark payloads, actual pinned OpenCode host, and all Rust/Bun product suites; any warning, test failure or budget excess blocks delivery"
  }
]
```

## Critical files

- `crates/core/protocol/src/launcher.rs` and its tests; `crates/cli/src/hook.rs` and its tests.
- The generated launcher goldens under `crates/core/protocol/src/tests/fixtures/` and `tooling/fixtures/native-launcher/`, plus OpenCode's bootstrap launcher text.
- `plugins/toolu/hooks/hooks.json`, eight fallback entry sources/bundles, `fixtures/rust-ported.json`, and the conformance seam.
- `crates/toolu/src/debug.rs` and its colocated modules/tests; `crates/cli/tests/debug_cli.rs`; captured inputs under `plugins/toolu/scripts/__tests__/fixtures/debug/`.
- `package.json`, `tooling/src/native-toolu-host-smoke.ts`, `tooling/src/__tests__/native-toolu-markdown.test.ts`, and only TypeScript files proven unused by knip.
- `plugins/toolu/skills/debug/SKILL.md`, `plugins/toolu/README.md`, `docs/{runtime,registry,install}.md`, `docs/cli/`, and `AGENTS.md`.

## Verification

Each behavior step starts with a real-input failing check and ends with that same check passing. Launcher tests inspect stdout, stderr and exit for native, fallback and missing-runtime paths. Debug tests use captured source logs and traces, not fabricated equivalents. The live-host check records actual Claude Code and Codex decisions plus gate state on macOS. OpenCode acceptance proves its coexistence path. Run the full gate under the brief's managed-job wrapper; the ledger itself acquires its shared lease.

## Delivery

Commit scoped increments after their affected checks. Before implementation and before push, fetch and rebase when `origin/main` moved; rerun stale checks. After all steps, commit the final diff, run `toolu ledger run docs/toolu/plans/2026-10-11-native-toolu-plugin.md --verify`, run `toolu-review:review`, and require `toolu ledger verdict status` to say `overall: ready`. Push only the issue branch and open a PR to `main` titled `feat(toolu): switch plugin to native binary (#425)`; its body starts with the issue and epic closing lines and states that the notice release must precede merge. Report `pr-open`, then `babysit`, and finally `ready` only after its strict success stop. Do not merge.

## Deviations

The launcher step's `paths` now includes its checked-in generator goldens and OpenCode bootstrap text, discovered while reading the protocol test before implementation. The hook-switch step now has a real committed-manifest regression test that requires all eight entries to be native and their Bun fallbacks to exist. Its paths include the coverage inventory and matrix, Bun launcher checker, OpenCode bootstrap's separate leaf command template and generated surface, and test fixes exposed by the Rust selector. The startup binary probe's 1-second bound misidentified a working, newly copied macOS binary during a real Bun subprocess check; the bound and real-binary test deadlines now cover the measured first invocation, and test sandboxes symlink the built binary to avoid repeated first-copy validation. This machine also has an older installed `toolu`, so source-checkout conformance sets `TOOLU_BIN` to this branch's built binary. These changes refine freshness and proof, not behavior or step order.

## Plan review

**Status:** Approved

All eight spec criteria map to a step, with no dangling references or dependency cycles. The docs step now tests the explicit no-Bun agent-Markdown requirement in addition to the command drift gate. Each behavior step names a real captured input or actual host invocation and an invalid or boundary result. The notice release remains an orchestrator-owned merge condition.
