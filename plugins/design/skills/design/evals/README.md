# design — trigger eval

A held-out eval of whether the `design` skill's `description` reliably routes the right prompts to it (target: precision/recall ≥ 0.90).

## Method

1. `cases.json` — 34 labeled prompts: 26 positives spanning the skill's full scope (evaluate: review/critique/audit; build: generate/redesign/init; refine/enhance/fix: bolder/quieter/animate/colorize/typeset/layout/clarify/harden) and 8 negatives that belong to other skills (bug/PR review, test fixes, refactors, debugging, search, commits, SQL/backend work).
2. A **router judge** is given ONLY the skill's frontmatter `description` (the real selection signal — not the body) plus each prompt, with no gold labels, and predicts fire/no-fire.
3. Predictions are scored against the gold labels → precision, recall, F1 (`results.json`).

This is an **LLM-judged** snapshot, not a gated ledger check: `results.json` records one run and re-runs may vary slightly. The Phase 1 plan gate only asserts `cases.json` is valid JSON with `n3`/`n8` relabeled and ≥20 cases. To refresh `results.json`, re-run the router judge over `cases.json` prompts and re-score.

## Scope change from the prior review-only skill

The skill is no longer review-only — it builds, refines, enhances, and fixes UI as well. So UI-generation prompts that were **negatives** under the old review-only scope (`n3` "generate a login screen", `n8` "build me a pricing page") are now **positives**. Negatives confirm the boundary still holds: backend work, bug/logic review, test fixes, refactors, debugging, search, and commits all route away.

## Latest result

precision 1.0, recall 1.0, F1 1.0 (tp 26, fp 0, fn 0, tn 8) — see `results.json`.
