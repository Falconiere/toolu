---
type: llm
focus: trace
weight: 2
---

Look at the result the subagent returned to the main conversation.

PASS if that result is 15 lines or fewer and names the test file, lists the tests (ideally
with the bug each catches), and gives the exact test command with its decisive result line.

FAIL if it pastes raw test output, exceeds 15 lines, or claims success without naming the
command that was run.
