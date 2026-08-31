# Host mapping

Use the host-native interface for the same workflow concept:

| Concept | Claude Code | Codex |
| --- | --- | --- |
| Invoke a plugin workflow | `/plugin:name` | `$plugin:name` |
| Delegate bounded work | `Agent` / `Task` with an explicit tier | `spawn_agent` with the matching custom agent when installed |
| Ask a structured user choice | `AskUserQuestion` | `request_user_input` when available; otherwise ask one concise question |
| Inspect or steer delegated work | the host's agent controls | Codex subagent thread controls (`/agent` in CLI) |
| Isolate a write-heavy task | `EnterWorktree` / `ExitWorktree` | native `git worktree` commands with an exact, validated path |
| Current external information | `WebSearch` / `WebFetch` or installed research plugins | Codex web access or installed research plugins |

Never name or call a host interface that is unavailable in the active host. A
missing optional interface is a reason to use the mapped fallback, not to
fabricate a tool call.

Workflow skills do not open the user-choice row. They pick the recommended
default, state it, and proceed. Open that interface only when the user asked
to choose.
