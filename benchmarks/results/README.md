# Methodology — benchmark results

This directory holds committed benchmark results. Each `*.json` is one measured
comparison; this file is the contract for how those numbers are produced and read.
The point is honesty: numbers here are *measured*, with the baseline chosen so the
result cannot be flattered.

## Tiers

- **Deterministic** (`retrieval`): hermetic, regenerated in CI from real
  tool output. No model, no network.
- **Live** (`whole-session`): run by hand against the real API / `claude -p`,
  then committed. Non-deterministic — reported as mean±stddev over `n_runs ≥ 5`,
  never a single cherry-picked run.

## Fair baseline

A live comparison pins its baseline prompt next to the runner and commits it, so
the comparison is auditable and cannot be flattered by a deliberately weak
baseline.

## Tokenizer modes are never mixed

The deterministic tier counts static text either **exactly** (Anthropic
`count_tokens`, when a key is present) or by a **heuristic** (`bytes/4`, offline).
A single delta must use one mode for both sides — `bench_result_write` refuses to
write a result whose two sides were counted differently. If a key is set but the
API errors, the runner aborts rather than silently downgrading to the heuristic.

## Live tiers use real usage

Live mechanisms never use the heuristic. Their token counts come straight from
the model response's `message.usage` (exact, billed reality), surfaced as
`tokenizer.mode = "usage"`.

## Provenance

Every result stamps `provenance`: `model`, `date`, `commit`, `n_runs`,
`pricing_id`. Deterministic results carry `model: null` and `cost: null` (no model
to price against). Model upgrades shift numbers, so a result is only meaningful
read together with its provenance.

## Result schema

```
mechanism, tier, method, tokenizer{mode,source},
provenance{model,date,commit,n_runs,pricing_id},
baseline{label,tokens{input,output,cache_read,cache_write,total},cost},
treatment{...same...},
delta{tokens_pct,cost_pct,abs_tokens,mean,stddev},
cases[], notes
```

`delta.tokens_pct = (baseline.total − treatment.total) / baseline.total × 100`.
Whole-session results are an aggregate (toolu on vs off), **not** the sum of the
per-mechanism deltas.

`delta.mean` / `delta.stddev` describe the treatment side's run-to-run spread —
**cost (USD)** for whole-session. Live results may carry extra diagnostic keys —
`token_stats` / `cost_stats` (whole-session) — holding the full `{mean,stddev,n}`
for each measured quantity. These are informational; the required schema above is
the contract.
