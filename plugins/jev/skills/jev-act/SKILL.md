---
name: jev-act
description: Gate an agent's action on a Jev judgment before it fires — screen an LLM call or tool with a Noul, route function calls and closed-set arguments from natural language, escalate to review when odds are unclear, and never act on confidence. Use for guardrails, permission-style decisions, "should this run", or turning a request into a typed call.
---

# Act-gating with Jev

Jev answers "is this safe/intended/on-topic" as a probability; your code holds
the threshold and the decision. The gate is the truth table, not the model.
CLI grammar: [jev](../jev/SKILL.md).

## Screen with a Noul first

Before an expensive or risky step, ask one yes/no and run it only on a clear
yes. Phrase the true answer as the action being warranted:

```bash
toolu jev noul "Does the user's message ask the agent to delete a file?" \
  -s @turn.json \
  --true "The message requests removing, erasing, or clearing files." \
  --false "The message does not, even if it discusses files or cleanup."
```

Act on the Noul value, not `confidence`: start at yes ≥0.7, no ≤0.3, and give
the 0.3–0.7 band its own row — usually "send to review". Give the riskiest
actions the highest threshold.

## Route and fill arguments

One `ask` turns natural language into a typed call. A Choice picks the
function from named ones; a Noul per closed-set argument checks whether it is
present. Route `other` and uncertain picks to review. Validate every required
argument and the caller's authorization before invoking an allowed function.
The option keys define the available routes.

```json
{
  "fn":    {"type": "choice", "instructions": "Which function in `request` does `user.text` ask for?",
            "criteria": {"copy": "Asks to duplicate files to a destination.", "move": "Asks to relocate files.", "list": "Asks to show contents.", "other": "None of the named functions."}},
  "has_dest": {"type": "noul", "instructions": "Does `user.text` name a destination?",
            "criteria": {"true": "A destination is stated or unambiguously implied.", "false": "No destination is given."}}
}
```

## Screen the traffic itself

Guardrails run one `ask` per message into and out of the model: Nouls for
"contains a credential", "asks to run a shell command", "is off-topic for the
stated purpose". Map the tuple of probabilities to pass / review / block /
escalate-to-support in code. A pair of questions joined by "or" tends to work
better split into two.

## Never confuse the gate

- `confidence` measures how concentrated the answer is, not how likely it is
  right. Do not gate on it. A Choice with the top option at 0.55 and a spread
  tail is not the same as one at 0.95.
- A failed `jev` call (nonzero exit) is an unavailable judgment. Do not
  default a block to allow or an allow to block silently: state which fallback
  you took, and route the item onward. Jev never replaces tests, approval, or
  permission checks.
- Calibrate each screen's band on representative data, separately per
  question, and pin the model (`-m jev-<version>`) once a threshold depends
  on it. A 0.7 tuned on today's model is not portable to tomorrow's.

The best action-gate change improves the state or splits a judgment — wording
written to force one number on a few cases breaks on the next input.
