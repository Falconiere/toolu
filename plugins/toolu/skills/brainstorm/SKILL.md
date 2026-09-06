---
name: brainstorm
description: "Use BEFORE writing code or planning a build, feature, refactor, or behavior change whose shape is unsettled. Tells: \"where do I even start\", \"this feels big\", \"help me scope it\", \"think through the approach and tradeoffs\", \"before I start coding\". Skip mechanical work with no design decision and already-scoped features."
---

# Brainstorm

Choose the smallest amount of design work that removes material uncertainty.
Default-and-proceed is the baseline: do not turn routine work into an interview.

## Triage

- **Skip** — mechanical work with no design decision: make the requested rename,
  formatting change, dependency bump, or one-line correction directly.
- **Compact** — bounded work with a material default or a small compatibility
  risk. Produce a concise capsule, then continue.
- **Full** — use only for cross-cutting work, a public interface,
  persistence-or-migration, security-privacy, external-cost, or an unclear goal.
  Evaluate only relevant axes and compare only genuinely distinct alternatives.

## Evidence and decisions

Start with memory recall, one targeted structural or exact-text search, then
inspect the best hits. Reuse demonstrated repository conventions when they
settle the choice. Delegate only when the search needs a broad map; keep the
final trade-off decision in the main architecture tier.

Set material defaults and proceed. Ask one structured question (2–3 options)
only when prompt and repository evidence cannot settle a goal-defining or hard-to-reverse fork. If several forks qualify, ask about the
highest-blast-radius decision and record defaults and risks for the rest.
Use the structured-choice mapping in [host-mapping.md](../../workflows/host-mapping.md).

## Compact capsule

- **Outcome:** the intended, observable result.
- **Material defaults/non-goal:** the chosen boundary and what stays out.
- **Repository evidence:** the recalled decision or best matching hit.
- **Risk:** the remaining compatibility, behavior, or delivery risk.
- **Handoff:** straight to `plan` for bounded implementation; handoff to `spec`
  when the contract is cross-cutting or must outlive this session.

## Full path

For each relevant material axis, state the default, evidence, and risk. Compare
only alternatives that would change the outcome, interface, persistence,
security, cost, or reversibility. Use the single-question exception above, then
record the chosen approach, rejected alternatives, defaults, and open risks.

## Handoff

Skip goes to the requested mechanical work. Compact goes straight to `plan`.
Full work hands off to `spec`, then `plan`. Carry forward real-data tests,
concise docs, and any user-facing documentation updates.
