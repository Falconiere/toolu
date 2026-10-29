# debug — trigger eval

A regression eval of the `debug` description's routing boundaries. These prompts
have been inspected during maintenance; they are not a held-out generalization set.

## Method

1. `cases.json` — 18 labeled prompts (12 positives that should fire `debug`, 6 negatives that belong to other skills: feature build, design review, writing new tests, mechanical refactor, research, explanation).
2. A **router judge** is given ONLY the skill's frontmatter `description` (the real selection signal — not the body) plus each prompt, with no gold labels, and predicts fire/no-fire.
3. Predictions are scored against the gold labels → precision, recall, F1 (`results.json`).

`results.json` records a single description-only smoke trial, not host activation.
For isolated repeats, baselines and evidence grading, follow
[the evaluation guide](../../../../../docs/agent-text-evals.md).

## Latest result

precision 1.0, recall 1.0, F1 1.0 (tp 12, fp 0, fn 0, tn 6) — see `results.json`.

Negatives confirm the boundary holds: feature work ("add a dark mode toggle"), design review, writing tests for new code (vs. fixing a break), refactors, version lookups, and "summarize this module" all correctly route away from `debug`. The split is deliberately clean; the value is the negative boundary against the adjacent `test` and `design-review` skills.
