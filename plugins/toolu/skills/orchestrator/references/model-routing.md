# Model routing

How to pick the model for a delegated task. The rule is: **route on the class of
work, not on how the request is phrased.** A one-line prompt can hide an
architecture decision; a long prompt can be a file listing.

Referenced by `orchestrator`, `brainstorm`, `plan`, and `execution`. The runtime
copy of the table is injected into every session by the toolu SessionStart hook
(`plugins/toolu/hooks/docs/model-routing.md`), so the tiers are in context from
turn one even when no skill has fired.

## The ladder

Tiers are named by **alias**, never by version id — `sonnet` means "the current
mid tier", so the routing survives a new model generation with no edits.

| Class | Default | What belongs here |
|---|---|---|
| `mechanical` | `haiku` | Listings, single-symbol lookups, exact-literal search, mechanical renames, formatting, extracting one value, running one command. |
| `exploration` | `sonnet` | Read-only search across many files; "where/how is X done"; mapping a subsystem. |
| `implementation` | `sonnet` | A bounded, already-decided edit plus its tests. |
| `review` | `sonnet` | Diff review, audits, convention checks. |
| `synthesis` | `opus` | Reconciling several agents' findings into one answer. |
| `architecture` | `opus` | Design, trade-offs, hard-to-reverse or cross-cutting decisions. |

`inherit` is also a valid value — "whatever the lead thread runs". Use it when a
subagent must match the session's model exactly rather than a fixed tier.

## Classify by signal, not by vibe

Score the task on four signals. Any single **yes** in the escalate column pulls
the task up a tier; all-no in a bounded task pulls it down.

| Signal | Escalate | De-escalate |
|---|---|---|
| **Reversibility** | Hard to undo — schema, public interface, migration, deletion | Trivially revertable edit |
| **Blast radius** | Cross-cutting: many callers, several subsystems | One file, one function |
| **Ambiguity** | The *how* is not decided; requirements are implicit | Fully specified — what, where, and how to verify are all given |
| **Reasoning depth** | Must weigh alternatives or reconcile conflicting inputs | One correct answer, verifiable on sight |

Two worked examples:

- *"Find every call site of `pl_recompute`"* — reversible, no ambiguity, no
  reasoning. Mechanical → `haiku`, even though the codebase is large.
- *"Add a `model` field to the ledger steps"* — touches a persisted contract
  every reader parses. Hard to reverse, cross-cutting → `opus` decides the
  shape, `sonnet` implements it once decided.

Note what the second example shows: **one task can split across tiers.** Deciding
and doing are different classes. Don't buy frontier reasoning to type out an
agreed edit, and don't hand an undecided design to the cheap tier because the
diff looks small.

## Escalation is one-way and cheap

If a subagent returns `ESCALATE: <reason>`, re-run the task one tier up with that
reason included in the prompt. That is the designed path, not a failure — the
`quick-task` and `implementer` agents are told to escalate rather than guess.
Never re-run a failed task on the *same* tier hoping for a better roll.

## Pre-tiered agents

Pinning the tier in the agent's own frontmatter is stronger than remembering to
pass `model:`, so prefer these when one fits:

| Agent | Tier | Job |
|---|---|---|
| `toolu:quick-task` | `haiku` | Mechanical lookups and bounded mechanical edits |
| `toolu:deep-explore` | `sonnet` | Structural exploration via ast-grep |
| `toolu:research-agent` | `sonnet` | External docs / web research |
| `toolu:implementer` | `sonnet` | One bounded plan step + its tests |
| `toolu:architect` | `opus` | Design, trade-offs, synthesis (read-only) |

For anything else, pass `model:` explicitly on the Agent call. Leaving it unset
routes everything to the lead thread's model — the exact waste this rubric exists
to stop.

## Plan steps carry their tier

A machine-readable plan step may declare `"model": "<alias>"`:

```json
{ "id": "s3", "title": "Add the resolver", "check": "bats plugins/toolu/hooks/lib/__tests__/config-models.bats",
  "model": "sonnet" }
```

`plan` assigns it while the complexity is still fresh; `execution` reads it back
(`plan-ledger.sh status` prints `model=<alias>` for the next step) and delegates
at that tier without re-deriving the judgment. The field is optional — a legacy
step without it is still valid, and the executor falls back to this rubric.

## Configuring the tiers

Remap any class in `toolu.config.json`:

```json
{ "models": { "review": "opus", "mechanical": "sonnet" } }
```

Values must be one of `haiku`, `sonnet`, `opus`, `fable`, `inherit`; anything
else is rejected with a warning and falls back to the default, so a typo
mis-tiers nothing. `{"models": {"enabled": false}}` turns off the session
injection entirely.

**Limit:** config remaps the *rubric* — the tiers you pass on Agent calls and the
table injected at session start. It cannot rewrite the `model:` line in an
agent's frontmatter, which Claude Code reads directly from the file. To re-tier a
pre-built agent, edit its frontmatter.

## Budget note

Routing saves money only if it is paired with restraint on fan-out. A wide
parallel sweep on the top tier is the expensive failure mode this rubric is meant
to prevent, not enable — cap the agent count first, then tier each one.
