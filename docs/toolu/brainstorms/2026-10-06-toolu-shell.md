# toolu-shell: Bash/Shell command analysis in Rust — Brainstorm

**Date:** 2026-10-06   **Issue:** [#416](https://github.com/Falconiere/toolu/issues/416)   **Epic:** [#402](https://github.com/Falconiere/toolu/issues/402)   **Path:** Full (public interface, security guardrail input, new external dependency, CI)

## Capsule

- **Outcome:** `crates/core/shell` (`toolu-shell`) answers what `@toolu/core/shell` answers for every fixture input: the simple commands a line runs, what it writes, its git invocations and whether an exit status is observable. Unknown input stays unknown. The crate is fuzzed in CI.
- **Material defaults/non-goal:** tree-sitter-bash is the only parser (brush-parser is deferred, with the evidence below). The parity oracle is a committed projected-analysis fixture checked by both implementations. The fuzzer is cargo-fuzz on nightly in the `fuzz/` package #455 admitted. Out of scope: the per-event analysis cache (`shellAnalysisOf`, an engine concern, #418), the gates that consume the analysis (#419–#422), and the `detect` git subprocess helpers (`pushTargetRoot`, `pushTargetBranch`).
- **Repository evidence:** `packages/toolu-core/src/shell/*.ts` (1,304 lines over unbash 4.0.11) and `fixtures/shell/*.json` (203 distinct inputs). #455 already admits a cargo-fuzz layout (`fixtures/guardrails/rust/fuzz/clean`). #413 already shares a fixture that both encoders must reproduce byte for byte (`fixtures/host/encode.json`).
- **Risk:** tree-sitter-bash's tree differs from unbash's AST, so each construct has to be mapped and checked. Its C error recovery can crash under fuzzing, and a crash would block the hook. The latency budget leaves about 40 µs for the walk.
- **Handoff:** spec.

## Axes

### Parser (constraint: dependencies; interface)

| | Evidence |
|---|---|
| brush-parser first (the issue's plan) | Every release from 0.2.11 to 0.4.0 fails this repository's `cargo deny` with no exemption possible. It pulls syn 2 (through `cached_proc_macro`/darling 0.20, and in 0.4.0 also `insta`→`pest_generator` as a *normal* dependency) next to the workspace's syn 3, plus duplicate darling and hashbrown versions. `foldhash`, licensed Zlib, is not on the allow list. Admitting it means a `chore(gates):` PR that relaxes `multiple-versions` and adds Zlib, and `deny.toml` itself forbids exception entries. |
| tree-sitter-bash only | `tree-sitter` 0.27 and `tree-sitter-bash` 0.25.1 pass `cargo deny` (bans, licences, sources) with the workspace's dependencies. They parse all 201 valid fixture inputs with no `ERROR` node and flag exactly the two malformed inputs that unbash flags. `git push origin main; echo "unterminated` still yields both commands. The parser is iterative: 10,000 nested `$(…)` parse in 19 ms and 100,000 in 194 ms, with no crash. Parse latency over the 203 inputs, in release on a shared loaded host: p50 7 µs, p99 58 µs. |
| hand-written port of unbash | About 6,100 lines of JavaScript lexer and parser would have to be ported under the 300-line-file, 50-line-function and no-indexing rules. This is the largest risk of the three. |

**Decision:** tree-sitter-bash for both complete and partial trees (Jev: 0.99 against 0.01 for brush with a gate change). The issue already names it as the recovery parser, and the epic's measured prototype used it. Deferring brush-parser is recorded in the spec and the PR. It can return as a first parser if a later `chore(gates):` PR admits its dependency graph.

### Parity oracle (data)

unbash's raw AST cannot be compared node for node with tree-sitter's. What both implementations produce is the analysis. **Decision:** `fixtures/shell/analysis.json` holds the TypeScript analysis of every input in `unbash-baseline.json`, in the same order. It is a projection without parser-specific error text or offsets. A Bun test requires TypeScript to keep producing it, and a Rust test requires Rust to produce it. An intended difference is a per-case `rust` override with a `reason` (Jev: 1.00 for this over spawning Bun from Rust tests). This follows #413's `host/encode.json` and outlives the TypeScript implementation.

### Fuzzer (constraint: #455)

**Decision:** cargo-fuzz (libFuzzer) in `crates/core/shell/fuzz/`, with its own `[workspace]` and nightly `rust-toolchain.toml`. This is exactly the layout and fixture #455 admitted, so no gate data changes (Jev: 1.00). A per-PR job in `tests.yml` (group `rust`) runs each target for a bounded time. A scheduled workflow runs them longer. "Both parsers" becomes two targets: the raw tree-sitter parse, and the full analysis including the `bash -c`/`eval` re-parse.

### Failure behaviour (security)

- Input over 1 MiB (UTF-16 units, as TypeScript counts them) is not parsed and is `unknown`.
- `ERROR`/`MISSING` nodes are errors. Commands read around them are kept, every `exitProves` turns false, and a line with errors and no command is `unknown`.
- Nesting past `MAX_NESTING` makes the analysis `unknown` with an error. This is an intended difference: unbash caps `$(…)` at 256 levels without marking the line unknown, so TypeScript can miss the innermost command of a 10,000-level line.
- Left-recursive chains (`a && b && …`, arithmetic `1+1+…`) are flattened or visited iteratively, so only real nesting counts against the limit.
- A C-level crash cannot be caught by `catch_unwind`. The launcher maps it to exit 2 (fail closed, #412), and fuzzing is the guard.

### Integration

- **Dependencies:** `tree-sitter` and `tree-sitter-bash` in `[workspace.dependencies]`. Only `toolu-shell` may link `tree-sitter-bash` (capability rule).
- **CI:** the `rust-musl` job builds the whole workspace, and cc-rs then needs a musl C compiler (measured: "failed to find tool x86_64-linux-musl-gcc"). The job installs `musl-tools` and sets `CC_<target>=musl-gcc`.
- **Documentation:** `docs/shell-analysis.md` (Rust section), `docs/rust-quality-bar.md` (the fuzzer decision), `fixtures/shell/README.md` and `fixtures/README.md` (`analysis.json`), and the AGENTS.md CI table and key files.

## Rejected

- **brush-parser in this PR:** blocked by `deny.toml`, and changing that is a separate gates decision.
- **A hand-written parser:** size and risk.
- **Rust tests spawning Bun:** not deterministic once TypeScript is deleted.
- **A stable, non-coverage-guided loop:** #455 already admitted cargo-fuzz, and that loop does not guide on coverage.
- **Mirroring unbash's partial answer for deep nesting:** it can let a command go unchecked.
