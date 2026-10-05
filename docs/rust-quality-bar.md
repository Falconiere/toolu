# The Rust quality bar

Epic #402 rewrites toolu in Rust, mostly by agents working in parallel. The bar
below existed before the first product crate (#455), because a gate added after
the code exists turns into a list of exemptions. The rule table and the
commands are in [AGENTS.md](../AGENTS.md#rust-conventions); this page gives the
reasons.

## Principles

1. **One owner per rule.** clippy, rustc, rustfmt, cargo-deny, cargo-machete
   and jscpd keep what they can enforce. `cargo xtask guardrails` owns only
   what they cannot see: file and `impl` length, test layout, the behaviour
   inventory, folders, crate hygiene, capability use, suppressions, to-do
   markers and secrets. Two enforcers of one rule drift apart.
2. **One source of numbers.** File, function and `impl` limits live in
   `lang.rust` of `.claude/toolu.config.json`, which the rust-quality plugin
   and the guardrails both read; the complexity thresholds live in
   `clippy.toml`. The guardrails fail when `too-many-lines-threshold` and
   `maxFnLines` disagree, or when `.codex/toolu.config.json` repeats
   `lang.rust` differently.
3. **We pass what we ship.** The gate runs the rust-quality plugin's own rules
   over every crate file (`bun run check:rust-quality`) at the same limits an
   agent sees after each edit, and toolu's quality gate refuses a commit while
   that post-edit check fails.
4. **No exemptions.** No per-path override, ignore list or suppression
   attribute. The data loaders reject unknown keys, and
   `crates/xtask/tests/no_exemptions.rs` fails on a `skip`, `ignore`,
   `exceptions`, `allowed-*` or extra `allow` lint level. A rule that is wrong
   is changed for everyone, in its own PR.
5. **A rule that cannot fail is not a rule.** Every rule has a clean and a
   violating fixture under `fixtures/guardrails/rust/`, run against this
   repository's real gate data by `cargo test -p xtask`.
6. **Same gate everywhere.** `cargo xtask gate` is the `rust` CI job on Linux
   and macOS, the `lefthook.yml` pre-push hook, and what an agent runs locally.

## Why these limits

- **300 code lines per file, 50 per function, 200 per `impl` block, cognitive
  complexity 15, nesting 4, 5 parameters.** Stricter than toolu-ghrunner
  (500, 150, 20, 6) because each file must fit in an agent's working view and
  be reviewable in one pass. Code lines exclude blank lines and comments, so
  documentation never pushes a file over. toolu-ghrunner's numbers are the
  fallback if these prove too tight, changed in a `chore(gates):` PR.
- **Unit tests beside the module** (`src/<dir>/tests/<module>_test.rs`, wired
  by `#[cfg(test)] #[path = …] mod tests;`). Production files stay readable,
  tests keep private access, and the guardrails can check that every module
  with a function has its test file.
- **Behaviour inventory.** Each hook entry, gate, rule, CLI verb and engine
  transition names a passing and a failing test, so a deny path is never
  untested. Discovery is data (`rules.json`): today the `cargo xtask` task
  table; later sub-issues add their kinds in `chore(gates):` PRs.
- **Coverage 85%, 90% for the crates that decide allow or deny**
  (`toolu-protocol`, `toolu-shell`, `toolu-state`, `toolu-engine`). A floor in
  `coverage-floor.json` may only rise; lowering it is a gate change.
- **No panicking paths in `src`.** Hooks fail closed; a panic is a crash, not
  a decision. Tests may unwrap (`allow-*-in-tests`); integration-test helpers
  return `Result`, because clippy treats only `#[test]` functions and
  `#[cfg(test)]` modules as tests.
- **Capability boundaries.** Environment reads, process spawning and the
  standard streams each have one owner (`toolu-runtime`, its `process` module,
  `toolu-protocol` and the CLI output module), so policy for them lives in one
  place. Only `toolu-http` links HTTP or TLS, only `toolu-shell` a shell
  parser. `cargo xtask` is the tooling crate and owns its own use.
- **No `anyhow`, `eyre` or `dyn Error` in a library's public API.** Callers
  match on typed errors to map them to today's messages (#414).
- **Folders.** An allowlist for the repository root, `crates/`, each crate and
  each plugin directory keeps structure deliberate; adding an entry for a new
  crate is registration data and ships with the crate.

## Gate data and registration data

Gate data sets the bar: `[workspace.lints]`, `clippy.toml`, `rustfmt.toml`,
`deny.toml`, `lang.rust`, and `rules.json` and `jscpd.json` under
`tooling/conventions/guardrails/rust/`. Registration data records that new
code exists: `layers.json`, `folders.json`, `inventory.json` and
`coverage-floor.json`. `cargo xtask check-gate-change` compares the working
tree with the merge base: registration data may only grow (append, add a key,
add a floor row at or above the default) and may ship with product code; any
other change to it, and any change to gate data, must ship alone in a PR
titled `chore(gates): …`, with the reason in its body.

## Fuzzing

A crate may hold an admitted `fuzz/` package (folder allowlist): its own
`[workspace]` and nightly `rust-toolchain.toml`, targets in `fuzz_targets/`.
It is not a workspace member, so the binary rule, layers, deny, coverage and
co-located tests do not apply; the guardrails still read its files. The
`fuzz/clean` fixture proves the gate accepts it (#416 decides the fuzzer).

## Budget

`cargo xtask gate` on a warm cache: about 20 seconds locally for the tests and
coverage steps of today's workspace; the CI duration per runner is recorded
in the pull request that introduced the gate and tracked by #410 (10 minutes
or less).
