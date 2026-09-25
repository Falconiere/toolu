# Design question bank

Referenced by `brainstorm` when triage selects the Full path. This is a menu of
material axes, not a universal checklist: ignore axes that cannot change the
design, state a supported default for the rest, and proceed.

## Default-and-proceed

Use repository evidence and the request to choose a default. Ask one structured
question (2–3 options) only when neither can resolve a goal-defining or
hard-to-reverse fork. When several forks qualify, ask the highest-blast-radius
one and record defaults and risks for the rest.

## Material axes

Consider only the axes that fit the work:

- **Intent and boundary:** observable outcome, non-goal, and first slice.
- **Data and state:** source of truth, persistence, migration, and recovery.
- **Interface:** public callers, names, UX, compatibility, and rollout.
- **Failure behavior:** errors, partial results, retries, and safe fallback.
- **Integration:** existing mechanisms, changed callers, and blast radius.
- **Constraints:** security, privacy, external cost, performance, dependencies,
  and supported platforms.
- **Horizon:** smallest durable implementation versus a later extension.

## Evidence order

Recall prior decisions, run one targeted structural or exact-text search, and
inspect the best hits before inventing alternatives. Delegate only for a broad
map; the main architecture tier keeps the final trade-off decision.

## Compact work

For bounded work, record Outcome, Material defaults/non-goal, Repository
evidence, Risk, and Handoff rather than sweeping these axes.
