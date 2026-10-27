# Code review deterministic pre-pass — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Cursor (epic worker, #664)   **Topic:** Trusted scanners before the LLM for #664

## Problem

A provider outage or model miss must not leave a diff un-reviewed for mechanical defects. Deterministic scanners have to run first as TRUSTED context: ast-grep pattern hits and secret/entropy hits on the shaped committed diff, tagged so later stages cannot dismiss them, with redaction spans so secrets never leave the machine cleartext.

## Non-Goals

1. Prompt assembly, OpenRouter, Jev judging, validate-and-anchor, verdict prose, gate-state writes, and skill docs belong to #666/#668/#669. This issue returns pre-pass data only; `toolu code-review run` stays fail-closed (`NOT_BUILT` / unconfigured).
2. No GitHub comment posting, no new CLI flags, no dependency on xtask or rule crates.
3. Applying redaction spans to outbound prompts/logs is owned by #666/#668/#669; this issue only produces the spans and masks each secret finding's own `quote`.

## Architecture

`prepass::run(env, root, shaped) -> PrepassResult` scans only `shaped.files` (committed evidence from #662). Two scanners:

1. **Ast-grep (engine path).** Depend on `toolu-engine` and `tempfile` (runtime). For each shaped file with at least one `Side::New` line, read the HEAD blob with `git cat-file blob HEAD:<path>` via `toolu_runtime::process` (same pattern as `rules.rs` / `shape`, not the worktree), write it under a temp root at the same repo-relative path (`EditedFile { path: repo_relative, absolute: temp_root.join(repo_relative), removed: false }`). Build a minimal `RuleContext` (`host: Host::Claude`, `env`, `project_root`/`config_root`/`cwd` = **temp root** so `scan_inline` argv relative paths resolve, empty `raw` map, no `edit`/`plugin_root`), and call `toolu_engine::quality::scan_inline` with an embedded review ruleset (inline YAML owned by this crate, `severity: warning` so a match does not fail the process). Map `AstGrepScan::Missing` and `Failed` to notices (never a silent skip). Keep hits whose `(file, line)` matches a shaped `Side::New` changed line; map each to a `Finding` with `source: AstGrep`, `side: New`, `category` = rule id, `severity: Medium` (engine `AstGrepHit` exposes no severity; do not invent metadata parsing). Clean up the temp directory before return.

2. **Secrets + entropy (native).** `include_str!` `tooling/conventions/guardrails/rust/rules.json` and parse only the `secrets` array (`id` + `regex`) — one on-disk source with the xtask guardrail; no table copy. Scan each shaped `Side::New` line with the compiled regexes (first match wins per line). Additionally, extract candidate tokens (runs of length ≥ 20 from `[A-Za-z0-9+/=_\-.]`) that matched no regex; if Shannon entropy of the token ≥ 4.5 bits/char, emit an entropy finding. Known-pattern hits: `severity: Blocker`, `category: secret-<id>`. Entropy hits: `severity: Medium`, `category: secret-entropy` (Jev 0.83). `source: Secrets` for both. Spans are UTF-8 byte offsets into the **unmasked** line text (`start` inclusive, `end` exclusive). The finding `quote` is that same line with each spanned region replaced by `***` (so later stages mask other payloads from `redaction_spans` + original line, never from guessing inside the already-masked quote).

**Ordering and cap.** Concatenate secrets findings then ast-grep findings. Sort each group worst-first (`severity` desc, then path, line). Enforce `PREPASS_FINDINGS_MAX` (50) in `budget.rs`: keep all secrets findings that fit, then fill remaining slots with ast-grep; if either group is truncated, set `overflow: true` and push one notice `prepass: capped at 50 findings (dropped N)`. Never silently drop.

**Trusted.** Findings carry `source: ast-grep | secrets`. Downstream (#666) must treat them as non-dismissible; this stage documents that contract in the result type.

Decisive trade-offs (Jev): `include_str` of `rules.json` over a chore(gates) extract (0.71); engine `scan_inline` over a private mirror (0.97); optional `redaction_spans` on `Finding` (0.83); Mechanical checks = findings + notices (0.92); New-side only for secrets (0.44, chosen because removals do not introduce secrets into HEAD).

## Interfaces / Schema

```rust
pub struct RedactionSpan { pub start: usize, pub end: usize } // bytes into unmasked line

// Additive on Finding (serde skip when empty):
pub redaction_spans: Vec<RedactionSpan>,

pub enum NoticeKind { MissingAstGrep, AstGrepFailed, Overflow, SecretsCompile }

pub struct Notice { pub kind: NoticeKind, pub text: String }

pub struct PrepassResult {
  pub findings: Vec<Finding>,   // source ast-grep | secrets only
  pub notices: Vec<Notice>,
  pub overflow: bool,
}

pub fn run(env: &Env, root: &Path, shaped: &Shaped) -> PrepassResult
pub fn mechanical_checks_lines(result: &PrepassResult) -> Vec<String>
```

- Mechanical checks payload (for #669 when LLM judgment is unavailable): `mechanical_checks_lines` returns the body lines for a section titled `Mechanical checks` — (1) every `findings` entry worst-first as `[{source}/{severity}] {path}:{line} {claim}`, (2) every `notices` text line, (3) if both empty, exactly one line `No mechanical findings.` — never an empty vec and never an approved-looking empty verdict by itself (verdict stays computed by `Report` from findings + coverage + error).
- JSON schema (`$defs/codeReview` finding items): add optional `redaction_spans` array of `{start, end}` integers (`minimum: 0`, `end > start` enforced in tests). `schema_version` stays 1 (additive).
- Embedded review ruleset (`include_str!` YAML beside the module). First shipped rule id `review-todo-macro`: language Rust, pattern `todo!()` or `unimplemented!()`, message "remove todo!/unimplemented! before merge", YAML `severity: warning`. Additional rules may be added in later calibration without changing this interface.
- Test seam: `run` calls an internal `scan_files(files, rules, ctx) -> AstGrepScan` that normally delegates to `toolu_engine::quality::scan_inline`. Tests override it via `run_with(env, root, shaped, scan)` so AC-3 can inject `Missing`/`Failed` without mocking crates or touching PATH races. Production `run` is thin over `run_with(..., scan_inline)`.
- `Finding.id`: stable within the run as `prepass:{source}:{path}:{line}:{category}` (memory fingerprinting / cross-round ids remain #668/#669).
- Text-fingerprint from the issue AC maps to the existing memory fingerprint inputs (`path` + `category` + normalized `claim`/`quote`); this stage fills those fields and does not add a separate fingerprint field.
- Claim strings: secrets known-pattern = `possible committed secret ({id})`; entropy = `high-entropy token looks like a secret`; ast-grep = the rule's human message (from our YAML message field, carried beside the rule id in the embedded set — not from engine hit metadata). Suggestion: `Option::None` in this slice.
- Module layout: `src/prepass.rs` re-exports `run` / `mechanical_checks_lines` / types; implementation split under `src/prepass/` (`mod.rs`, `secrets.rs`, `astgrep.rs`, `cap.rs`, `mechanical.rs`) if a single file would exceed the 300-line guardrail.

## Failure modes and edge cases

- `ast-grep` absent from PATH (`AstGrepScan::Missing`): notice `prepass: ast-grep not installed; structural rules skipped`; secrets still run; review proceeds.
- Ast-grep non-zero / timeout / parse failure (`Failed`): notice with capped stderr first line; secrets still run; no structural findings from that scan.
- Temp materialization or `git cat-file` failure for one file: skip that file with a notice; continue others.
- Pure deletion files (no New lines): skipped by both scanners.
- Old-side lines: not scanned for secrets/entropy or ast-grep anchors (New-only).
- Secrets regex compile failure (corrupt embedded JSON): one `SecretsCompile` notice, no secrets findings (programmer/data bug surfaced, not panic).
- Non-UTF-8 HEAD blob: skip file with notice (shape already UTF-8 for changed lines; blob materialization may still fail).
- Cap overflow: truncated findings omitted from `findings`, counted in the overflow notice; `overflow: true`.
- Empty `shaped.files`: empty findings, no notices.
- A secret finding's `quote` is always masked; cleartext appears only inside the scanner long enough to compute spans.

## Acceptance criteria

- **AC-1:** Given a shaped fixture whose New-side line contains a pasted GitHub token matching the shared `github-token` pattern, pre-pass emits a `source: secrets` finding with masked quote, non-empty `redaction_spans` into the unmasked line, and `category` `secret-github-token` (pattern id), plus `path`/`line`/`end_line`/`severity` filled.
- **AC-2:** Given a real temp Git repo whose HEAD blob contains a `todo!()` on a New-side changed line, production `run` (engine `scan_inline`, no test override) emits `source: ast-grep`, `category: review-todo-macro`, path/line/end_line/`severity: medium`/claim/quote grounded on that changed line — proving reuse of the engine scanner path, not a private argv mirror.
- **AC-3:** Given `run_with` injecting `AstGrepScan::Missing` or `Failed`, pre-pass still returns secrets findings (when present), includes a degrade notice naming the failure (`MissingAstGrep` / `AstGrepFailed`), and never treats Missing as a clean structural pass.
- **AC-4:** Given more than `PREPASS_FINDINGS_MAX` findings, the result has at most that many findings, `overflow: true`, an overflow notice, and prefers retaining secrets findings over ast-grep when both compete.
- **AC-5:** Given a New-side high-entropy token that matches no known regex, pre-pass emits a `source: secrets` finding with `category: secret-entropy` and `severity: medium`.
- **AC-6:** `mechanical_checks_lines(&PrepassResult) -> Vec<String>` returns a non-empty list: either rendered finding/notice lines or exactly `No mechanical findings.` when both are empty — the Mechanical checks payload #669 renders on LLM outage.
- **AC-7:** Every pre-pass finding serializes with `source` in `ast-grep|secrets` (the issue's tool tag) and with `path`, `category`, and claim/quote text sufficient for the memory fingerprint (`path + category + normalized text`); no separate fingerprint field is added.

## Acceptance evidence

- AC-1: Tests build a minimal `Shaped` with a runtime-constructed token (`format!("ghp_{}", …)` so no committed secret); `cargo test -p toolu-code-review prepass` asserts source, category, masked quote, span offsets against the unmasked line, and line fields.
- AC-2: Real Git `TempDir` (shape-test pattern): commit a Rust file with `todo!()` on a changed line, `shape` then production `run` with `ast-grep` on PATH (required in CI like other quality crates); assert `source: ast-grep`, `category: review-todo-macro`, and line binding. Boundary: the same token only on an unchanged context line does not appear.
- AC-3: `run_with` injects `Missing` and `Failed`; assert notice kind/text and continued secrets results when a secret New line is also present.
- AC-4: Synthetic findings over the cap via the capping helper; assert length, overflow flag, notice, and secrets-before-ast-grep retention.
- AC-5: Synthetic high-entropy token on a New line; assert category and severity. Boundary: a low-entropy long token does not match.
- AC-6: Unit test of `mechanical_checks_lines` for (a) findings+notices, (b) empty → exact empty line text.
- AC-7: Serialize a secrets and an ast-grep finding to JSON; schema helper asserts `source` enum and required fingerprint fields present; `redaction_spans` omitted when empty.

## Documentation impact

Internal stage only for user-facing CLI. Update this design, its plan, and the brainstorm. Additive JSON schema field on findings. No README/skill change in this issue (#669).

## Open Questions

None blocking. Deferred: expanding the embedded review ruleset beyond the minimal tested rules (owner: later calibration / #670); Old-side advisory secret hits (rejected for this slice).

## Spec review (2026-10-27)

- Fixed — Architecture: temp-root cwd for `scan_inline` relative paths; always `Medium` ast-grep severity (engine drops severity); spans defined on unmasked line text; `tempfile` runtime dep.
- Fixed — Interfaces: `run_with` scan seam, named `review-todo-macro` rule, claim strings, module split guidance, `mechanical_checks_lines`.
- Fixed — ACs: AC-2 proves production engine path; AC-3 uses injected Missing/Failed; AC-7 covers tool tag + fingerprint fields; AC-2 evidence requires ast-grep in CI.
- Kept — New-side-only secrets (Jev 0.44): removals do not introduce secrets into HEAD.
- Kept — `prefer_secrets` overflow (Jev insufficient → security default).
- Status: Approved.
