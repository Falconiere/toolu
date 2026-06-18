# design-review — trigger eval

A held-out eval of whether the `design-review` skill's `description` reliably routes the right prompts to it (spec acceptance criterion 9: trigger precision/recall ≥ 0.90).

## Method

1. `cases.json` — ≥20 labeled prompts (14 positives that should fire `design-review`, 9 negatives that belong to other skills: bug review, test fixes, refactors, debugging, search, commits, UI generation).
2. A **router judge** is given ONLY the skill's frontmatter `description` (the real selection signal — not the body) plus each prompt, with no gold labels, and predicts fire/no-fire.
3. Predictions are scored against the gold labels → precision, recall, F1 (`results.json`).

This is an **LLM-judged** gate: `results.json` is a snapshot, and re-runs may vary slightly. The ledger check (`plan-ledger`) reads the committed `results.json` and asserts `precision ≥ 0.90 && recall ≥ 0.90`. To regenerate, re-run the router judge over `cases.json` prompts and re-score.

## Latest result

precision 1.0, recall 1.0, F1 1.0 (tp 14, fp 0, fn 0, tn 9) — see `results.json`.

Negatives confirm the boundary holds: bug/PR review, UI generation ("build me a pricing page", "generate a login screen"), refactors, debugging, search, and commits all correctly route away from `design-review`.
