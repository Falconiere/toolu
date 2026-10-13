# python-quality as a Rust rule crate — Plan

**Date:** 2026-10-12   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-12-python-quality-rust-design.md   **Topic:** port python-quality's post-edit rules into `toolu` (#427)

## Evidence and approach

Inspected: the TypeScript module and rules (`plugins/python-quality/hooks/src/`), the golden corpus (`fixtures/quality/python.json`, 96 cases, `python-golden.json`), #426's crate (`crates/ts-quality/src/{rule,register,hooks,lib}.rs`), its CLI routing (`crates/cli/src/hook/plugins.rs`), its conformance move (`tools/toolu-conformance/src/ts-quality/`, `harness/pretool.ts` `NATIVE_REGISTER`), its OpenCode and tooling consumer edits, the shared runner (`crates/core/engine/src/quality/`), and the Rust helpers already ported (`detect_python`, `count_python_code_lines`, `tool_available`, `QualityLang::Python`, `quality_threshold`, `quality_flag`, `toolu_runtime::atomic::write_atomic`). Every python-quality consumer was listed with `git grep python-quality`.

Approach per the approved spec: first extract ts-quality's register/bridge/check-toolu into `toolu_engine::quality::plugin::QualityPlugin` with ts-quality unchanged in behaviour; then the python rule crate; then wiring, conformance move, plugin cleanup and consumers, docs, gate.

## Workstream summary

