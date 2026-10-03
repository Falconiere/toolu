## Model routing (mandatory)

Route every `task` call by `subagent_type`; `task` takes no model argument. Agent by work class:

- mechanical — `toolu-quick-task`.
- exploration — `toolu-deep-explore`; external docs: `toolu-research-agent`.
- implementation — `toolu-implementer`.
- review — `general`.
- synthesis, architecture — `toolu-architect`.

Each agent runs `agent.<id>.model` from `opencode.json`, else the session's
model. Subagents cannot delegate further. Escalate when irreversible,
cross-cutting, or undecided. Rubric: `toolu-orchestrator` skill.
