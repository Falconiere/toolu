# toolu-conventions provenance

**Upstream:** [Falconiere/toolu-conventions](https://github.com/Falconiere/toolu-conventions)  
**Pinned revision:** `3562c63eb29a05bdb3cbcae5380196043c159759` (tag path used by #213)  
**Vendored paths:** `tooling/conventions/guardrails/` (patterns, oxlint-plugin, schemas, README) and `tooling/conventions/lint/base.oxlintrc.json`.
**Ported:** the bash runner, `lib/` and `checks/` are a TypeScript port at `tooling/src/guardrails/` ([#277](https://github.com/Falconiere/toolu/issues/277)), no longer byte-identical to upstream. `tooling/fixtures/guardrails/golden.json` records the upstream bash verdicts on the upstream fixture trees; `golden-parity.test.ts` replays them. The one deliberate difference: a filename-case-only violation in repo mode now fails the gate (bash lost the failure in a pipeline subshell).

## Update / drift procedure

1. Choose a new commit SHA on `Falconiere/toolu-conventions` after reviewing CORE.md + guardrails changelog.
2. Replace the vendored data trees with that revision (no live git clone in CI/hooks), and port any runner, lib or check change into `tooling/src/guardrails/` with its upstream fixture assertions.
3. Update this file's pin and the adoption table in `docs/conventions-adoption.md`.
4. Re-run `bun run test:conventions` and fix adapted configs (`.oxlintrc.json`, `tooling/guardrails.config.json`) for any new upstream checks.
5. Do **not** auto-fetch upstream at runtime — the pin in-tree is the policy source of truth.

## License note

Upstream package metadata marks the create CLI `UNLICENSED`; this repo vendors only the guardrails/lint policy artifacts needed for local gates under toolu's MIT tree. Revisit if upstream relicenses or publishes a dedicated policy package.
