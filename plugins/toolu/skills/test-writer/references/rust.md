# Rust tests

## Write it well

- Put unit tests next to the code, in a `#[cfg(test)]` module or in the repository's sibling-file layout. Keep `tests/` for black-box tests of the public API.
- Each file directly under `tests/` compiles into its own binary and links the crate again. Prefer one `tests/it/main.rs` with modules over many files: on Cargo's own suite this compiled 3x faster and used 5x less disk. If the repository's layout says otherwise, follow it.
- Use real temp dirs, real processes and real files instead of trait mocks or mocking crates.
- Compare exact values with `assert_eq!`. For errors, match the variant, not just `is_err()`.
- Use `proptest` for invariants only if it is already a dependency. Do not add a dependency for one test.

## Run it cheap

- Edit loop: `cargo test -p <crate> <filter>`. The filter matches the module path (`module::tests::name`), not the file name. A filter that matches nothing passes, so check that the `running N tests` line shows N > 0.
- With nextest, run `cargo nextest run -p <crate> -E 'test(<name>)'`. Put heavy tests in a `[test-groups]` entry with `max-threads`, or give them `threads-required`.
- Every test binary uses all cores. When several agents share the machine, cap it with `-- --test-threads=<n>`, `NEXTEST_TEST_THREADS` and `CARGO_BUILD_JOBS`.
- Check the tests have teeth on the diff: write `git diff <base>..` to a file, then run `cargo mutants --in-diff <file> -p <crate>`. A diff that touches only tests runs no mutants.
