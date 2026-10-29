---
type: llm
focus: { source: file, path: test_duration.py }
---

PASS if the file has at most 12 test methods and each covers a distinct behavior from the
spec (each unit, concatenation, each error class), with exact expected values.

FAIL if several tests check the same behavior with trivially different inputs, if tests
assert private helpers, or if a test only checks that no error was raised where the spec
gives an exact value.
