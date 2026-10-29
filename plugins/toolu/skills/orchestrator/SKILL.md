---
name: orchestrator
description: Decide whether and how to delegate substantial work. Use for independent parallel tasks or isolating large reads; keep small or dependent work inline.
---

# Orchestrate only useful delegation

Do cheap discovery inline. Delegate only when the work exceeds a few tool calls
and buys either independent parallel progress or isolation of a large read.
Keep dependent steps inline; do not have multiple agents repeat the same work.
See [cost examples](references/delegation-cost.md) only if the trade-off is unclear.

For each delegation, specify one bounded question or task, permitted files and
side effects, needed evidence, verification, and a compact return shape.
Pass the session's applicable memory/search requirements. Agents do not delegate
again unless a nested workflow is explicitly needed. The main thread owns
integration and verifies claims it relies on.

Use the active host's [delegation interface](../../workflows/host-mapping.md).
Start independent units together within available concurrency and budget.
If an agent goes quiet, check its state once; stop or reassign it before taking
over, so two agents cannot edit the same files. Report incomplete work honestly.
An `ESCALATE` response needs a narrower question, more evidence or a higher tier,
not an identical retry.

## Route by work

Use SessionStart's configured class/model/effort values when present; read
[model routing](references/model-routing.md) for missing defaults or overrides.

| Agent | Bounded job |
| --- | --- |
| `quick-task` | Mechanical lookup or exact edit |
| `deep-explore` | Local code relationships and architecture evidence |
| `research-agent` | External facts and documentation |
| `implementer` | One decided step plus verification |
| `test-author` | Spec-only tests for one behavior |
| `architect` | Design recommendation or synthesis |

Claude names these `toolu:<agent>`; Codex custom profiles use the agent name.
Muse exposes `/toolu:<agent>` as a command in the current thread, not a spawned
or tier-pinned agent. Use only capabilities the host exposes.

Return conclusions and evidence locations, not file/transcript dumps. Record
scope limits and unfinished units. Delegation count is not a coverage metric.
