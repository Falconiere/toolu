# Design question bank

Referenced by `brainstorm`. The dimension sweep in `SKILL.md` names the axes; this
file says what a good default on each axis looks like. Pick one, state it, proceed.
Do not open the host mapping's user-choice interface.

## How to pick a default

For each material dimension, name two or three real options internally, take the
recommended one, and say so out loud. Rules that make the difference:

- **Options are the design, not the survey.** Each option is a concrete commitment
  you could act on — "SQLite table, one row per run" beats "some persistence".
- **Span the range.** Three options that differ by 5% are not a choice. Reach for the
  minimal one, the conventional one, and the ambitious one, then cut whatever is
  genuinely dead.
- **Mutually exclusive.** If two options can both be true, they are one question
  about scope, not two rival designs.
- **Lead with your recommendation** and say why. A menu with no pick pushes the
  work back onto the user.
- **State the cost.** Not a restatement of the label — the thing that is easy to
  miss: what it forecloses, what it costs, what breaks later.

## The dimensions

Sweep all of these. Pick a default for every axis that would change the design;
skip the ones that don't apply.

### Intent & success
What does this actually accomplish, and how do we know it worked?
- Which of these is the real goal — and which is a nice-to-have you'd drop under time pressure?
- What does "done and working" look like from outside the code?
- Who is this for: you, your team, or end users? That changes the tolerance for rough edges.

### Scope boundary
What is deliberately *not* in this change?
- Does this cover just X, or X and Y?
- One-off fix, or the general mechanism?
- Is this the whole surface, or the first slice of a larger thing?

### Data & state
Where does the truth live, and who is allowed to change it?
- Source of truth — file, DB, API, in-memory, derived on read?
- Persistence: does this survive restart? A session? Forever?
- Migration: is there existing data that has to keep working?

### Interface & UX shape
What does the user or caller actually touch?
- CLI flag, config key, env var, or interactive prompt?
- What does this look like when it works — and when it's still loading?
- Naming: this becomes a public surface, so what do we call it?

### Failure behavior
The question people forget until it bites.
- On error: fail loud, fall back to a default, or retry?
- Partial results — usable, or all-or-nothing?
- What happens on the second run after a crash mid-way?

### Integration & blast radius
What else moves when this moves?
- Extend the existing mechanism, or add a parallel one?
- Which existing callers change behavior — and is that a break?
- Does this need a flag or a staged rollout, or can it just land?

### Constraints
The walls the design has to fit inside.
- Performance, size, or latency budget worth designing around?
- Dependencies: is adding one acceptable, or must this stay stdlib?
- Compatibility floor — versions, platforms, runtimes we can't drop?

### Effort & horizon
How much of the good idea do we actually buy today?
- Ship the minimal thing this session, or build the durable one over several?
- Is this a throwaway prototype, or code that will be maintained for years?
- What are you willing to leave as a TODO with a comment?

## Follow-through on a default

Picking a default often opens a fork the first pass couldn't see. Settle that
fork the same way — pick, state, proceed. Common chains:

- "persist it" → *where*, and *does it migrate?*
- "reuse the existing mechanism" → *what happens to its current callers?*
- "fail loud" → *loud to whom — logs, exit code, or a user-facing error?*
- "just the first slice" → *what is the seam, so slice two isn't a rewrite?*

Stop when no remaining fork would change the design.

## Worked example

Request: *"add caching to the API client."*

Sweep, pick, state:

| Dimension | Pick | Why |
| --- | --- | --- |
| Intent | Latency on repeat reads | The usual reason to cache an API client |
| Lifetime | Process memory, dies on restart | Avoids invalidation bugs; enough for the stated goal |
| Staleness | TTL 5m | Seconds is noisy; "always fresh" is not a cache |
| Write failure | Ignore, serve from origin | Cache is an optimization, not a dependency |
| Location / eviction | N/A | In-memory, so no disk path or LRU to choose |

Approach options are now worth writing, because they're arguing about a design
whose constraints are actually pinned down. Take the recommended one and proceed.
