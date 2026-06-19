# Dashboard test fixtures

Real Claude Code transcript data captured from an actual session, used by the
activity-lane tests. No mocks: every line uses the real transcript schema and is
parsed by the real reader.

- `transcript-main.jsonl` — three real lines (two `Agent` spawns, one real
  `tool_result`) plus a deliberately truncated final line, for the tolerant
  reader (AC-15).
- `cc-store/` — a `~/.claude`-shaped store (`projects/<slug>/<session>.jsonl` +
  `<session>/subagents/agent-*.{jsonl,meta.json}`) assembled from **verbatim real
  captured lines**, with two faithful edits for coverage the live session lacked:
  one spawn's `tool_result` is omitted (a real "captured mid-run = running" case)
  and one `tool_result` carries `is_error: true` (the real error shape, since no
  agent errored in the source session). Large prompt bodies are elided — the
  parser reads only `type`/`timestamp`/`id`/`name`/`tool_use_id`/`is_error`/
  `input.{description,subagent_type}`.

Scenario in `cc-store`: top-level `a4db96c6` (done) → children `ab647f22`,
`a8eb8e23` (both done); top-level `a720f8d7` (running); top-level `a273da` (error).
