# benchmarks

Measured token/cost deltas for toolu's efficiency mechanisms — built to replace
unsubstantiated headline claims with real, committed numbers. Honest by
construction: it records what tools and models actually return, never a
fabricated counterfactual.

## Tiers

- **Deterministic (CI, hermetic):** `retrieval`. No model in the loop — compares
  full-file read bytes vs ast-grep targeted-match bytes, as tokens. Runs in CI
  with no API key; its result is committed under `results/`.
- **Hook resources (CI, `hook-bench` job):** `bun run bench:hooks` measures max
  RSS, CPU and wall per spawn of every hook entry and gates ported Rust entries
  against `hook-budgets.json` (#410). Fixed payloads live in `cases/hooks/`, the
  Bun baselines and the Rust prototype measurement in `results/`. Budgets and
  method: [`docs/resource-budgets.md`](../docs/resource-budgets.md).
- **Live (manual, non-CI):** `whole-session`. Real API / `claude -p` runs; token
  counts come from real `message.usage`. Results are run by hand and committed
  with provenance (model, commit, n_runs, variance).

## Layout

```
benchmarks/                       data only
  cases/<mechanism>/              per-mechanism inputs (queries.tsv, tasks/, hooks/payloads.json)
  hook-budgets.json               p50 RSS/CPU budget per ported hook entry
  fixtures/                       stable test corpus + a real transcript set
  results/                        committed result JSON + methodology (results/README.md)
tooling/src/benchmarks/           the harness, TypeScript on Bun
  run.ts                          entry point (`bun run benchmarks`)
  hook-resources.ts               `bun run bench:hooks`; helpers in lib/hook-*.ts
  lib/                            root, tokens, result, pricing, usage
  cases/                          retrieval.ts, whole-session.ts (each also runnable directly)
  __tests__/                      bun tests
```

## Usage

```sh
bun run benchmarks --tier deterministic            # hermetic; writes results/retrieval-*.json
bun run benchmarks --tier live --mechanism whole-session  # manual; needs the claude CLI
bun run benchmarks --validate <result.json>        # schema check
bun run bench:hooks [--runs N] [--warmup N] [--only <plugin/entry>] [--out FILE] [--assert]
bun run tooling/src/benchmarks/cases/whole-session.ts --n 3 --model <id>  # case flags go to the case itself
bun run tooling/src/benchmarks/cases/retrieval.ts --queries <tsv> --corpus <dir>
```

## Reuse, not reinvention

Token math is benchmarks' own, imported not reimplemented per-case:
`usageRollup` and the pricing in `tooling/src/benchmarks/lib/` (`usage.ts` +
`pricing.ts`; originally written for the now-removed `stats` plugin's report,
kept here as benchmarks' single source of truth for token/cost accounting). The
deterministic tier reuses the byte-savings comparison shape from
`plugins/ast-grep`.

## Conventions

TypeScript on Bun; one responsibility per file; bun tests colocated in
`tooling/src/benchmarks/__tests__/` against real data (no mocks; the paid `claude`
CLI is the only substituted boundary, in the live-tier contract test). Design lives in the (gitignored) spec
and plan under `docs/toolu/`. Methodology contract: [`results/README.md`](results/README.md).
