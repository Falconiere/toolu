# Design question bank

Referenced by `brainstorm`. The dimension sweep in `SKILL.md` names the axes; this
file says what a good question on each axis looks like, and how to turn one into
options worth choosing between.

## How to ask

Use the `AskUserQuestion` tool, not prose. Prose questions get one skimmed answer;
options get a decision. Rules that make the difference:

- **Options are the design, not the survey.** Each option is a concrete commitment
  the user could act on — "SQLite table, one row per run" beats "some persistence".
- **Span the range.** Three options that differ by 5% waste the ask. Reach for the
  minimal one, the conventional one, and the ambitious one, then cut whatever is
  genuinely dead.
- **Mutually exclusive**, unless you set `multiSelect: true`. If two options can
  both be true, they are one question about scope, not two rival designs.
- **Lead with your recommendation** as the first option, suffixed `(Recommended)`,
  and say in its `description` why you'd go that way. A menu with no opinion pushes
  the work back onto the user.
- **`description` carries the cost.** Not a restatement of the label — the thing the
  user can't see: what it forecloses, what it costs, what breaks later.
- **Use `preview` when the choice is a shape** — a file layout, a schema, a CLI
  surface, an interface signature, a rendered UI block. Seeing two ASCII sketches
  side by side settles arguments that two sentences cannot. Single-select only.
- **Four questions per call, max.** Batch a round; never trickle.

## The dimensions

Sweep all of these. Most rounds ask about three or four of them — the rest either
have an obvious default (state it, don't ask) or don't apply (skip silently).

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

## Turning an answer into the next question

A round of answers usually opens a fork the first round couldn't see. That's not
scope creep, it's convergence — ask the follow-up. Common chains:

- "persist it" → *where*, and *does it migrate?*
- "reuse the existing mechanism" → *what happens to its current callers?*
- "fail loud" → *loud to whom — logs, exit code, or a user-facing error?*
- "just the first slice" → *what is the seam, so slice two isn't a rewrite?*

Stop when a round produces no answer that would change the design. That's the
convergence test, and it's the only reason to stop — not question count, and not
the sense that you've asked enough.

## Worked example

Request: *"add caching to the API client."*

**Round 1** — four questions, options each:

| Question | Options |
| --- | --- |
| What are we optimizing? | Latency on repeat reads *(Recommended — the usual reason)* / Rate-limit headroom / Offline capability |
| Cache lifetime? | Process memory, dies on restart *(Recommended — no invalidation bugs)* / On-disk, survives restarts / Shared across processes (Redis) |
| Staleness tolerance? | Seconds — TTL 30s / Minutes — TTL 5m *(Recommended)* / Must always be fresh, validate with ETag |
| On a cache write failure? | Ignore, serve from origin *(Recommended — cache is an optimization)* / Fail the request / Log and disable the cache for the session |

Answers: latency, on-disk, ETag validation, ignore failures.

**Round 2** — "on-disk + ETag" opened a fork round 1 couldn't see:

| Question | Options |
| --- | --- |
| Where does the disk cache live? | `$XDG_CACHE_HOME/<app>` *(Recommended — respects the platform)* / Alongside the config / Configurable, defaulting to XDG |
| Eviction? | Size-capped LRU, 100 MB *(Recommended)* / TTL-only, never evict early / Manual `cache clear` only |

Round 3 would produce nothing that changes the design — so it doesn't run. Now the
approach options (step 5) are worth writing, because they're arguing about a design
whose constraints are actually pinned down.
