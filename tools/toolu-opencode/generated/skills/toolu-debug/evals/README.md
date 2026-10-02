# debug — trigger eval

A held-out eval of whether the `debug` skill's `description` reliably routes the right prompts to it (spec acceptance criterion 1: 100% positive recall, 0 false-positives).

## Method

1. `cases.json` — 18 labeled prompts (12 positives that should fire `debug`, 6 negatives that belong to other skills: feature build, design review, writing new tests, mechanical refactor, research, explanation).
2. A **router judge** is given ONLY the skill's frontmatter `description` (the real selection signal — not the body) plus each prompt, with no gold labels, and predicts fire/no-fire.
3. Predictions are scored against the gold labels → precision, recall, F1 (`results.json`).

This is an **LLM-judged** gate: `results.json` is a snapshot, and re-runs may vary slightly. To regenerate, re-run the router judge over `cases.json` prompts (judge sees the description only) and re-score.

## Latest result

precision 1.0, recall 1.0, F1 1.0 (tp 12, fp 0, fn 0, tn 6) — see `results.json`.

Negatives confirm the boundary holds: feature work ("add a dark mode toggle"), design review, writing tests for new code (vs. fixing a break), refactors, version lookups, and "summarize this module" all correctly route away from `debug`. The split is deliberately clean; the value is the negative boundary against the adjacent `test` and `design-review` skills.
