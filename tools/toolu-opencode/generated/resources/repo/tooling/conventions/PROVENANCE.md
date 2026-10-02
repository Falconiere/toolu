# toolu-conventions provenance

**Upstream:** [Falconiere/toolu-conventions](https://github.com/Falconiere/toolu-conventions)\
**Pinned revision:** `3562c63eb29a05bdb3cbcae5380196043c159759` (tag path used by #213)\
**Vendored paths:** `tooling/conventions/guardrails/` (patterns, oxlint-plugin, schemas, README) and `tooling/conventions/lint/base.oxlintrc.json`.
**Ported:** the bash runner, `lib/` and `checks/` are a TypeScript port at `tooling/src/guardrails/` ([#277](https://github.com/Falconiere/toolu/issues/277)), no longer byte-identical to upstream. `tooling/fixtures/guardrails/golden.json` records the upstream bash verdicts (Linux, GNU grep) on the upstream fixture trees; `golden-parity.test.ts` replays all 229 cases.

**Deliberate differences from the bash runner** (each a bash defect, pinned by a test):
- A filename-case violation in repo mode fails the gate; bash printed it from a pipeline subshell and exited 0.
- The lint-suppressions lexer honours backslash escapes; bash compared against `'\\'`, a two-character string, so an escaped quote ended a string early.
- The repo-mode secret-content and lint-suppressions scans work on macOS; bash's `grep -Z` means "decompress" there, so both found nothing.
- ast-grep or git killed by a signal exits 3; bash reported the exit code, the port no longer mistakes "no status" for "no match".
- A config field of the wrong type, or a malformed `filenameCase.regex`, exits 3 naming the key; bash read `null` or failed inside awk.
- `|` inside a filenameCase regex or a pattern message no longer splits fields.
- Workspace packages run in one process, one after another, with output in manifest order; bash forked a child per package in parallel.

**Inherited from bash, kept for parity** (candidates for a follow-up): an unparseable `package.json` reports no banned dependency; `--only` with an unknown id runs nothing and exits 0; `--stop` treats a failing `git status` as an unchanged tree; unreadable directories are skipped by the tree walks; file-size skips unreadable files; `--only` is not forwarded to workspace packages.

## Update / drift procedure

1. Choose a new commit SHA on `Falconiere/toolu-conventions` after reviewing CORE.md + guardrails changelog.
2. Replace the vendored data trees with that revision (no live git clone in CI/hooks), and port any runner, lib or check change into `tooling/src/guardrails/` with its upstream fixture assertions.
3. Update this file's pin and the adoption table in `docs/conventions-adoption.md`.
4. Re-run `bun run test:conventions` and fix adapted configs (`.oxlintrc.json`, `tooling/guardrails.config.json`) for any new upstream checks.
5. Do **not** auto-fetch upstream at runtime — the pin in-tree is the policy source of truth.

## License note

Upstream package metadata marks the create CLI `UNLICENSED`; this repo vendors only the guardrails/lint policy artifacts needed for local gates under toolu's MIT tree. Revisit if upstream relicenses or publishes a dedicated policy package.
