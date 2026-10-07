---
description: "Use when installing, previewing, updating, backing up, or removing toolu's custom Codex agent profiles."
name: "toolu-setup"
---

# Set up toolu agents

On OpenCode there is nothing to install. The toolu plugin's `config` hook
registers its five agents at every start: `toolu-quick-task`, `toolu-deep-explore`, `toolu-research-agent`, `toolu-implementer` and `toolu-architect`. Codex agent profiles do
not apply to OpenCode.

Tell the user that, then offer the OpenCode equivalents:

- **Pin a model:** set `agent.<id>.model` (for example
  `agent.toolu-quick-task.model`) in `opencode.json`. toolu's prompt and
  permissions stay.
- **Drop an agent:** set `agent.<id>.disable` to `true`.
- **Update:** `npx @toolu/plugins update --host opencode`, then restart OpenCode.

`toolu setup agents` refuses to run on OpenCode: it exits 2
with this explanation and writes nothing. Do not edit Codex profiles from here.
