# code-review digest — Plan

**Date:** 2026-11-12   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-12-code-review-digest-design.md   **Topic:** `toolu code-review digest` over the review's diff shaping

## Evidence and approach
Inspected `crates/code-review/src/shape/snapshot.rs` (`review` :14, conflict refusal in `capture`), `crates/code-review/src/shape.rs` (`Shaped`, `ShapedFile`, `resolve_base`, `ShapeError::EmptyDiff`), `crates/code-review/src/pipeline.rs` (`scope` :152, `project_root`, `review_shape`), `crates/code-review/src/review/window.rs` (`centers` :41), `crates/code-review/src/review/context.rs` (`DEFINITION` :21), `crates/core/engine/src/outline.rs` (`outline`, `enclosing`, `supported`, `MISSING`, from #866), `crates/code-review/src/{cli,lib}.rs`, test helpers in `crates/code-review/src/tests/shape_test.rs`, and `crates/cli/tests/ast_grep_outline.rs` (real binary + git + ast-grep pattern). Comemory recall was unavailable (embedding endpoint 404). Approach per the approved spec.

## Workstream summary
visibility of reused helpers → digest data and caps with unit tests → CLI verb and black-box test → agent text and generated docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "digest.rs + digest/render.rs: build capped Digest from Shaped (counts, outline items, (new), removed names, hunks with range left, 6 KB budget, 80-file cap) and render text; make scope, centers, DEFINITION pub(crate)",
    "check": "cargo test -p toolu-code-review digest",
    "ac_refs": ["AC-3"],
    "paths": ["crates/code-review/src/digest.rs", "crates/code-review/src/digest", "crates/code-review/Cargo.toml", "crates/code-review/src/lib.rs", "crates/code-review/src/pipeline.rs", "crates/code-review/src/review.rs", "crates/code-review/src/review/window.rs", "crates/code-review/src/review/context.rs", "crates/code-review/src/shape.rs", "crates/code-review/src/shape", "crates/core/engine/src/outline.rs"],
    "input": "temp Git repo shaped by the real shape::shape: a 26-line added Rust function, a file with four separate edits, enough files of ~1 KB hunks to pass 6 KB; real ast-grep for items; boundaries: hunk cut prints '... 18 more lines: read new 9-26', fourth hunk prints '... 1 more hunks', budget switch prints the --path hint, an item wholly added is (new)"
  },
  {
    "id": "s2",
    "title": "digest verb in cli.rs and lib.rs dispatch: --since, --path (scope + skipped/unread filter), --json, No changes against <base>, error mapping",
    "check": "cargo test -p toolu-cli --test code_review_digest && cargo test -p toolu-code-review && cargo test -p toolu-cli --test code_review_cli",
    "depends_on": ["s1"],
    "ac_refs": ["AC-1", "AC-2", "AC-4"],
    "paths": ["crates/code-review/src", "crates/cli/tests/code_review_digest.rs", "crates/cli/tests/code_review_cli.rs", "crates/core/engine/src/outline.rs"],
    "input": "real temp Git repo: main with a base commit, feature branch with one commit, then an uncommitted edit, an untracked .rs file and a Cargo.lock edit; the built toolu and the real ast-grep; boundaries: --since HEAD shows only local files, --path narrows (no Cargo.lock), clean branch prints 'No changes against main', PATH holding only git prints 'items unavailable', --json parses as one document"
  },
  {
    "id": "s3",
    "title": "git-better skill: why from commits, what from the digest; regenerate docs/cli and the CLI contract snapshot",
    "check": "cargo xtask docs-cli --check && cargo xtask check-markdown-cli && cargo test -p toolu-cli --test contract",
    "depends_on": ["s2"],
    "ac_refs": ["AC-5"],
    "paths": ["plugins/git-better/skills/git-better/SKILL.md", "docs/cli", "crates/code-review/src/cli.rs", "crates/cli/tests/contract.rs", "crates/cli/tests/fixtures/commands_json.snap"],
    "input": "the built toolu's commands --json and --help output; boundary: a stale docs/cli page fails --check"
  },
  {
    "id": "s4",
    "title": "Full quality gate over the branch",
    "check": "cargo xtask gate",
    "depends_on": ["s1", "s2", "s3"],
    "ac_refs": ["AC-4"],
    "paths": ["crates", "plugins", "docs", "fixtures", "tooling", "Cargo.toml", "Cargo.lock"],
    "input": "the whole workspace at the committed branch head"
  }
]
```

## Critical files
- `crates/code-review/src/digest.rs`, `crates/code-review/src/digest/render.rs`, `crates/code-review/src/digest/tests/`
- `crates/code-review/src/cli.rs`, `crates/code-review/src/lib.rs`, `crates/code-review/src/pipeline.rs`, `crates/code-review/src/review.rs`, `crates/code-review/src/review/{window,context}.rs`
- `crates/cli/tests/code_review_digest.rs`, `crates/cli/tests/fixtures/commands_json.snap`
- `plugins/git-better/skills/git-better/SKILL.md`, `docs/cli/`

## Verification
- End to end: `toolu code-review digest` on this branch prints its own changes by file and item.
- Failure and boundary checks: clean branch, `--since HEAD`, `--path`, no ast-grep, hunk/line/budget caps (s1, s2).
- `code-review run` unchanged: `cargo test -p toolu-code-review` (s2).
- Docs: s3; `cargo xtask gate` as one `toolu epic job` before push.
- Delivery: scoped commits, `toolu ledger run <plan> --verify`, code review, `toolu ledger verdict status` = ready, push, PR with `Closes Falconiere/toolu#861` / `Part of Falconiere/toolu#853`, then `toolu epic report babysit --pr N`.

## Plan review
- AC coverage checked by hand: AC-3 → s1; AC-1, AC-2, AC-4 → s2; AC-5 → s3; AC-4 → s4. No dangling refs.
- Round 1 fix: s1 paths add `crates/code-review/Cargo.toml`.
- Jev: ready 0.85. No blockers.

**Status:** Approved
