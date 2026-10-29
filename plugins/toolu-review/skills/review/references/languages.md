# Language checks

Read only the section matching changed code. Repository rules and configured
toolchains determine supported syntax, test layout and checks; do not impose a
different framework or upgrade dependencies as part of review.

## TypeScript

- Treat external JSON, storage and network values as untrusted. Narrow validated
  values; a type assertion does not check runtime data. Check null/undefined,
  empty strings, zero and discriminated-union exhaustiveness.
- Follow asynchronous work through callers: awaited/reported rejection,
  cancellation, stale responses and resource cleanup. Check public type changes
  against consumers, not only the edited file.
- Use the project's typecheck and tests. Check forbidden assertions/suppressions
  against installed rules; do not add a validator dependency when one exists.

Sources: [narrowing](https://www.typescriptlang.org/docs/handbook/2/narrowing.html),
[type assertions](https://www.typescriptlang.org/docs/handbook/2/everyday-types.html#type-assertions).

## Rust

- Propagate recoverable errors with context; inspect panic/index/overflow paths
  for reachable inputs. Trace ownership and cleanup across early returns.
- For concurrent code, inspect lock ordering, guards across waits, cancellation,
  bounded queues and shutdown. Any unsafe code requires explicit invariant
  evidence; a repository ban takes precedence.
- Check public/API and persisted-format compatibility, feature combinations and
  platform assumptions touched by the diff. Run project formatting, lints and
  focused tests; distinguish unit/integration coverage from documentation tests.
- Do not hide a failure with lint suppression or skip a required gate. This
  repository puts unit tests in module-sibling `tests/` and integration tests at
  crate root; follow `AGENTS.md` for the exact wiring and permitted test commands.

Sources: [recoverable errors](https://doc.rust-lang.org/book/ch09-02-recoverable-errors-with-result.html),
[shared state](https://doc.rust-lang.org/book/ch16-03-shared-state.html).

## Python

- Validate dynamic input before use. Check mutable defaults, missing/empty
  values, iterator exhaustion, timezone and encoding assumptions where relevant.
- Catch specific exceptions at a boundary that can handle them; preserve the
  cause when translating errors. Ensure files, transactions and tasks clean up
  on failure. Do not swallow asynchronous cancellation.
- Run the project's configured tests and static checks. Types alone do not
  validate runtime values. Use real temporary resources for filesystem/process
  behavior; do not mock away the integration under review.

Sources: [errors and cleanup](https://docs.python.org/3/tutorial/errors.html),
[task cancellation](https://docs.python.org/3/library/asyncio-task.html#task-cancellation).
