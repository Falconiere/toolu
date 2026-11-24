---
name: jev-eval
description: Evaluate outputs with Jev as a cheap, calibrated judge — grade answers or diffs on Score levels, verify claims against their source, measure self-consistency, and pin thresholds for regression evals. Use for LLM-as-judge evals, answer scoring, citation checking, or benchmarking two model runs.
---

# Jev as an evaluator

`toolu jev` answers typed questions about supplied state. Keep the weights,
bands, and aggregation in code and ask Jev for each individual judgment.
See the [Jev CLI reference](../jev/SKILL.md).

## Grade one output

One Score per dimension, self-contained situation levels, lowest first:

```bash
toolu jev score "How well does answer.md resolve the question in ticket.txt?" \
  -s @state.json \
  -l "Contradicts or ignores the question's subject." \
  -l "Addresses the subject but misses what the question asks." \
  -l "Answers as asked; leaves the stated context unused." \
  -l "Answers correctly using the context; nothing material missing."
```

Read the legend and the per-level probabilities: a fractional score is not an
exact quantity, and a middle position can hide a two-peaked answer. Assert
bands, not exact values, in evals.

For many dimensions over the same state, one `ask` call bundles them
(parallel questions cost the state once). Weight and combine the answers in
code, never by asking Jev for a total.

## Verify an answer against its source

Three Nouls in one `ask`, per (answer, sources) pair:

```json
{
  "supported":  {"type": "noul", "instructions": "Do `answer`'s claims follow from `sources`?", "criteria": {"true": "The sources establish each claim as stated.", "false": "Some claim is absent from or contradicted by the sources."}},
  "contradicts": {"type": "noul", "instructions": "Does `sources` state something incompatible with a claim in `answer`?", "criteria": {"true": "A contradiction exists.", "false": "No contradiction exists."}},
  "cites":       {"type": "noul", "instructions": "Does `answer` attribute the quoted passage in `answer.quote` to `sources`?", "criteria": {"true": "The citation names this source.", "false": "The citation is missing, wrong, or points elsewhere."}}
}
```

Route on the pattern, not one number: `supported ≥0.7, contradicts ≤0.3` →
pass; a contradiction over 0.5 → human review. Doubtful answers go to review,
not auto-fail.

## Measure reliability without labels

Self-consistency: ask the same question twice over the same state and count
how often the answer changes. Run the pair as two questions with distinct ids
in one `ask`; divergence on borderline cases is the signal, stable answers are
trustworthy whatever the value.

## Calibrate and pin

- Judge a representative set (pass, fail, borderline) with `--raw` kept for
  model/usage records; set each dimension's band from observed distributions,
  separately per question.
- Pin the model (`-m jev-<version>`) once thresholds depend on it; rerun the
  labeled set on any model bump.
- `P(yes)` need not equal `1 - P(not yes)`: write `--true`/`--false` (or the
  `criteria` pair) as concrete situations and calibrate each side's band.
- A failed call (nonzero exit) is not a low score. Never read absent output
  as a judgment; record the eval as errored and retry.

Extra questions cost tokens: bundle, reuse results while state and questions
are unchanged, and slice before reaching for `--long-context`.
