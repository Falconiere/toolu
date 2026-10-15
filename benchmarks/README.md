# benchmarks

Measured token/cost deltas for toolu's efficiency mechanisms — built to replace
unsubstantiated headline claims with real, committed numbers. Honest by
construction: it records what tools and models actually return, never a
fabricated counterfactual.

## Tiers

- **Deterministic (CI, hermetic):** `retrieval`. No model in the loop — compares
  full-file read bytes vs ast-grep targeted-match bytes, as tokens. Runs in CI
  with no API key; its result is committed under `results/`.
- **Hook resources (CI, `hook-bench` job):** `cargo xtask bench-hooks` measures max
  RSS, CPU and wall per spawn of every hook entry and, with `--assert`, gates
  them against `hook-budgets.json` (#410). Fixed payloads live in `cases/hooks/`,
  the former Bun baselines and the Rust prototype measurement in `results/`. Budgets and
  method: [`docs/resource-budgets.md`](../docs/resource-budgets.md).
- **Epic baseline (committed, derived):** `epic-baseline.json` holds the
  TypeScript watcher's epic figures (lead time, phase shares, rebase rounds,
  relaunches), derived from recorded state directories by `toolu epic
  baseline`; `toolu epic metrics` and `toolu epic summary` print them beside a
  run's measures. Method: [`docs/epic-orchestrator/README.md`](../docs/epic-orchestrator/README.md#metrics-and-budgets).
- **Statusline render (CI):** `cargo xtask bench-statusline --assert` times
  `toolu statusline render` against the payload in
  `fixtures/statusline/performance.json` and gates the warm p50 wall and max RSS.
- **Retired:** the `live` (`whole-session`) and `container` tiers went with the
  TypeScript harness (#440). `cases/whole-session/` keeps the task inputs.

## Layout

```
benchmarks/                       data only
  cases/<mechanism>/              per-mechanism inputs (queries.tsv, tasks/, hooks/payloads.json)
  hook-budgets.json               p50 RSS/CPU budget per ported hook entry
  startup-budgets.json            the `toolu --version` wall budget (`cargo xtask check-startup`)
  epic-baseline.json              the TypeScript-era epic baseline (`toolu epic baseline`)
  fixtures/                       stable test corpus + a real transcript set
  results/                        committed result JSON + methodology (results/README.md)
```

The harness is `cargo xtask`: `crates/xtask/src/bench*.rs`.

## Usage

```sh
cargo xtask bench deterministic    # hermetic; writes results/retrieval-*.json
cargo xtask bench-hooks [--runs N] [--only <plugin/hook>] [--out FILE] [--assert]
cargo xtask bench-statusline [--runs N] [--out FILE] [--assert]
```

## Reuse, not reinvention

The deterministic tier reuses the byte-savings comparison shape from
`plugins/ast-grep`. Token and cost accounting use the pricing id stamped on each
result (`pricing_id`).

## Conventions

Rust, in `crates/xtask`, with unit tests beside each module against real data (no
mocks). Design lives in the (gitignored) spec and plan under `docs/toolu/`.
Methodology contract: [`results/README.md`](results/README.md).
