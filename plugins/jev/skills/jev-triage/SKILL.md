---
name: jev-triage
description: Triage and route incoming items with Jev — classify issues, tickets, findings, or messages into categories with a Choice, gate severity with Score levels, ask the companion none-fits Noul, and map answers to routes in code. Use for any sort-into-buckets, assign-priority, or needs-human-review workflow.
---

# Triage with Jev

Triage is a judgment, not a computation: what kind of thing is this, how
severe, does anything fit. `toolu jev` answers those as typed questions; the
routing table stays in your script. CLI grammar: [jev](../jev/SKILL.md).

## One pass per item

Bundle the triage judgments of one item into a single `ask` — the item text
goes in the state once:

```json
{
  "category": {
    "type": "choice",
    "instructions": "Which category in `item.text` does this report belong to?",
    "criteria": {
      "crash": "Describes a failure, exception, or wrong output that already happened.",
      "performance": "Describes slowness, memory, or timeout while results stay correct.",
      "question": "Asks how or whether something is supported; reports nothing broken.",
      "other": "A support request none of the listed categories describes."
    }
  },
  "severity": {
    "type": "score",
    "instructions": "How much does `item.text` block the reporter's work?",
    "criteria": [
      "Noticed; everything they need still works.",
      "Their main path is degraded but has a workaround stated or obvious.",
      "A required path is dead; nothing else in `item.text` shows a workaround."
    ]
  },
  "any_fit": {
    "type": "noul",
    "instructions": "Does `item.text` describe a failure or wrong result, a performance problem, or a question about how or whether something is supported?",
    "criteria": {"true": "One category describes the report.", "false": "The report matches none of them."}
  }
}
```

`category` always picks, so `any_fit` is what separates a real pick from a
forced one. As of Jev 1.14 the companion Noul beats a "none-of-the-above"
option — keep `other` only when it means something concrete.

## Route on probabilities, in code

```text
probabilities[category] ≥ 0.7  → file it, act on severity band
0.3–0.7 or any_fit low         → reorder the options and ask once more
still split                    → queue for a human
```

- Riskier routes get higher thresholds: auto-labeling is cheap, auto-closing
  is not — require more certainty the further the action is irreversible.
- Severity: act on the level and its probabilities, not `confidence`.
- Ask the same question twice with the options reordered before trusting a
  Choice near the boundary: the first option is favoured.

## Batch a queue

Triage N items with N calls, never one call about N items: put one item in the
state per request so the answers cannot mix. Run the requests concurrently to
keep wall time down; each extra `ask` id is cheap, the state is the cost.

Never treat a failed call as "untriaged" or "low severity": record the error
and retry. A missing answer is not a judgment.
