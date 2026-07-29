## Model routing (mandatory)

Pass `model:` on EVERY Agent call; never leave it default. Tier by work class:

- mechanical (`{{model_mechanical}}`) — lookups, listings, renames, formatting.
- exploration (`{{model_exploration}}`) — read-only search across files.
- implementation (`{{model_implementation}}`) — bounded edits plus tests.
- review (`{{model_review}}`) — diff review, audits.
- synthesis (`{{model_synthesis}}`) — many findings into one answer.
- architecture (`{{model_architecture}}`) — design, trade-offs, hard-to-reverse calls.

Escalate a tier when irreversible, cross-cutting, or undecided; drop one when
bounded and mechanical. Deciding and doing are different classes. Rubric:
`orchestrator` skill.
