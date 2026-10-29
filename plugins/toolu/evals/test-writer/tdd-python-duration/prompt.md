---
description: TDD — tests come first and run red, then go green; few tests, no mocks, few test runs.
tags: [behavior, python]
max_turns: 30
timeout_seconds: 900
allowed_tools: [Read, Glob, Grep, Skill, Agent, TodoWrite]
---

Implement `parse_duration(text)` test-first, in `duration.py`, with tests in
`test_duration.py` written with the standard library `unittest` (pytest is not installed).
Run the tests with `python3 -m unittest -q`.

Spec:

- An integer followed by a unit: `ms`, `s`, `m` or `h` (`250ms`, `90s`, `5m`, `2h`).
- The result is the total in milliseconds, as an `int`.
- Parts may be concatenated: `1h30m` is 5400000.
- Empty input, a missing unit, an unknown unit, or a unit with no digits raises
  `ValueError`.

Use only the standard library. When done, reply with the test names and the final test
result line.
