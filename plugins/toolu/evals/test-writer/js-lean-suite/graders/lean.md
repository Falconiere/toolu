---
type: llm
focus: { source: file, path: cart.test.mjs }
weight: 2
---

PASS if the file has at most 8 test cases, each checking a distinct behavior from the spec
(input not mutated, same `sku` merges `qty`, invalid `qty` throws `RangeError`, `total`),
with exact expected values.

FAIL if it has more than 8 tests, if several tests check the same behavior with trivially
different inputs, or if a test asserts internal details such as how `items` is built
rather than the returned values.
