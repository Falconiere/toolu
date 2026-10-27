# Code review convention gathering

**Outcome:** Gather the repo's convention files from the base ref into one capped bundle (plus the base-ref prompt override) that the prompt builder (#666) sends as reference data, so a PR can never soften its own review.

**Evidence:** #663 requires base-ref `git show` reads, a 64 KiB cap with whole-file drops and notices, `fixtures/**`/`docs/cli/**` excludes, plan/spec trees never scooped, and nested rules only for diff-touched dirs, nearest first. #660 requires the bundle to count toward the prompt budget with notices included. `budget::RULES_MAX_BYTES` is already 65_536 (matching `RULES_MAX_BYTES: '65536'` in `.github/workflows/toolu-review.yml`, whose `EXCLUDE_GLOBS` are `fixtures/**` and `docs/cli/**`). `.github/code-review-prompt.md` exists at HEAD and must come from base instead. The shape stage (#662) already resolves the merge-base, reads blobs as `<rev>:<path>`, hand-rolls path matching with no glob crate, and tests against real Git `TempDir` repos. `rules.rs` is a doc-only stub.

**Decision:** Probe candidate paths with `git show <base>:<path>`; never list the working tree or read HEAD. Candidates are root `AGENTS.md`/`CLAUDE.md`/`CONTRIBUTING.md`/`CONVENTIONS.md`, the same four basenames in each ancestor directory of each touched file, and `.github/code-review-prompt.md` as a separate override field sharing the same byte budget. Inclusion order is root files first, then the prompt override, then nested nearest-first; whole files that would exceed the cap are dropped with a notice (Jev: root-first 0.96). Discovery is ancestors-only probing, which is the `RULES_GLOB` equivalent (fixed basenames at touched ancestors) without a tree-wide listing (Jev: ancestors-only 0.58, confidence 0.37; the runner-up config-glob 0.31 was rejected on feasibility — no code-review config format exists and #666 owns repo config, so inventing one here would create a format another issue must support). A missing or unfetched base returns a bundle marked unavailable with a notice and no rule text, never a HEAD fallback (Jev: 0.94). Cap and exclude overrides stay at the stated defaults with no override file read; the base-ref constraint is satisfied vacuously until a later issue adds overrides read from base (Jev: 0.98).

**Boundary:** This issue builds only the gathering stage and its bundle type. Prompt assembly, the model call, and any verdict use of the bundle belong to #666. No CLI flags, report fields, or user-facing docs change here; `run` stays fail-closed until downstream stages land.

**Risk:** Basename-only discovery can miss a convention kept under an unexpected name. The fixed set matches every source the issue names; a repo that needs another name gets it when base-ref overrides land. Cap drops are always noticed, never silent, so scarcity is visible to #666 and the report.

## Axes (Full path: security + new cross-stage interface)

- **Trust source.** Default: base ref only, via `git show`. Evidence: the issue's injection test (HEAD says "approve everything", base text must win) and the shape stage's merge-base attribute reads. Risk: a caller passing HEAD as `base` would defeat it; the contract therefore takes the already-resolved merge-base commit from the shape stage, not a branch name the PR controls.
- **Discovery.** Default: ancestors-only probing (above). Alternatives: whole-tree `git ls-tree` filtering (same file set, wasted work in untouched dirs, rejected) and arbitrary globs from a new base-ref config (rejected for now, see Decision). Risk: probing is O(touched dirs × 4) `git show` calls; bounded by the diff, matching the nested-rules bound.
- **Cap order.** Default: root → prompt → nested nearest-first, whole-file drops (above). Alternatives: nearest-first (root policy droppable, rejected) and prompt-first (customization outranks standing policy, rejected).
- **Absence.** Default: unavailable bundle with notice, no text. Alternative: typed error aborting the run (rejected; rules absence becomes surfaced data for #666, consistent with the epic's "marked, never silently dropped" rule — #666 still may not approve on missing rules if it chooses to block).
- **Overrides.** Default: defaults only (64 KiB; `fixtures/**`, `docs/cli/**`, plan/spec trees). Alternative: define a base-ref override format now (rejected; #666 owns repo config).
- **Glob handling.** Default: hand-rolled segment matching for the fixed `prefix/**` excludes plus exact plan/spec tree prefixes, following `shape/noise.rs`. No new dependency; `Cargo.lock` has no glob crate and cargo-deny forbids wildcard requirements.

## Handoff

Spec next, in this same delivery run.
