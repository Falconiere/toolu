# ast-grep symbol — Plan

**Date:** 2026-11-12   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-12-ast-grep-symbol-design.md   **Topic:** `toolu ast-grep symbol` over git grep and `toolu_engine::outline`

## Evidence and approach
Files inspected:
- `crates/ast-grep/src/map.rs`: the `outline` verb pattern (`Env::process`, `current_dir`, `binary`, `Spec`, `Outcome`, exits).
- `crates/ast-grep/src/cli.rs`: verb registration and dispatch.
- `toolu_engine::outline`: `outline`, `enclosing`, `supported`, `MISSING`, and `Item` fields. The signature is whitespace-collapsed, and `cap` appends `...`.
- `crates/core/engine/src/outline/rules.rs:20`: the extension set.
- `crates/code-review/src/review/context.rs:148`: NUL-separated grep parsing.
- `crates/cli/tests/ast_grep_outline.rs`: the real-git, real-ast-grep test pattern, including the PATH-only-git case.
- `toolu_runtime::process::{Spec, Output}`: `max_output_bytes`, `truncated`, `stdout_bytes`.

Memory recall was unavailable because the comemory embed returned 404. The approach follows the approved spec.

## Workstream summary
symbol module (grep → classify → render) with unit tests → CLI verb and black-box tests → agent text and generated docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "crates/ast-grep/src/symbol.rs: git grep parse, definition/enclosing classification, caps and text/JSON render; unit tests",
    "check": "cargo test -p toolu-ast-grep symbol",
    "ac_refs": ["AC-1", "AC-2", "AC-4"],
    "paths": ["crates/ast-grep/src/symbol.rs", "crates/ast-grep/src/symbol", "crates/ast-grep/src/tests/symbol_test.rs", "crates/ast-grep/src/lib.rs", "crates/core/engine/src/outline.rs", "crates/core/engine/src/outline"],
    "input": "real grep output bytes (NUL fields, a path with ':', a partial last line) and real FileOutline/Item values (multi-line signature first line, signature capped with '...', a doc line inside the item range, an impl block); boundaries: 9 hits in one file prints 8 plus '... 1 more in this file', 41 hits prints 40 plus Narrow, a 130-char line is cut to 120 ending in '...'"
  },
  {
    "id": "s2",
    "title": "Wire `symbol <NAME> [PATH]...` into cli.rs; black-box tests against a real git repo and real ast-grep",
    "check": "cargo test -p toolu-cli --test ast_grep_symbol && cargo test -p toolu-ast-grep && cargo test -p toolu-cli --test contract",
    "depends_on": ["s1"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "paths": ["crates/ast-grep/src/cli.rs", "crates/ast-grep/src/symbol.rs", "crates/ast-grep/src/symbol", "crates/ast-grep/src/tests", "crates/cli/tests/ast_grep_symbol.rs", "crates/cli/tests/contract.rs", "crates/cli/tests/fixtures/commands_json.snap", "crates/core/engine/src/outline.rs", "crates/core/engine/src/outline"],
    "input": "temp git repo: src/a.rs defines `add` (doc comment) and calls nothing, src/b.rs calls it inside fn main, src/c.ts calls it inside a function, src/d.py calls it at top level, notes.md mentions it, an untracked src/e.rs calls it; a second repo with 6 files x 10 calls; boundaries: unknown name exits 1 with empty stdout, PATH holding only git prints references without `in` plus the single note and exits 0, --json parses as one document, empty name exits 64"
  },
  {
    "id": "s3",
    "title": "Agent text: ast-grep skill names `symbol`; regenerate docs/cli",
    "check": "cargo xtask docs-cli --check && cargo xtask check-markdown-cli",
    "depends_on": ["s2"],
    "ac_refs": ["AC-5"],
    "paths": ["plugins/ast-grep/skills/ast-grep/SKILL.md", "plugins/toolu/agents/deep-explore.md", "docs/cli", "crates/ast-grep/src/cli.rs"],
    "input": "the built toolu's commands --json and --help output; boundary: a stale docs/cli page fails --check"
  },
  {
    "id": "s4",
    "title": "Full quality gate over the branch",
    "check": "cargo xtask gate",
    "depends_on": ["s1", "s2", "s3"],
    "ac_refs": ["AC-5"],
    "paths": ["crates", "plugins", "docs", "fixtures", "tooling", "Cargo.toml", "Cargo.lock"],
    "input": "the whole workspace at the committed branch head"
  }
]
```

## Critical files
- `crates/ast-grep/src/symbol.rs` (and `symbol/` submodules if it passes 300 lines), `crates/ast-grep/src/tests/symbol_test.rs`, `crates/ast-grep/src/lib.rs`, `crates/ast-grep/src/cli.rs`
- `crates/cli/tests/ast_grep_symbol.rs`, `crates/cli/tests/fixtures/commands_json.snap`
- `plugins/ast-grep/skills/ast-grep/SKILL.md`, `plugins/toolu/agents/deep-explore.md`, `docs/cli/`
- Registration data if the guardrails require it (`tooling/conventions/guardrails/rust/inventory.json` for a new CLI verb)

## Verification
- End to end: `toolu ast-grep symbol outline crates` on this repository with the real ast-grep prints bounded definitions and references.
- Failure and boundary checks: exit 1 when nothing matches, exit 0 with the single note when ast-grep is absent, exit 64 for an empty name, and all caps exercised with generated inputs (s1, s2).
- Docs: s3; `cargo xtask gate` runs once as a `toolu epic job` before the push.
- Delivery: a scoped commit, `toolu ledger run <plan> --verify`, code review, `toolu ledger verdict status` = ready, the push, and a PR whose body has `Closes Falconiere/toolu#860` and `Part of Falconiere/toolu#853`, followed by `toolu epic report babysit --pr N`.

## Plan review
- s1, s2: 🟡 should-fix (fixed): `paths` lacked `toolu_engine::outline`, which both checks compile and run. Added.
- AC coverage: AC-1 → s1 and s2; AC-2 → s1 and s2; AC-3 → s2; AC-4 → s1 and s2; AC-5 → s3 and s4. No AC is left unmapped.
- Jev readiness check: 0.74. No blockers.

**Status:** Approved
