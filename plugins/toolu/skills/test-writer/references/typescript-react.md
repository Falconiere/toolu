# TypeScript, React and React Native tests

## Write it well

- Query the way a user sees the screen: `getByRole` with a `name` first, then label, placeholder, text and display value. Use `getByTestId` only as a last resort.
- Prefer `userEvent` to `fireEvent`. Use `findBy*` for async content instead of `waitFor` with `getBy*`. Never put side effects inside `waitFor`, and keep one assertion per `waitFor`.
- React Native Testing Library:
  - Use `screen` queries and `toBeOnTheScreen()`.
  - Use `queryBy*` only to assert absence.
  - Do not wrap calls in `act()` or call `cleanup()`; the library does both.
- No shallow rendering. A snapshot of a whole tree is never the only assertion; keep snapshots for small, stable output.
- Stub the network at the HTTP boundary (MSW or a local test server). Do not replace your own modules with `vi.mock` or `jest.mock`.
- Use fast-check (`fc.assert(fc.property(...))`) for pure functions. It runs 100 cases by default.
- `tsc` already checks types. Do not write tests that only re-check them.

## Run it cheap

- Edit loop:
  - Vitest: `vitest run <file>`, or `vitest related <changed source files>`. `related` follows static imports only.
  - Jest: `jest <file>`, or `jest --findRelatedTests <files>`.
- Jest defaults to every core but one. Pass `--maxWorkers=50%`, or a fixed number, when other agents run tests on the same machine.
- If Jest memory keeps growing, set `workerIdleMemoryLimit` (for example `0.2` or `'512MB'`). It restarts any worker left above the limit after a test.
- Vitest:
  - Set `isolate: false` in a project of pure unit tests, and keep isolation for integration tests.
  - Use `pool: 'forks'` if native modules crash under threads.
- `stryker run --incremental` reruns only the mutants affected by changed code or tests.
