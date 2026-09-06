---
name: test
description: "Use while executing any feature, fix, refactor, or regression to design high-signal, real-data tests. Enforces test-first discipline and colocated layout; it is a reusable method, not a terminal workflow phase."
---

# Test

`test` is a **reusable execution-time method**: use it in every behavior step
and again when reproducing regressions. Tests are written with the change,
through red → green → refactor, never saved for a terminal phase.

## Non-negotiables

- Exercise real inputs and real code paths. Do not use mocked or fabricated
  data as a mock-substitute for the behavior under test. A controlled external
  failure may be stubbed only to prove failure propagation.
- Colocate tests: TS/TSX in sibling `__tests__/`, Rust in module-sibling
  `tests/` (or crate-root integration `tests/`), and Python `test_<module>.py`
  beside its module. Use the project runner and existing no-mock gates.

## Behavior-to-evidence map

Before writing each test, record a compact map containing:

- relevant AC or risk;
- representative real input or fixture;
- observable expected result;
- boundary or failure case when applicable; and
- runner command.

Prioritize changed behavior, invalid/boundary input, failure propagation, and
reproduced regressions. Reject duplicate tests, implementation-detail tests,
mock-substitute tests, and happy-path-only tests.

## Test-first loop

1. **Red** — add a focused failing test that demonstrates the intended behavior
   or reproduces the regression; run it and observe the expected failure.
2. **Green** — make the smallest production change that passes with the real
   input.
3. **Refactor** — improve structure without changing behavior; keep the runner
   green.

One test demonstrates one behavior and names that behavior. A red test is a
finding: report its output and never claim the execution step complete until
the mapped runner is green.
