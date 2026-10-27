# code-review scaffolding — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Claude (epic worker, #661)   **Topic:** crate, namespace, contract, plugin assets and registration for the native `code-review` plugin (epic #660)

## Problem

Epic #660 builds a native `code-review` plugin in eight later issues (#662–#666, #668–#670). They run in parallel and need one mergeable target that already holds: the crate, the `toolu code-review` namespace, the JSON output contract, the memory record types, the budget constants, and every registration point. `check-cli-compat` pins documented flags and values. If the first PR left out `--effort` or one of its values, a later issue could not add it back without a protocol bump.

## Non-Goals

1. No review pipeline: diff shaping, conventions, pre-pass, OpenRouter, Jev and validation belong to #662–#668.
2. No skill and no hook. The issue makes hooks conditional ("Hooks, only if the epic's SessionStart line ships … Without a hook the plugin follows the skill-only `brainstorm` layout, and the AC's 'Claude/Muse hook entries generated' does not apply"); #669 owns the thin skill and any SessionStart line (Jev: skill later 0.68, hook now 0.08). The plugin follows the hookless `brainstorm` layout, so the issue's "Claude/Muse hook entries generated" item does not apply.
3. No `toolu.config.json` read. #666 reads the `effort` setting and the per-repo opt-in. The repository has no JSON schema for that file, only `docs/config.md`, which #666 updates when the key is read.
4. No push-review state write, and no state file of any kind (#669).

## Architecture

The new crate `crates/code-review` (package `toolu-code-review`, `PLUGIN = "code-review"`) follows the `crates/jev` shape: `cli` holds the clap tree, and `lib.rs` holds `PLUGIN`, `command()` and `run()`. It is a plugin crate, so it depends on core only (`toolu-runtime`, `toolu-protocol`, `serde`, `serde_json`, `clap`). `layers.json` does not change because the layer is not new.

Modules:
- `contract`: the JSON report: `Report`, `Finding`, `Severity`, `Side`, `Source`, `Verdict`, `Coverage`, `Usage` and `ErrorKind`. It also maps a report to `Exit` and holds `SCHEMA_VERSION`.
- `memory`: the record types `PriorFinding` (`fingerprint`, `id`, `severity`, `status`), `Dismissal` (`id`, `reason`, `by`) and `Pass` (`diff_sha`, `verdict`, `counts`).
- `budget`: every byte and token cap in one place, plus the per-effort table. It also defines `Effort` (`auto | small | medium | high | xhigh`) and its `medium` fallback.
- `shape`, `rules`, `prepass`, `review`, `validate`, `report`: one doc-only module per stage. Each has its `mod` line now, so #662–#665 and #668–#669 each edit only their own file.

`toolu code-review run [--base REF] [--path PATH]... [--effort LEVEL]` is the one verb (Jev 0.83 over flags on the bare namespace or a `planned` placeholder). Until the pipeline lands, it fails closed (Jev 0.75): it emits a `Report` with `verdict: "error"`, `error.kind: "unconfigured"` and a message saying the pipeline is not built yet, sends nothing, writes nothing and exits 69.

Schema: a `codeReview` `$def` in `crates/cli/src/commands.schema.json` (the docs copy matches). This reuses the repository convention that every `--json` document validates against its `$def` (`crates/cli/tests/helpers/schema.rs`). `crates/cli/tests/helpers/code_review_schema.rs` validates both the binary's document and full reports built through the contract, using the one existing validator.

Registration points (each from the issue's review additions):
- Root `Cargo.toml` members, `Cargo.lock` (updated before `cargo xtask docs-cli`, which builds `--locked`), `crates/cli/Cargo.toml` dependency, `folders.json` `crates` entry; `layers.json` unchanged (plugin crate, no new layer).
- `crates/cli/src/registry.rs` `verb("code-review", …)`, additive; `crates/cli/src/tests/tree_test.rs` and `crates/cli/tests/contract.rs` namespace order; `crates/cli/tests/fixtures/commands_json.snap` regenerated; `crates/cli/tests/helpers/inventory.rs` plugin count 12 → 13; `crates/cli/tests/helpers/schema.rs` routes the new namespace to `codeReview`.
- `crates/xtask/src/packaging_assets.rs` `PLUGINS` 12 → 13. `SKILLS` and `HOOKS` unchanged and the `context_budget.rs` `SKILLS` entry not added, because this PR ships no skill and no hook; #669 adds the skill, its count and its budget entry together (a budget entry for a missing `SKILL.md` would fail the check).
- `plugins/code-review/` with `README.md` and `.claude-plugin`, `.codex-plugin`, `.muse-plugin` `plugin.json` (`hookProtocol: 2` on Claude and Codex only; descriptions byte-identical); `.claude-plugin/marketplace.json` and `.agents/plugins/marketplace.json` entries; three `release-please-config.json` `extra-files` rows.
- `crates/toolu/src/doctor/tools.rs`: `code-review` requires `git`.
- `README.md` (count, group table, tree) and `docs/plugins/index.md`.

Trade-off: a real verb now, rather than a placeholder, means the scaffold must define fail-closed behaviour. That is what lets the `--effort` values be pinned in this PR.

## Interfaces / Schema

```text
toolu code-review run [--base <REF>] [--path <PATH>]... [--effort auto|small|medium|high|xhigh]
  --effort default: auto
```

Report (`toolu --json code-review run`):

```json
{
  "schema_version": 1,
  "toolu_version": "8.5.0",
  "verdict": "approved | blocked | error",
  "effort": "auto | small | medium | high | xhigh",
  "findings": [{
    "id": "f1", "path": "src/a.rs", "line": 10, "end_line": 12, "side": "new | old",
    "severity": "blocker | high | medium | low | nit", "category": "correctness",
    "source": "llm | ast-grep | secrets", "claim": "...", "quote": "...", "suggestion": "... (optional)"
  }],
  "coverage": {"reviewed": ["path"], "unreviewed": [{"path": "p", "reason": "r"}], "skipped": [{"path": "p", "reason": "r"}]},
  "usage": {"tokens_in": 0, "tokens_out": 0, "cost_usd": 0.0, "model": null},
  "error": {"kind": "unconfigured | outage", "message": "..."}
}
```

- Versions: `schema_version` (the contract, bumped on a breaking change) and `toolu_version` (the binary's `CARGO_PKG_VERSION`) let an agent tell which contract and build produced the document.
- Memory records (serde, snake_case, used by #666/#668 and persisted by #669): `PriorFinding {fingerprint: string, id: string, severity, status: "open" | "resolved" | "dismissed"}`, where `fingerprint` is `path + category + normalized text` (normalization owned by #668); `Dismissal {id, reason, by}`; `Pass {diff_sha, verdict, counts: {blocking, advisory}}`. A crate test round-trips each through JSON.
- `side: "old"`: `line`/`end_line` are old-side line numbers. Use it for removal-only hunks and deleted files (#662's anchor rule). `"new"` is the default.
- Blocking severities are `blocker`, `high` and `medium` (`Severity::blocking`). `low` and `nit` are advisory.
- Construction: `Report::reviewed(effort, findings, coverage, usage)` computes the verdict: `blocked` when a finding blocks or any path is unreviewed, otherwise `approved`. `Report::failed(effort, kind, message)` gives verdict `error`. The verdict is never taken from the model.
- `cost_usd` must be finite and non-negative. Otherwise serialisation fails, and `--json` exits 1 rather than print `null`.
- Exit: `approved` → 0, `blocked` → 2 (`Exit::Blocked`), `error` → 69 (`Exit::Unavailable`, for both `unconfigured` and `outage`). Bad flags → 64 (clap usage).
- `error` is `null` unless the verdict is `error`.

Budget constants (`budget.rs`): `RULES_MAX_BYTES = 65_536`, `CHECKLIST_MAX_BYTES = 4_506`, `JEV_STATE_MAX_BYTES = 90_000`, `JEV_REQUEST_MAX_BYTES = 180_000`, `HTTP_RESPONSE_MAX_BYTES = 1_048_576`, and a per-effort `Budget {source_file_bytes, source_package_bytes, judge, reasoning, max_tokens, deadline_secs}`:

| level | file/package | judge | reasoning | max_tokens | deadline |
| --- | --- | --- | --- | --- | --- |
| small | 8/16 KiB | blocking only | low | 16_384 | 180 s |
| medium | 16/32 KiB | all + recheck | medium | 32_768 | 360 s |
| high | 32/64 KiB | all + recheck | high | 65_536 | 720 s |
| xhigh | 64/128 KiB | all + recheck + context | xhigh | 131_072 | 1200 s |

`Effort::Auto` resolves to `medium` for budgeting (`Effort::budget`, through `Effort::FALLBACK`) until #666's selector runs.

## Failure modes and edge cases

- Unknown `--effort` value, or an unknown flag: clap usage error, exit 64, nothing on stdout.
- `run` with any arguments, before the pipeline exists: exit 69 with the `unconfigured` report. It never reports `approved`, never makes a network call and never writes state.
- A missing verb (`toolu code-review`): help is shown, and it is a usage error, as in the other verb namespaces.
- Serialising a report with no suggestion omits `suggestion`. A finding without `side` in input is not accepted here, because there is no parsing yet.

## Acceptance criteria

- **AC-1:** `cargo test -p toolu-code-review` passes. The tests check that the crate dir equals `PLUGIN`, the blocking severities, the exit mapping, that the effort table is monotonic, the computed verdict (blocking finding or unreviewed coverage gives `blocked`), the cost guard, and the memory round-trip.
- **AC-2:** `toolu --json code-review run --effort high` exits 69 and prints one JSON document. That document validates against `$defs/codeReview`, with `verdict: "error"`, `error.kind: "unconfigured"` and `effort: "high"`. Stderr is empty.
- **AC-3:** `toolu code-review run --effort huge` exits 64.
- **AC-4:** The `toolu code-review --help` output matches the committed snapshot `docs/cli/code-review.md`, generated from the binary and compared by `cargo xtask docs-cli --check`. The `docs/cli/commands.json` contract lists `code-review run` with all five `--effort` values.
- **AC-5:** All registration points are in place, and these pass: `cargo xtask check-hooks`, `check-markdown-cli`, `check-cli-compat`, `packaging`, `context-budget`, the toolu-cli contract tests (13 plugins) and `cargo xtask gate`.

## Acceptance evidence

- AC-1: `cargo test -p toolu-code-review`. The real input is a hand-built report with blocking and advisory findings and an old-side anchor. Boundaries: `medium` blocks and `low` does not; a missing `suggestion` is omitted.
- AC-2 and AC-3: `cargo test -p toolu-cli --test code_review_cli` runs the built binary. The boundary is a bad effort value.
- AC-4: `cargo xtask docs-cli --check` and `cargo xtask check-cli-compat`. AC-2's full-report schema cases run in `cargo test -p toolu-cli --test contract` (`code_review_schema`).
- AC-5: `cargo test -p toolu-cli --test contract`, `cargo xtask packaging`, `cargo xtask check-hooks`, `cargo xtask check-markdown-cli`, then `cargo xtask gate` (run in the container, per the local gate notes).

## Documentation impact

- New `plugins/code-review/README.md` (from the template), plus `docs/cli/code-review.md` and `docs/cli/commands.json`, generated by `cargo xtask docs-cli`.
- `README.md` (plugin count, group table, tree) and `docs/plugins/index.md` gain the plugin.
- The marketplace entries, and the `release-please-config.json` extra-files.

## Open Questions

- Config key name and docs for `effort` and the opt-in. Owner: #666. Non-blocking.
- The exact wording of `CHECKLIST_MAX_BYTES` (about 4.4 KiB). It is calibrated by #670, and the starting value is the issue's figure. Non-blocking.

## Spec review

- Coverage (Jev noul 0.10, then 0.23 after fixes). Rounds 1–2 found gaps that are now fixed: an explicit registration list, the doctor row, the SKILLS/context-budget deferral, the `toolu_version` field, and memory field shapes. Jev's last gap pick (hooks, 0.64) is the issue's own conditional item. With no SessionStart line in this PR, the issue states that the hook AC does not apply, so it stays deferred to #669.
- Config deferral to #666 (Jev 0.73): no `toolu.config.json` schema file exists, and #666 reads the setting.
- Every AC names a real input, a boundary and a runnable check. The open questions are owned and non-blocking.

## Code review (xhigh, 2026-10-27)

- Fixed:
  - `$defs/codeReview` now ties `error` to `verdict: error` (if/then/else).
  - `Effort::Auto` budgets through `Effort::FALLBACK`.
  - The status-name test is exact.
  - The README no longer promises a SessionStart path.
  - `Report::reviewed` computes the verdict (Jev 0.65).
  - The `cost_usd` guard.
  - The schema validation moved into the CLI's existing helper, removing the duplicate validator and the `jsonschema` dev-dependency.
  - The insta help snapshots are dropped, because the generated `docs/cli/code-review.md` is the pinned snapshot (Jev 0.82).
  - The plugin description now says the pipeline is in development (Jev 0.65).
  - The issue list in the fail-closed message is exact.
- Kept:
  - Outage exits 69, as the issue's exit table and the epic decide (Jev 0.07 for changing it to 75).
  - The `code-review` name, which the issue mandates.
  - The `wip:` commit subject, which the squash merge replaces.
