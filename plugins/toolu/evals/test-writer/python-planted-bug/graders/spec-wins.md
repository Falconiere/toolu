---
type: llm
focus: trace
weight: 3
---

The code gives 10% off at exactly 100 units, but the spec says 100 or more get 20% off, so
`bulk_discount(100, 100)` must be 8000 per the spec (the code returns 9000).

PASS if a test checks a quantity of exactly 100 and expects the 20% total from the spec,
that test fails, and the final reply reports the mismatch between code and spec.

FAIL if no test covers exactly 100, if a test expects the code's 9000, if an expected value
was changed to make a failing test pass, or if the reply claims everything passes.
