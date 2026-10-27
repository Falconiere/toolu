# Code review deterministic pre-pass — Brainstorm

**Date:** 2026-10-27  
**Issue:** Falconiere/toolu#664 (Part of #660)  
**Mode:** Delivery / Full (security-privacy)

## Capsule

- **Outcome:** `prepass` runs trusted native scanners (ast-grep review ruleset + secrets/entropy over the shaped diff) before any LLM call, emits contract `Finding`s tagged `source: ast-grep | secrets` with optional redaction spans, degrades with notices on scanner failure/missing binary, and caps finding volume so TRUSTED context fits its budget. Defines the Mechanical checks payload later stages render when LLM judgment is unavailable.
- **Material defaults/non-goal:** Stage returns data only; does not call the model, write gate state, or change `toolu code-review run` from fail-closed. No GitHub posting. Pattern table stays one source with the xtask guardrail via `include_str!` of `rules.json` (no chore(gates) table move).
- **Repository evidence:** Stub `prepass.rs` (#661); `Finding`/`Source`/`Report` in `contract.rs`; `shape::Shaped`/`ChangedLine` (#662/#720); `toolu_engine::quality::scan_inline` + `AstGrepScan::{Missing,Ok,Failed}`; xtask secrets in `tooling/conventions/guardrails/rust/rules.json` (plugin may depend on engine; nothing may depend on xtask).
- **Risk:** Materializing HEAD blobs for ast-grep must stay committed-content-only; schema add for `redaction_spans` must stay additive for LLM findings; entropy false positives need a clear severity and category.
- **Handoff:** `spec`.

## Axes

### Intent and boundary
Default: implement `prepass::run` over a `&Shaped` (and env for PATH/process), return findings + notices. Out: orchestration (#666), masking application (#666/#668/#669), report prose (#669).

### Constraints (security / privacy)
Secrets never leave the machine as cleartext in later outbound payloads; this issue produces byte spans on each secret hit and masks the finding's own `quote`. Scan shaped diff lines only, fully offline.

### Integration
- **Ast-grep (Jev `engine_scan_inline` 0.97):** depend on `toolu-engine`; materialize HEAD (new-side) blobs to a temp dir as `EditedFile`s; build a minimal `RuleContext`; call `scan_inline` with an embedded review ruleset; map `Missing`/`Failed` to notices (never silent skip); filter hits to changed `Side::New` lines.
- **Secrets (Jev `include_str_rules_json` 0.71):** `include_str!` the existing `rules.json` and parse `secrets[]`; add Shannon-entropy detection on shaped line candidates; first pattern match wins per span; no xtask dependency.

### Interface
- **Spans (Jev `extend_finding_optional` 0.83):** add optional `redaction_spans: Vec<{start, end}>` (byte offsets into `quote`) on `Finding` + JSON schema; omit when empty.
- **Mechanical checks (Jev `findings_plus_notices` 0.92):** `PrepassResult { findings, notices, overflow: bool }`; #669 renders Mechanical checks from these when LLM is unavailable; never a blank verdict.
- **Volume (Jev score ~1.75 ≈ about-right for 50):** `PREPASS_FINDINGS_MAX = 50` in `budget.rs`; overflow drops lowest-severity first (or excess after sort) with an overflow notice; never silent drop.

### Failure behavior
Scanner failure / missing `ast-grep` → continue with secrets (+ any successful partial) and a notice. Regex compile failure at startup is a hard error (programmer/data bug). Empty shaped files → empty findings, no notice.

## Rejected

- Copying the secrets regex table into the crate (jscpd / issue ban).
- Mirroring `scan_inline` with a private process wrapper (duplicates engine path).
- Sidecar-only spans or re-detecting secrets later (drift; misses AC).
- First-class `mechanical_checks` on `Report` now (premature; #669 owns render).

## Jev

State: issue AC + review additions + scaffolding/shape/engine/xtask evidence.  
Answers: `include_str_rules_json` 0.71; `engine_scan_inline` 0.97; `extend_finding_optional` 0.83; `findings_plus_notices` 0.92; finding_cap score 1.75 (P(about-right)=0.70).
