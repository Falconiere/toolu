# Host mapping

Use the host-native interface for the same workflow concept:

| Concept | Claude Code | Codex | OpenCode |
| --- | --- | --- | --- |
| Invoke a plugin workflow | `/plugin:name` | `$plugin:name` | the `skill` tool with the generated skill ID |
| Delegate bounded work | `Agent` / `Task` with an explicit tier | `spawn_agent` with the matching custom agent when installed | `task` with `subagent_type` naming a `toolu-*` agent; the model comes from `agent.<id>.model` in `opencode.json` |
| Ask a structured user choice | `AskUserQuestion` | `request_user_input` when available; otherwise ask one concise question | `question` when available; otherwise ask one concise question |
| Inspect or steer delegated work | the host's agent controls | Codex subagent thread controls (`/agent` in CLI) | the child session (`task_id` resumes it) |
| Isolate a write-heavy task | `EnterWorktree` / `ExitWorktree` | native `git worktree` commands with an exact, validated path | native `git worktree` commands with an exact, validated path |
| Current external information | `WebSearch` / `WebFetch` or installed research plugins | Codex web access or installed research plugins | `websearch` / `webfetch` or installed research plugins |

Never name or call a host interface that is unavailable in the active host. A
missing optional interface is a reason to use the mapped fallback, not to
fabricate a tool call.

Workflow skills default-and-proceed according to materiality triage. Use the
structured question row only for one question (2–3 options) when prompt and
repository evidence cannot settle a goal-defining or hard-to-reverse fork. If
several qualify, ask the highest-blast-radius choice and record defaults and
risks for the rest; otherwise state the recommended default and proceed.
