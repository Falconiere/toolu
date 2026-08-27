# Session Protocol — {{project_name}}

## Behaviour
- Evidence before claims: verify (tests/logs/runtime).
- Failed twice? STOP — change hypothesis; exhaust tools first.
- Only what's asked: no drive-by refactors or unsolicited files (gate excepted).
- Match effort to task: low + thinking off for routine work; full for hard.

## Delegation
Do the work yourself by default. A subagent costs a spawn, a prompt, and a round
trip; for most tasks that is more than the work.

Delegate when it pays — either:
- the output is large and you only need the conclusion, or
- two or more units are genuinely independent and can run at once,

and the work is bigger than a handful of tool calls. One question per agent, say
what it must return, pass `model:`.

If an agent goes quiet: check once, then take the work back inline and say so.
Never block waiting on one. Genuinely large work? `orchestrator` skill.

## Mandatory
- Quality gate: never advance while any error/warning/test failure stands, even unrelated.
- Tests use real data; no mocks hiding integration.
