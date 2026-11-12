# Point agents at outline, symbol and digest — Plan

**Date:** 2026-11-12   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-12-ast-grep-point-agents-design.md   **Topic:** nudges, savings kinds and docs for the read commands

## Evidence and approach
Inspected `crates/ast-grep/src/nudge.rs` (`shell`, `message`, `Kind`), `crates/ast-grep/src/savings.rs` (`kind`, `runs_ast_grep`), `crates/ast-grep/src/launched.rs` (`program_indexes`, `program_at`), `crates/ast-grep/src/report.rs` (`render` groups any kind), `crates/cli/tests/ast_grep_hook.rs` (real-binary pre/post hook tests driven by `fixtures/ast-grep/nudge.json` with `deviation.contains` cases), and `crates/xtask/src/context_budget/budgets.rs` (session-start 110 words; file has 94). No existing nudge fixture collides with the new classification (the only declaration grep is pipe-filtered and stays silent). Approach per the approved spec: Bash-only outline/symbol kinds with generic fallback when ast-grep is missing or opted out; `toolu` subcommand detection in `savings::kind`.

## Workstream summary
nudge kinds → savings kinds → agent docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "s1",
    "title": "nudge.rs: Outline/Symbol kinds, declaration classifier, messages with generic fallback; unit tests and real-binary fixture cases",
    "check": "cargo test -p toolu-ast-grep nudge && cargo test -p toolu-cli --test ast_grep_hook nudge",
    "ac_refs": ["AC-1", "AC-2"],
    "paths": ["crates/ast-grep/src/nudge.rs", "crates/ast-grep/src/tests/nudge_test.rs", "fixtures/ast-grep/nudge.json", "fixtures/ast-grep/golden.json", "crates/cli/tests/ast_grep_hook.rs", "crates/ast-grep/src/launched.rs"],
    "input": "commands `grep -n \"pub fn \\|pub(crate) fn \" crates/x/src/a.rs`, `rg \"fn hunk_start\"`, `rg TODO docs/`; boundaries: `rg \"fn x\" docs/` generic, `cat a.rs | grep \"pub fn main\"` silent, `grep \"impl Foo\"` structural; missing/opt-out states give the generic text"
  },
  {
    "id": "s2",
    "title": "savings.rs: record `outline` and `digest` kinds; unit test and real-binary post hook + savings report test",
    "check": "cargo test -p toolu-ast-grep savings && cargo test -p toolu-cli --test ast_grep_hook post",
    "ac_refs": ["AC-3"],
    "paths": ["crates/ast-grep/src/savings.rs", "crates/ast-grep/src/tests/savings_test.rs", "crates/ast-grep/src/report.rs", "crates/ast-grep/src/cli.rs", "fixtures/ast-grep/savings.json", "fixtures/ast-grep/golden.json", "crates/cli/tests/ast_grep_hook.rs", "crates/ast-grep/src/launched.rs"],
    "input": "post-tool payloads for `toolu ast-grep outline src/a.rs` and `toolu code-review digest` with real output text; `toolu ast-grep savings <ledger>` on the written ledger; boundary: `toolu ast-grep symbol x` records nothing"
  },
  {
    "id": "s3",
    "title": "Session-start Behaviour bullet and worker-brief Sync sentence",
    "check": "cargo xtask gate --only context-budget && cargo xtask check-markdown-cli && cargo test -p toolu-epic-orchestrator launch::brief && cargo nextest run -p toolu-cli --test epic_markdown --test golden_lifecycle",
    "ac_refs": ["AC-4"],
    "paths": ["plugins/toolu/hooks/docs/session-start.md", "fixtures/gates/lifecycle-golden.json", "crates/cli/tests/golden_lifecycle.rs", "crates/cli/tests/helpers", "plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md", "crates/xtask/src/context_budget", "crates/epic-orchestrator/src/launch", "crates/cli/tests/epic_markdown.rs"],
    "input": "the edited session-start.md (word count under 110) and worker-brief.md rendered by the brief tests"
  },
  {
    "id": "s4",
    "title": "Full quality gate over the branch",
    "check": "cargo xtask gate",
    "depends_on": ["s1", "s2", "s3"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "paths": ["crates", "plugins", "docs", "fixtures", "tooling", "Cargo.toml", "Cargo.lock"],
    "input": "the whole workspace at the committed branch head"
  }
]
```

## Critical files
- `crates/ast-grep/src/nudge.rs`, `crates/ast-grep/src/tests/nudge_test.rs`
- `crates/ast-grep/src/savings.rs`, `crates/ast-grep/src/tests/savings_test.rs`
- `fixtures/ast-grep/nudge.json`, `crates/cli/tests/ast_grep_hook.rs`
- `plugins/toolu/hooks/docs/session-start.md`
- `plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md`

## Verification
End to end through the real `toolu` binary: pre-tool hook advisories for the three AC-1 commands and the missing/opt-out fallbacks; post-tool hook writes `outline` and `digest` ledger lines that `toolu ast-grep savings` lists. Boundaries: pipe-filtered grep silent, non-code path generic, `symbol` not recorded. Docs synchronized in the session text and worker brief; context-budget and markdown CLI checks prove they fit and name real commands.

Delivery: commit per step (`feat(ast-grep): …` with an `Intent:` trailer), push the branch after green steps, run `toolu ledger run docs/toolu/plans/2026-11-12-ast-grep-point-agents.md --verify` on the final commit, code review, `toolu ledger verdict status` = ready, push, open the PR (`Closes Falconiere/toolu#862`, `Part of Falconiere/toolu#853`), then report babysit to the epic engine.

## Deviations
- s3: the session-start text is captured in `fixtures/gates/lifecycle-golden.json` (48 SessionStart cases); the first full gate failed `golden_lifecycle`. Added the bullet to those captures and the `golden_lifecycle` test to s3's check and paths.

## Plan review
- Jev step/AC alignment 0.66; checked by hand: AC-1..AC-4 each mapped, boundaries listed per step, paths cover tests and fixtures. Approved.
