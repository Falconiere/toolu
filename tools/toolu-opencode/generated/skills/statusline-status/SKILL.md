---
description: "Use when the user asks for current repository, branch, working-tree, quality-gate, push-review, or waiver status in OpenCode."
name: "statusline-status"
---

# Status

Run `toolu --host opencode status` and report its repository, branch, working-tree,
gate, push-review and waiver fields. The explicit host selects OpenCode's state
paths even though lifecycle plugin variables are absent from an ordinary skill
shell. Do not invent Claude-only account, model, effort, or context-window
values when OpenCode does not expose them.
