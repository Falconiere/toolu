---
name: test-writer
description: "Use to write tests or TDD a change — unit, integration, Playwright, Appium, React Native — or when a suite is bloated or slow. Produces few spec-derived tests that catch real bugs, without mocks, and runs only affected tests. Not for diagnosing a failing test (debug)."
---

# Test writer

Write few tests, each catching a bug you can name, derived from the spec, and cheap to run. Agents fail at tests in known ways: they delete or weaken failing tests, hard-code expected values, mock every collaborator, and copy what the code currently does into the assertions, bugs included. Each rule below blocks one of those.

## Pick the mode

- **New behavior:** run the TDD loop.
- **Existing untested code:** take expected values from the spec, docs, issue or user, never from running the code. When code and spec disagree, keep the spec's expectation, let the test fail, and report the mismatch. Do not change the assertion to make it pass.
- **Bloated or slow suite:** prune it (below).
- **Large task, or you also write the code:** delegate the tests to the `test-author` agent with the spec, the public interface and the test command. It returns a short receipt and keeps test output out of your context.

## TDD loop

1. Write the spec as input → expected output pairs, plus the edge cases the spec states. Say it is TDD, so no mock implementation gets written.
2. Write the tests and run them. Confirm each fails for the expected reason (a failed assertion or a missing symbol), not because of a typo or broken setup.
3. Commit the tests, or note the commit SHA, before writing any code.
4. Implement the minimum that passes. Do not edit test files. Before claiming green, run `git diff <tests-sha> -- <test paths>`; it must print nothing. If a test is wrong, stop and say so instead of editing it.
5. Refactor while the tests stay green.
6. Check for overfitting: no branches on test inputs, and no code that reads test files.

## What earns a test

Before adding a test, state in one line the bug it would catch. Skip the test when:

- another test already catches that bug;
- it would break on a refactor that keeps behavior (it asserts private calls, internal state, CSS classes or call order);
- it tests the framework, a third-party service, or a getter with no logic;
- it mocks the collaborators it should integrate with.

Prefer:

- One test per behavior, named after the behavior.
- The public, user-facing interface: role and label queries in UIs, the public API in libraries.
- Real dependencies when they fit in a small test (in-memory database, temp dir, local server). Fake only the network, payments, the clock and randomness, at the boundary.
- One property test (fast-check, Hypothesis, proptest) over many near-duplicate examples. Keep the default 100 cases in the edit loop.
- Exact values from the spec. Never assert truthiness when the value is known.

## Size every test

- **Small:** one process; no network, file system, sleep or threads. Most tests belong here.
- **Medium:** localhost, temp files, a real database. Integration tests.
- **Large:** a browser or a device. Keep these to a few critical user journeys.

Never put a sleep, a network call or the real clock into a small test. Use the framework's waits or a fake clock.

## Run cheap

- While editing, run only the affected tests: one file, one test name, or the runner's related-tests mode. Run the full suite once, at the gate.
- When other agents share the machine, cap the worker count (see your stack reference).
- Read failures through a summary: pipe test output to `toolu debug testfail` instead of pasting it into context.
- Record traces, videos and screenshots only on retry or failure.

## Prune a bloated suite

1. Measure the total count and the slowest tests.
2. Find duplicates: tests that assert the same behavior or always fail together.
3. Run diff-scoped mutation testing (`cargo mutants --in-diff`, `stryker run --incremental`). A test that kills no mutant the others miss is a candidate for deletion.
4. Delete or merge only with the user's approval. Never delete a test just because it fails.

## Stop and report

Stop and tell the user when: the same fix fails twice, a test you wrote has to change before it can pass, someone asks you to skip or delete a failing test, expected values were copied from the code, or the work grows past the behavior you were asked to test.

## Stack references

Read exactly one, for the stack under test:

- Rust: `references/rust.md`
- Python: `references/python.md`
- TypeScript, React and React Native components: `references/typescript-react.md`
- Browser E2E with Playwright: `references/e2e-web.md`
- Mobile E2E with Appium, Detox or Maestro: `references/e2e-mobile.md`

The repository's own test layout and commands come first (`AGENTS.md`, `CLAUDE.md`, the existing tests). The references fill the gaps.

## Done means

Report each test added (its path and the bug it catches), the red evidence (the command and its failing line), the green evidence (the command and its passing line), and any part of the spec you could not test.
