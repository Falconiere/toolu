# benchmarks

Measured token/cost deltas for toolu's efficiency mechanisms — built to replace
unsubstantiated headline claims (caveman "~75%", cavecrew "~60%") with real,
committed numbers. Honest by construction: it records what tools and models
actually return, never a fabricated counterfactual.

## Tiers

- **Deterministic (CI, hermetic):** `retrieval`. No model in the loop — compares
  full-file read bytes vs ast-grep targeted-match bytes, as tokens. Runs in CI
  with no API key; its result is committed under `results/`.
- **Live (manual, non-CI):** `caveman`, `cavecrew`, `whole-session`. Real API /
  `claude -p` runs; token counts come from real `message.usage`. Results are run
  by hand and committed with provenance (model, commit, n_runs, variance).

## Layout

```
benchmarks/
  run.sh                 entry point
  lib/                   common.sh (bootstrap), tokens.sh, result.sh
  cases/<mechanism>/     per-mechanism runner + inputs
  results/               committed result JSON + methodology (results/README.md)
  __tests__/             bats suites + fixtures
```

## Usage

```sh
benchmarks/run.sh --tier deterministic            # hermetic; writes results/retrieval-*.json
benchmarks/run.sh --tier live --mechanism caveman # manual; needs ANTHROPIC_API_KEY
benchmarks/run.sh --validate <result.json>        # schema check
```

## Reuse, not reinvention

Token math is benchmarks' own, sourced not reimplemented per-case:
`stats_usage_rollup` and `stats_pricing_jq` from `benchmarks/lib/` (`usage.sh` +
`pricing.sh`; originally written for the now-removed `stats` plugin's report,
kept here as benchmarks' single source of truth for token/cost accounting). The
deterministic tier reuses the byte-savings comparison shape from
`plugins/ast-grep`.

## Conventions

Bash, `set -u`, shellcheck-clean; one responsibility per file; tests colocated in
`__tests__/` against real data (no mocks). Design lives in the (gitignored) spec
and plan under `docs/toolu/`. Methodology contract: [`results/README.md`](results/README.md).