shared `QualityPlugin` (ts-quality green) → python rule crate → hub/CLI wiring and register → conformance golden replay (native + Bun bridge) → plugin cleanup and consumers (OpenCode, tooling, bench, engine bridge test) → docs and generated references → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "shared-plugin",
    "title": "toolu_engine::quality::plugin::QualityPlugin (register, bridge_script, check_toolu) extracted from ts-quality; ts-quality delegates; CLI routes quality plugins through one table",
    "check": "cargo test -p toolu-engine quality && cargo test -p toolu-ts-quality && cargo build -p toolu-cli && cargo test -p toolu-cli --test ts_quality_hook",
    "ac_refs": ["AC-7", "AC-11"],
    "paths": ["crates/core/engine/", "crates/ts-quality/", "crates/toolu/src/", "crates/cli/src/", "crates/cli/tests/ts_quality_hook.rs", "crates/cli/tests/helpers/", "Cargo.lock"],
    "input": "ts-quality register in a real temp config dir seeded with the old .js; a Codex home without toolu@toolu",
    "model": "inherit"
  },
  {
    "id": "rule-crate",
    "title": "crates/python-quality: PyFile model, size/layout/suppression/no-mocks/docs rules, PythonQuality (Rule + QualityRule), its QualityPlugin, Guide namespace, unit tests per module",
    "check": "cargo test -p toolu-python-quality",
    "ac_refs": ["AC-1", "AC-3", "AC-5"],
    "depends_on": ["shared-plugin"],
    "paths": ["crates/python-quality/", "crates/core/engine/src/quality/", "Cargo.lock"],
    "input": "Python sources from fixtures/quality/python.json cases written to temp project dirs",
    "model": "inherit"
  },
  {
    "id": "wiring",
    "title": "Hub RULES and re-export, `toolu python-quality hook register|check-toolu|post-tools`, CLI integration tests",
    "check": "cargo build -p toolu-cli && cargo test -p toolu-hub && cargo test -p toolu-cli --test python_quality_hook",
    "ac_refs": ["AC-2", "AC-3", "AC-5", "AC-10", "AC-11"],
    "depends_on": ["rule-crate"],
    "paths": ["crates/toolu/", "crates/cli/src/", "crates/cli/tests/python_quality_hook.rs", "crates/cli/tests/helpers/", "crates/python-quality/", "plugins/python-quality/"],
    "input": "real config dir seeded with the old .js: register twice, then a bare-except .py edit through `toolu hook post-tools`; .ts/.md edits with recording PATH stubs; uninstalled plugin; doctor --json; Codex home without toolu@toolu",
    "model": "inherit"
  },
  {
    "id": "golden-replay",
    "title": "Golden suite moved to tools/toolu-conformance/src/python-quality, registered by the built toolu, every case on the native and the Bun-bridge legs; register and read-failure suites",
    "check": "cargo build -p toolu-cli && bun test --timeout 120000 tools/toolu-conformance/src/python-quality tools/toolu-conformance/src/ts-quality",
    "ac_refs": ["AC-1", "AC-4", "AC-7"],
    "depends_on": ["wiring"],
    "paths": ["tools/toolu-conformance/src/python-quality/", "tools/toolu-conformance/src/ts-quality/", "tools/toolu-conformance/src/harness/", "fixtures/quality/python.json", "fixtures/quality/python-golden.json", "crates/python-quality/", "crates/core/engine/src/", "crates/toolu/src/", "crates/cli/src/", "plugins/python-quality/hooks/hooks.json"],
    "input": "fixtures/quality/python.json and python-golden.json",
    "model": "inherit"
  },
  {
    "id": "plugin-cleanup",
    "title": "Native hooks.json, delete hooks/src and hooks/dist, delete the engine quality_bridge test, make the OpenCode startup failure a hand-written register command, update OpenCode tests, host-paths, bench fixture, knip/oxlint/package references, delete python-quality-latency bench",
    "check": "cargo xtask gate --only hooks --only dist && cargo test -p toolu-engine --test dispatch_fixture && bun test --timeout 120000 tools/toolu-opencode tooling/src tools/toolu-conformance && bun run test:conformance && bun run smoke:opencode-entry entry.startup-failure",
    "ac_refs": ["AC-1", "AC-2", "AC-6", "AC-9"],
    "depends_on": ["golden-replay"],
    "paths": ["plugins/python-quality/", "crates/core/engine/tests/", "packages/toolu-core/src/dispatch/__tests__/", "tools/toolu-conformance/src/", "tools/toolu-opencode/", "tooling/src/", "benchmarks/", "knip.json", "plugins/.oxlintrc.json", "package.json", "fixtures/gates/"],
    "input": "OpenCode adapter with a real registry dir and the built toolu; rust-quality's committed bundle for the engine bridge test",
    "model": "inherit"
  },
  {
    "id": "bench",
    "title": "Hook bench with python-quality registered natively: toolu/post-tools on a README.md edit",
    "check": "cargo build --release -p toolu-cli && bun run bench:hooks",
    "ac_refs": ["AC-3"],
    "depends_on": ["plugin-cleanup"],
    "paths": ["tooling/src/benchmarks/", "benchmarks/", "tools/toolu-conformance/src/harness/pretool.ts", "crates/python-quality/"],
    "input": "benchmarks/cases/hooks/payloads.json toolu/post-tools (Edit README.md) in the bench sandbox",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Generated CLI reference and snapshot, plugin and registry docs, root README, fixtures index, gate-coverage inventory, AGENTS.md, OpenCode generated mirror",
    "check": "cargo xtask gate --only docs-cli --only cli-compat && cargo xtask check-markdown-cli && cargo test -p toolu-cli --test contract",
    "ac_refs": ["AC-6"],
    "depends_on": ["plugin-cleanup"],
    "paths": ["docs/", "README.md", "plugins/python-quality/README.md", "fixtures/index.json", "fixtures/README.md", "fixtures/gate-coverage/", "AGENTS.md", "crates/cli/tests/fixtures/commands_json.snap", "tools/toolu-opencode/generated/"],
    "input": "the built binary's help tree",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full quality gate",
    "check": "cargo xtask gate",
    "ac_refs": ["AC-8"],
    "depends_on": ["docs", "bench"],
    "input": "whole repository",
    "model": "inherit"
  }
]
```

## Critical files

- Create: `crates/core/engine/src/quality/plugin.rs` and `tests/plugin_test.rs`; `crates/python-quality/src/{file.rs,size_rules.rs,layout_rules.rs,suppression.rs,no_mocks.rs,docs.rs,rule.rs}` and `src/tests/*_test.rs`; `crates/cli/tests/python_quality_hook.rs`; `tools/toolu-conformance/src/python-quality/` (moved harness and tests).
- Modify: `crates/core/engine/{Cargo.toml,src/quality.rs}`, `crates/ts-quality/{Cargo.toml,src/lib.rs}` (delete `register.rs`, `hooks.rs` and their tests, moved into the engine), `crates/python-quality/{Cargo.toml,src/lib.rs}`, `crates/toolu/src/{tool_hook.rs,lib.rs}`, `crates/toolu/Cargo.toml`, `crates/cli/src/hook/plugins.rs`, `plugins/python-quality/{hooks/hooks.json,README.md}`, `crates/core/engine/tests/quality_bridge.rs` and `packages/toolu-core/src/dispatch/__tests__/dispatch-cli.ts` (deleted), `tools/toolu-conformance/src/harness/pretool.ts`, OpenCode tests, `tooling/src/opencode-host/scenarios-startup.ts`, `tooling/src/__tests__/host-paths.test.ts`, docs listed in the spec.
- Delete: `plugins/python-quality/hooks/src/`, `plugins/python-quality/hooks/dist/`, `tooling/src/benchmarks/python-quality-latency.ts`.

## Verification

End to end: the 96-case golden corpus on Claude Code and Codex through the native hook and through the Bun bridge; the register migration in a real config dir followed by a violating edit; doctor's view of the manifest; OpenCode's adapter with the built binary; ts-quality's suites unchanged after the extraction; `cargo xtask gate`. Boundaries: non-Python edits spawn nothing, uninstalled plugin silent, ast-grep missing/crashing, unreadable file, moved/deleted file, no `python3`. Documentation: generated `docs/cli/`, plugin README, `docs/python-quality/README.md`, `docs/registry.md`, root README, fixtures index, AGENTS.md.

Delivery: commit after each green step (`wip:` allowed) and push `feat/427-python-quality-as-a-rust`; full gate through the epic `job.ts`; then `plan-ledger.js run <plan> --verify`, a toolu-review pass, the PR (`Closes Falconiere/toolu#427`, `Part of Falconiere/toolu#402`; first stacked on #426, rebased onto `origin/main` once PR 515 merged) and the babysit handoff. The PR body reports the `bun run bench:hooks` `toolu/post-tools` row.
