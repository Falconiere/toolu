# ast-grep outline — Plan

**Date:** 2026-11-11   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-11-ast-grep-outline-design.md   **Topic:** `toolu ast-grep outline` over a shared scan runner

## Evidence and approach
Inspected `crates/core/engine/src/quality/scan.rs` (`binary` :116, `parse` :181, `scan_inline` :212; callers `quality/run/batch_scan.rs`, `crates/rust-quality/src/check.rs`, `crates/python-quality` tests, `crates/core/engine/tests/quality_scan_fixture.rs`), `crates/ast-grep/src/cli.rs` (verb dispatch, `Outcome`, `Exit::Unavailable` = 69), `crates/xtask/src/layers.rs` (rule → core allowed), `crates/cli/tests/ast_grep_cli.rs` (real ast-grep test pattern), `crates/cli/tests/contract.rs` + `fixtures/commands_json.snap`, `cargo xtask docs-cli [--check]`. Probed real ast-grep 0.51.2: compact JSON carries ranges and `metaVariables.single.NAME`, no node kind; `.jsx` resolves to the javascript document; JSON ≈ 4.7× source on this repo. Approach per the approved spec: split the runner, add `toolu_engine::outline`, add the `outline` verb with a directory map, update agent text and generated docs.

## Workstream summary
scan runner split → outline core module → CLI verb and map → agent text and generated docs.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "Split quality scan runner into pub binary(env, cwd) and run_scan(env, cwd, rules, paths); scan_inline keeps its behavior",
    "check": "cargo test -p toolu-engine quality::scan && cargo test -p toolu-engine --test quality_scan_fixture && cargo nextest run -p toolu-cli --test golden_quality",
    "ac_refs": ["AC-4"],
    "paths": ["crates/core/engine/src/quality/scan.rs", "crates/core/engine/src/quality/tests/scan_test.rs", "crates/core/engine/src/quality/run/batch_scan.rs", "crates/core/engine/tests/quality_scan_fixture.rs", "crates/rust-quality/src/check.rs", "fixtures"],
    "input": "existing real-ast-grep scan tests, the shared quality scan fixture cases and the golden quality corpus; boundary: missing ast-grep, output-limit and nonzero-exit cases in those fixtures keep their results"
  },
  {
    "id": "s2",
    "title": "toolu_engine::outline: per-language rules, JSON parse, text derivation (range, signature, doc, depth, public), batching, enclosing",
    "check": "cargo test -p toolu-engine outline",
    "depends_on": ["s1"],
    "ac_refs": ["AC-1", "AC-2", "AC-6"],
    "paths": ["crates/core/engine/src/outline.rs", "crates/core/engine/src/outline", "crates/core/engine/src/tests/outline_test.rs", "crates/core/engine/src/lib.rs", "crates/core/engine/src/quality/scan.rs", "crates/core/engine/Cargo.toml"],
    "input": "temp Rust file (documented struct above #[derive], impl with a method and a multi-line where-clause method holding a local fn, a free function with a local fn), TypeScript file (JSDoc function with an inner function, decorated exported class with methods, exported const arrow, interface), Python file (decorated def with multi-line params, docstring and nested def; class with docstring and method), scanned by the real ast-grep; boundaries: locals absent, enclosing on a method line returns the method, on a blank top-level line returns None, an unsupported extension errors, missing ast-grep on PATH errors"
  },
  {
    "id": "s3",
    "title": "toolu ast-grep outline verb with directory map, caps, --json, exit 69/64/1",
    "check": "cargo test -p toolu-cli --test ast_grep_outline && cargo test -p toolu-ast-grep && cargo test -p toolu-cli --test contract",
    "depends_on": ["s2"],
    "ac_refs": ["AC-3"],
    "paths": ["crates/ast-grep/src/cli.rs", "crates/ast-grep/src/map.rs", "crates/ast-grep/src/tests", "crates/ast-grep/Cargo.toml", "crates/cli/tests/ast_grep_outline.rs", "crates/cli/tests/contract.rs", "crates/cli/tests/fixtures/commands_json.snap", "crates/core/engine/src/outline.rs", "crates/core/engine/src/outline"],
    "input": "real temp Git repo (git init, git add) with Rust, TypeScript and Python files plus an untracked file and a .gitignored file, the built toolu and the real ast-grep; boundaries: PATH holding only git gives exit 69 with the install command, an unsupported explicit file gives exit 64, a missing path gives exit 64, the ignored file is absent from the map, --json parses as one document; caps: a generated file with 151 items prints \"... 1 more items\", a directory of 151 files prints \"... 1 more files\", a file with 13 public names prints \", +1 more\", output over 12 KB ends with the truncation line"
  },
  {
    "id": "s4",
    "title": "Agent text (ast-grep skill, deep-explore agent): outline first, then read only printed ranges; regenerate docs/cli",
    "check": "cargo xtask docs-cli --check && cargo xtask check-markdown-cli",
    "depends_on": ["s3"],
    "ac_refs": ["AC-5"],
    "paths": ["plugins/ast-grep/skills/ast-grep/SKILL.md", "plugins/toolu/agents/deep-explore.md", "docs/cli", "crates/ast-grep/src/cli.rs"],
    "input": "the built toolu's commands --json and --help output; boundary: a stale docs/cli page fails --check"
  },
  {
    "id": "s5",
    "title": "Full quality gate over the branch",
    "check": "cargo xtask gate",
    "depends_on": ["s1", "s2", "s3", "s4"],
    "ac_refs": ["AC-4"],
    "paths": ["crates", "plugins", "docs", "fixtures", "tooling", "Cargo.toml", "Cargo.lock"],
    "input": "the whole workspace at the committed branch head"
  }
]
```

## Critical files
- `crates/core/engine/src/quality/scan.rs`, `crates/core/engine/src/lib.rs`
- `crates/core/engine/src/outline.rs`, `crates/core/engine/src/outline/{rules,parse,text}.rs`, `crates/core/engine/src/tests/outline_test.rs`
- `crates/ast-grep/src/cli.rs`, `crates/ast-grep/src/map.rs`, `crates/ast-grep/Cargo.toml`, `Cargo.lock`
- `crates/cli/tests/ast_grep_outline.rs`, `crates/cli/tests/fixtures/commands_json.snap` (if the contract snapshot covers the new verb)
- `plugins/ast-grep/skills/ast-grep/SKILL.md`, `plugins/toolu/agents/deep-explore.md`, `docs/cli/`

## Verification
- End to end: `toolu ast-grep outline crates/ast-grep/src/cli.rs crates/ast-grep/src` on this repository with the real ast-grep prints a bounded outline and map; byte-exact checks live in `ast_grep_outline.rs`.
- Failure and boundary checks: exit 69 without ast-grep, 64 for unsupported or missing paths, caps (150 items, 150 files, 12 names, 12 KB) exercised in tests with generated inputs.
- Quality unchanged: s1 checks.
- Docs: s4 checks; `cargo xtask gate` as one `toolu epic job` before push.
- Delivery: scoped commits, `toolu ledger run <plan> --verify`, code review, `toolu ledger verdict status` = ready, push, PR with `Closes Falconiere/toolu#857` / `Part of Falconiere/toolu#853`, then `toolu epic report babysit --pr N`.

## Deviations
- Added s5 (full gate) as the final step, as execution requires.

## Plan review
- AC coverage checked by hand: AC-1, AC-2, AC-6 → s2; AC-3 → s3; AC-4 → s1; AC-5 → s4. No dangling refs.
- Round 1 fix: s3 input now names the cap boundaries (151 items, 151 files, 13 names, 12 KB).
- Jev: ready 0.86; weakest-aspect choice `none` (0.35, low confidence, no single gap). No blockers.

**Status:** Approved
