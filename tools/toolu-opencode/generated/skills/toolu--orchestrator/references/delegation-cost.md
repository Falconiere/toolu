# What delegation costs

Delegation is usually presented as free — split the work, run it in parallel, win.
It is not free, and the cases where it loses are common enough that a default of
"delegate" makes ordinary tasks slower.

This is the arithmetic the [orchestrator skill](../SKILL.md) refers to.

## The price of one subagent

Every delegation pays, whether or not it earns it back:

| Cost | What it is |
|---|---|
| Prompt authoring | You must write a self-contained brief. The subagent cannot see your conversation, so context you already hold has to be re-stated. |
| Spawn + round trip | Wall-clock before a single useful token, plus the return trip. |
| Summary risk | You get the agent's reading of what it found, not what it found. Anything it judged irrelevant is gone. |
| Verification | A returned claim you rely on has to be trusted or re-checked. Re-checking pays for the work twice. |
| Attention | You have to hold "what did I delegate, has it come back" while doing something else. |

A subagent buys two things and only two: **isolation** (a large read that never
enters your context) and **parallelism** (independent work at the same time). If a
delegation buys neither, it is pure cost.

## When it pays

**Isolation.** "Which of these 40 files register a hook?" — the agent reads
everything, you get a `file:line` table. The 40 files never enter your context and
never re-enter it on every later turn. This is the strongest case, and it gets
stronger the longer the session runs.

**Parallelism.** Four independent searches with no dependency between them, in one
message. Wall-clock is the slowest, not the sum. The word doing the work is
*independent*: if search two needs search one's answer, there is no parallelism to
win.

**Isolation of a long job.** A sweep that takes minutes and produces noise you do
not want interleaved with your work.

## When it loses

**The task is small.** Under roughly a handful of tool calls, the brief alone costs
more than the work. A single-file edit, a known-symbol grep, reading a file you
already know — do them.

**The chain is dependent.** A → B → C where each needs the last is *strictly*
worse delegated: every round trip is paid and no parallelism is gained. Chain
inline; delegate only branches.

**You will re-read the output anyway.** If you cannot act on the summary without
opening the files yourself, the isolation you paid for was never real.

**The work is the decision.** Trade-offs, architecture, "which of these is right" —
delegating produces a recommendation you must then evaluate, which is most of the
work. Get the *inputs* delegated; make the call yourself.

**You are splitting to look thorough.** Three agents on one small file produce
three summaries of the same thing. Coverage is a property of the search, not of
the agent count.

## A worked case

A session that built a cross-cutting feature end to end — a written spec, five new
shell libraries, seven module migrations, ~300 real-data tests, eight commits, a
pull request, three rounds of review response — ran entirely on one thread. Zero
subagents.

The bottleneck was never reasoning. It was the test suite: 131–165 seconds a run,
several runs. No amount of fan-out moves that number, because the suite is one
command. The delegable work — "read these files and tell me what is there" — was
a small fraction of the total, and the files were mostly ones the thread had
already read.

Fanning that same work across five agents would have added five briefs, five round
trips, and five summaries to verify, to save reading that had already happened.
The measurable win would have been zero and the wall-clock strictly worse.

The lesson is not "never delegate". It is that **the size of a task is not the
same as its delegability.** A big task made of dependent steps on files you
already understand is a big inline task.

## The test, restated

Before spawning anything:

1. Does this buy **isolation** (large output, conclusion only) or **parallelism**
   (genuinely independent units)?
2. Is the work bigger than a handful of tool calls?

Both no → do it. One yes → probably delegate, with a bounded scope and a stated
return contract. Neither → you are about to make a fast task slow.

## Bounding what you do delegate

A delegation that cannot fail visibly will eventually hang:

- One answerable question per agent. "Find every call site of X" returns.
  "Improve the module" does not.
- State the return shape in the prompt — a table, a verdict, a five-line summary.
- If it does not come back: check once, then do the work inline and say you did.
  Never block indefinitely; an agent that went quiet has already cost more than
  the work.
- Never re-spawn an identical prompt hoping for a better result. Change the tier
  (see [model routing](model-routing.md)) or take it back.
