---
description: Use to think a change through before building — scope, evidence, alternatives, trade-offs, and a recommended approach — without editing code. Triggers on "brainstorm", "think through", "design options", "how should we approach", "trade-offs". delivery-flow runs it as phase 1.
name: brainstorm--brainstorm
---

# Brainstorm

Choose the smallest amount of design work that removes material uncertainty.
Default-and-proceed is the baseline: do not turn routine work into an interview.

## Modes

- **Standalone** — the user invoked this skill directly. Work through the
  procedure, post the capsule, and suggest the next step (for example
  `/delivery-flow:delivery-flow` or `$delivery-flow:delivery-flow` to build it).
  Standalone brainstorm never edits code, commits, or starts delivery; the user
  decides what happens next.
- **Delivery** — delivery-flow invoked this skill as its first phase. Record the
  outcome, evidence, risks, and decisions, then hand off to `spec` in the same
  turn without waiting for confirmation.

## Triage

- **Minimal** — mechanical work with no design decision: record the requested
  outcome, existing convention, and direct check before proceeding.
- **Compact** — bounded work with a material default or a small compatibility
  risk. Produce a concise capsule, then continue.
- **Full** — use only for cross-cutting work, a public interface,
  persistence-or-migration, security-privacy, external-cost, or an unclear goal.
  Evaluate only relevant axes from
  [design-questions.md](references/design-questions.md) and compare only
  genuinely distinct alternatives.

## Evidence and decisions

Start with memory recall, one targeted structural or exact-text search, then
inspect the best hits. Reuse demonstrated repository conventions when they
settle the choice. Outside a repository, say "no repository evidence" and work
from the request. Delegate only when the search needs a broad map, on a
read-only exploration tier; keep the final trade-off decision in the main
thread on the most capable tier.

When Jev is installed, call it to compare concrete alternatives against the
user's stated preferences before deciding. When it is unavailable, say so once
and decide from explicit evidence. Jev informs the choice; it never replaces the
agent's own feasibility and architecture judgment.

Set material defaults and proceed. Ask one structured question (2–3 options)
only when prompt and repository evidence cannot settle a
goal-defining or hard-to-reverse fork. If several forks qualify, ask about the
highest-blast-radius decision and record defaults and risks for the rest. Use
the host's structured-choice tool: `AskUserQuestion` in Claude Code,
`request_user_input` in Codex when available; otherwise ask one concise plain
question.

## Capsule

- **Outcome:** the intended, observable result.
- **Material defaults/non-goal:** the chosen boundary and what stays out.
- **Repository evidence:** the recalled decision or best matching hit.
- **Risk:** the remaining compatibility, behavior, or delivery risk.
- **Handoff:** the next step — `spec` in Delivery mode, a suggested command in
  Standalone mode.

## Full path

For each relevant material axis, state the default, evidence, and risk. Compare
only alternatives that would change the outcome, interface, persistence,
security, cost, or reversibility. Use the single-question exception above, then
record the chosen approach, rejected alternatives, defaults, and open risks.

## Output

Always post the capsule in chat, on every path, so the decision is visible where
the work happens. Write `docs/toolu/brainstorms/<YYYY-MM-DD>-<slug>.md`
only when the Full path runs or the user asks for a file: the file holds the capsule
plus the Full-path axes, alternatives, and rejected options, which are worth
keeping beside the spec that follows. Create the directory when absent and
never overwrite an existing file — add a `-2` suffix instead.

## Handoff

In Delivery mode, Minimal and Compact both hand off to `spec`; Full does the
same after resolving material choices. Carry forward real-data tests, concise
docs, and any user-facing documentation updates. In Standalone mode, stop after
the capsule.
