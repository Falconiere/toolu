---
type: llm
focus: trace
weight: 2
---

PASS if the trace shows tests for `parse_duration` written first, then a test run that
failed (a failing assertion, or an import or attribute error because the function was
missing or a stub) before the real implementation, then a later test run that passed.

FAIL if the tests were first run only after the implementation was complete, if they were
never run, or if an assertion was weakened or a test deleted to reach green.
