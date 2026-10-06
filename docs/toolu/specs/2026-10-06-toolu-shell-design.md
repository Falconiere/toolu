# toolu-shell: Bash/Shell command analysis in Rust — Design

**Date:** 2026-10-06   **Status:** Approved   **Author:** epic worker (Claude Code)   **Topic:** #416. Port `@toolu/core/shell` to `crates/core/shell` (`toolu-shell`). For every fixture input it gives the same answers: what a command line runs, what it writes, its git invocations, and whether an exit status is observable. It is fuzzed in CI.

Brainstorm: `docs/toolu/brainstorms/2026-10-06-toolu-shell.md`. Builds on #455 (the quality bar and the admitted `fuzz/` layout), #408 (the shell fixtures and `unbash-baseline.json`) and #413 (the shared-fixture precedent, `fixtures/host/encode.json`).

## Problem

The pre-tool and post-tool gates that #418–#423 port to Rust all ask one question first: what does this Bash/Shell command run, and what does it write? Today `@toolu/core/shell` answers it over unbash 4.0.11. `crates/core/shell` is an empty skeleton. Without a Rust analysis that matches the TypeScript one input for input, each port would read commands differently. A guardrail could then allow what it denies today, for example `bash -c "node -e …"`, a write to `.env` through `>.env`, or a push hidden in `$(…)`.

## Non-Goals

1. **No brush-parser.** Every release from 0.2.11 to 0.4.0 fails `cargo deny` under `deny.toml`. It brings syn 2 next to the workspace's syn 3, duplicate darling and hashbrown versions, and `foldhash`, whose Zlib licence is not allowed. `deny.toml` forbids exception entries, and gate data changes only in its own `chore(gates):` PR. tree-sitter-bash, which the issue names as the recovery parser, parses every fixture input and recovers partial trees (brainstorm, Jev 0.99). brush-parser can come back as a first parser after such a PR admits it. The PR records this decision.
2. No consumer is wired. The gates (#419–#422), the engine's per-event cache (`shellAnalysisOf`, #418), and `detect`'s `pushTargetRoot`/`pushTargetBranch`, which spawn git, stay in TypeScript. Only their fixture outcomes are reproduced in tests.
3. No gate data changes: `[workspace.lints]`, `clippy.toml`, `rustfmt.toml`, `deny.toml`, `rules.json`, `jscpd.json` and `lang.rust` stay as they are. Registration data only grows: `coverage-floor.json` and `inventory.json` stay unchanged unless the gate asks for a row.
4. No TypeScript behaviour changes. `packages/toolu-core/src/shell/*.ts` stays as is. Only a fixture test and a projection helper are added under `__tests__/`.
5. Error message text and error offsets are parser-specific and are not part of parity. The oversize message, which a gate shows, is.

## Architecture

**Parser.** `tree-sitter` 0.24.7 with `tree-sitter-bash` 0.23.3, which is language ABI 14 (`[workspace.dependencies]`). Only `toolu-shell` may link them, under the `shell-parser` capability in `rules.json`.

The versions are forced by `deny.toml`. Every `tree-sitter` from 0.25 to 0.27 build-depends on `serde_json` with `preserve_order`, which pulls in `indexmap` and then `hashbrown`. In this workspace `hashbrown` is unified with default features, because the `jsonschema` dev-dependency's `referencing` enables them. That brings `foldhash`, licensed Zlib, onto a non-dev path, and `cargo deny` licences fails, as measured during execution. 0.24.7 has no such build dependency, and `tree-sitter-bash` 0.25 needs ABI 15, which 0.24 cannot load. The 0.23.3 grammar was re-probed: it gives the same result on all 203 fixture inputs and the same construct trees, except `(( … ))`, which it reads as a `command` whose name is an `arithmetic_expansion` (see the table).

One `Parser` is created per `analyze` call and reused for every nested re-parse. Before each parse, `set_timeout_micros` is set to what remains of `PARSE_BUDGET` (1 s, wall clock, shared by every nested parse), and a parse that returns `None` was cancelled. The measured worst case that made this necessary is 1 MiB of `${`, which takes 4.2 s in release. Real commands parse in microseconds, and a cancelled parse is `unknown`, which the gates treat as "ask". Timing noise can therefore only make an answer more conservative.

**Walk.** The walk ports `shell-walk.ts`, mapping tree-sitter-bash's node kinds to unbash's semantics. It holds one context per level: `source`, `origin`, `depth` (the `bash -c`/`eval` level), `proves`, `pipeline` and `nesting`. Each construct maps as follows:

| unbash node | tree-sitter-bash node(s) | Handling |
|---|---|---|
| Statement list | `program`, compound bodies, `do_group`, `ERROR` | `walk_list`: only the last statement keeps `proves`, and only when it is not backgrounded. A statement is backgrounded when the terminator token after it is `&` |
| AndOr | left-recursive `list` (`&&`, `\|\|`) | flattened iteratively into elements and operators. `proves` needs `&&` on both sides |
| Pipeline | `pipeline` (`\|`, `\|&`), `negated_command` | positions `{index, size}`. Only the last element of a non-negated pipeline proves |
| Command | `command`, `declaration_command`, `unset_command`, and a `variable_assignment(s)` statement, which is an empty command with prefix only | `emit_command`, below |
| Command `[ … ]` | `test_command` whose first token is `[` | a command whose words are `[`, each word and operator token inside in source order, then `]`, as unbash reads `[` |
| Coproc | a `command` whose static first word is `coproc` | the remaining words are the command, walked with `proves = false`, as unbash's `Coproc` |
| Statement redirects | `redirected_statement` around a compound body | `compound_redirects` |
| If / While / For / Select / ArithmeticFor / Case | `if_statement`, `while_statement`, `for_statement` (which is also `select`), `c_style_for_statement`, `case_statement` | bodies walked with `proves = false`. Words and conditions are scanned for nested scripts |
| Function | `function_definition` | body walked with `origin = function`, `proves = false`. Its `redirect` children go to `compound_redirects` |
| Subshell / BraceGroup | `subshell`, a `compound_statement` whose first token is `{` | body walked with the same `proves` |
| TestCommand `[[ … ]]` / ArithmeticCommand `(( … ))` | `test_command` whose first token is `[[`; a `command` whose `command_name` is an `arithmetic_expansion` written `((` (0.23.3), or a `compound_statement` whose first token is `((` (newer grammars) | scanned for nested scripts only |

**Mapping notes.** These were measured with tree-sitter-bash 0.25.1 and re-measured with 0.23.3, against the TypeScript answers.

- **Heredoc continuation.** After `heredoc_start`, the rest of the line is nested inside the `heredoc_redirect` node:
  - further redirects (`redirect:` fields) belong to the same command;
  - a `pipeline` child continues the pipeline (`cat <<EOF | bash`);
  - an operator plus a `right:` child continues the and-or list (`cat <<'EOF' >f && git push`).

  The walk rebuilds the logical pipeline and and-or list from these, so the positions, `proves` and the order of commands match unbash. For `cat <<EOF | bash` that is: `cat` at pipeline 0 of 2, `bash` at 1 of 2, then `bash`'s unknown stdin script.
- **Line continuation.** tree-sitter-bash treats `\` plus newline as whitespace, but bash removes it inside a word: `node -\<newline>e x` runs `node -e x`. Adjacent word nodes separated by exactly `\<newline>` or `\<CR><LF>` are joined into one word before resolution, so a deny rule still matches. `text` keeps the source as written.
- **Glob detection.** tree-sitter-bash splits `.en[v]` into several `word` nodes. Consecutive unquoted literal parts are merged before the `*`/`?`/`[…]` test, which matches unbash's single `Literal` part.

Rules that cut across these nodes:

- **Nested scripts.** `command_substitution` (`$(…)` and backticks), `process_substitution`, `arithmetic_expansion` operands, parameter-expansion operands, assignment values, redirect targets and unquoted `heredoc_body` contents are walked as nested scripts. They get `origin = substitution`, `proves = false` and position `ALONE`, and a nested command is emitted before the command that contains it.
- **Arithmetic and test expressions.** Their values are never static. Their subtrees are searched iteratively with a `TreeCursor` for nested substitutions, so a long chain like `$((1+1+…))` or `[[ a && b && … ]]` uses no stack.
- **`ERROR` nodes.** Every recognised child of an `ERROR` node is walked as a statement, so the commands around a syntax error are reported.
- **Nesting limit.** Each nested script or compound body adds 1 to `nesting`. Entering a level beyond `MAX_NESTING` (64) emits nothing more. It records a `nesting` error and marks the analysis `unknown`. tree-sitter's parser and tree deletion are iterative, which was measured with 100,000 nested `$(…)`.

**Simple commands** (`emit_command`):
- Words are resolved by `words.rs`, the port of `scanWord`. A word is static only when every part is literal. Quote removal follows bash: backslash escapes outside quotes, `\$ \` \" \\ \newline` inside double quotes, raw single quotes, and ANSI-C `$'…'` decoding.
- `"$(cat <<'EOF' … EOF)"` resolves to the heredoc body without trailing newlines (`heredocCat`).
- An unquoted word with `*`, `?` or `[…]` has value `None` and keeps its `pattern`. `text` is the word with quotes removed and expansions left as written.
- Wrappers are peeled with the `WRAPPERS` option tables of `shell-argv.ts`.
- `bash|sh|zsh|dash|ksh -c STRING`, a shell on a static stdin heredoc or herestring, and `eval ARGS` are re-parsed up to `MAX_RUN_DEPTH` (4). A dynamic or too-deep string is an unknown command (`argv: [None]`).
- `xargs` appends `None` and turns `exitProves` off.

**Errors and unknown.** `analyze` never panics: clippy denies `unwrap` and indexing in `src`, and every slice is a `get`. Errors come from:
- `ERROR` and `MISSING` nodes, in the line and in every re-parsed string;
- the nesting limit;
- a cancelled parse.

A line with errors has every `exitProves` false. It is `unknown` when it has errors and no command, when the parse was cancelled, or when nesting overflowed. Input over `MAX_SHELL_INPUT` (1,048,576 UTF-16 code units, counted as TypeScript counts `length`) is not parsed. It reports `oversize: N characters exceeds the 1048576 cap`, the TypeScript message, and is `unknown`.

**Git, writes and rules** port `shell-options.ts`, `shell-git.ts`, `shell-writes.ts` and `shell-rules.ts` line for line: getopt-style `parse_args`, `git_invocation` with the `-C` chain, `runs_git_subcommand`, `push_targets` with the refspec destination, `commit_messages`, `write_targets` (redirects, `tee`, `sed -i`, `perl -i`, `cp`/`mv`/`install` with `DEST/basename(SRC)`, `dd of=` and python `open(…)`), and `matches_rule`. TypeScript's regular expressions with a backreference (the Python string literal) are matched by hand. The rest use hand-written scanners, which add no new dependency.

**Parity oracle.** `fixtures/shell/analysis.json` holds TypeScript's projected analysis of every input in `unbash-baseline.json`, in the same order. It was captured once from `projectAnalysis` in `packages/toolu-core/src/shell/__tests__/parity-helpers.ts`. `analysis-fixture.test.ts` requires the inputs to equal the baseline's and TypeScript to reproduce every `expect`. `crates/core/shell/tests/analysis_fixture.rs` requires Rust to reproduce every `expect`, or the case's `rust.expect` where `rust.reason` records an intended difference. A second Rust test, `fixture_cases.rs`, replays every case of `bats-parity.json` and `issue-283.json` through the crate's API (Interfaces).

**Fuzzing.** `crates/core/shell/fuzz/` is the package #455 admitted: `Cargo.toml` with its own `[workspace]`, a nightly `rust-toolchain.toml`, `fuzz_targets/`, and `.gitignore` covering `target`, `corpus`, `artifacts` and `Cargo.lock`. It has two targets, matching the issue's "both paths" now that there is one parser:
- `analyze`: arbitrary bytes, as UTF-8, go through `analyze`, then `write_targets`, `push_targets`, `runs_git_subcommand` and `commit_messages`. This fuzzes tree-sitter's C parser, its error recovery and the walk.
- `nested`: the bytes choose and nest fragments (`$(`, backticks, `"`, `'`, `<<EOF`, `bash -c '`, `eval "`, `{`, `(`, `if`, `case`, `[[`, `$((`, `${x:-`, redirects and wrappers). This fuzzes the re-parse, the nesting limit and the word decoder past what random bytes reach.

CI has two parts:
- **Per PR:** a `fuzz` job in `tests.yml`, in path group `rust` and among the `typescript` aggregate's needs. It installs nightly and cargo-fuzz, copies the root `Cargo.lock` into `fuzz/` to pin the shared versions, seeds a corpus from the fixture inputs, and runs each target for 60 s with `-timeout=10`.
- **Scheduled:** `.github/workflows/fuzz.yml` runs daily and on `workflow_dispatch`, 30 minutes per target, and uploads the artifacts when it fails.

**Musl.** `rust-musl` builds the whole workspace, and cc-rs then needs a musl C compiler for tree-sitter. Measured locally: "failed to find tool x86_64-linux-musl-gcc". The job installs `musl-tools` and sets `CC_<target>=musl-gcc`.

## Interfaces / Schema

```rust
// toolu_shell (crate root)
pub const LAYER: &str = "shell";
pub const MAX_SHELL_INPUT: usize = 1_048_576;   // UTF-16 code units
pub const MAX_RUN_DEPTH: usize = 4;            // bash -c / eval levels
pub const MAX_NESTING: usize = 64;             // nested scripts and compound bodies
pub const PARSE_BUDGET: std::time::Duration;   // 1 s per analysis
pub fn analyze(source: &str) -> ShellAnalysis;

// toolu_shell::analysis — the records (shell-types.ts)
pub struct ShellAnalysis { pub source: String, pub commands: Vec<ShellCommand>,
  pub compound_redirects: Vec<ShellRedirect>, pub errors: Vec<ShellError>, pub unknown: bool }
pub struct ShellCommand { pub words: Vec<Option<String>>, pub argv: Vec<Option<String>>,
  pub patterns: Vec<Option<String>>, pub texts: Vec<String>, pub wrappers: Vec<String>,
  pub redirects: Vec<ShellRedirect>, pub pipeline: PipelinePosition, pub exit_proves: bool,
  pub origin: CommandOrigin, pub depth: usize, pub text: String }
pub struct ShellRedirect { pub operator: RedirectOperator, pub fd: Option<u32>,
  pub target: Option<String>, pub pattern: Option<String>, pub text: String,
  pub heredoc: Option<Heredoc> }
pub struct Heredoc { pub content: Option<String>, pub quoted: bool }
pub enum RedirectOperator { Out, Append, In, Heredoc, HeredocStrip, HereString, ReadWrite,
  DupOut, DupIn, Clobber, OutErr, AppendOutErr }    // as_str(): ">" ">>" "<" "<<" "<<-" "<<<" "<>" ">&" "<&" ">|" "&>" "&>>"
pub struct PipelinePosition { pub index: usize, pub size: usize }
pub enum CommandOrigin { Line, Substitution, Shell, Eval, Function }  // as_str()
pub struct ShellError { pub message: String, pub pos: usize /* byte offset */, pub origin: CommandOrigin }
pub enum Tristate { Yes, No, Unknown }

// toolu_shell::git (shell-git.ts)
pub struct GitInvocation<'a> { pub command: &'a ShellCommand, pub subcommand: Option<&'a str>,
  pub args: &'a [Option<String>], pub c_chain: Vec<Option<&'a str>> }
pub enum Refspec<'a> { Absent, Dynamic, Static(&'a str) }   // TS undefined / null / string
pub struct GitPush<'a> { pub invocation: GitInvocation<'a>, pub refspec: Refspec<'a>, pub destination: Option<String> }
pub fn git_invocation(command: &ShellCommand) -> Option<GitInvocation<'_>>;
pub fn runs_git_subcommand(analysis: &ShellAnalysis, sub: &str) -> Tristate;
pub fn push_targets(analysis: &ShellAnalysis) -> Vec<GitPush<'_>>;
pub fn commit_messages(invocation: &GitInvocation<'_>) -> Vec<Option<String>>;

// toolu_shell::writes (shell-writes.ts)
pub enum WriteVia { Redirect, Tee, Sed, Perl, Cp, Mv, Install, Dd, Python }  // as_str()
pub struct WriteTarget<'a> { pub path: Option<String>, pub pattern: Option<String>,
  pub text: String, pub via: WriteVia, pub command: Option<&'a ShellCommand> }
pub fn write_targets(analysis: &ShellAnalysis) -> Vec<WriteTarget<'_>>;

// toolu_shell::rules (shell-rules.ts)
pub fn matches_rule(command: &ShellCommand, rule: &str) -> bool;
```

Private modules, each at most 300 code lines and paired with its `src/tests/<module>_test.rs`:
- `parse` (tree-sitter, budget, errors);
- `walk` (statements);
- `command` (simple commands);
- `words` and `words/quote` (values, quote removal);
- `heredoc`;
- `argv` (wrappers, run targets);
- `options` (getopt);
- `writes/copy` and `writes/python`.

A module that would pass 300 code lines is split into a child module, such as `walk/compound`, at the same layer depth. Both the layer-depth limit (3 under `src`) and the size limit hold.

`fixtures/shell/analysis.json`:

```json
{ "version": 1, "parser": "unbash@4.0.11", "cases": [
  { "input": "echo x >.env",
    "expect": {
      "unknown": false, "errored": false,
      "commands": [{ "words": ["echo","x"], "argv": ["echo","x"], "patterns": [null,null],
        "texts": ["echo","x"], "wrappers": [],
        "redirects": [{ "operator": ">", "fd": null, "target": ".env", "pattern": null, "text": ".env", "heredoc": null }],
        "pipeline": { "index": 0, "size": 1 }, "exitProves": true, "origin": "line", "depth": 0,
        "text": "echo x >.env" }],
      "compoundRedirects": [],
      "writes": [{ "path": ".env", "pattern": null, "text": ".env", "via": "redirect", "command": 0 }],
      "git": [], "pushes": [], "commitMessages": [],
      "runs": { "push": "no", "commit": "no" } },
    "rust": { "expect": { … }, "reason": "…" } } ] }
```

The fields:
- `git[i]` is `{command, subcommand, args, cChain}` for every command with a git invocation.
- `pushes[i]` is `{command, refspec?, destination}`. A missing `refspec` key means absent, and `null` means dynamic.
- `commitMessages` holds one array per `git commit` invocation.
- `command` fields are indexes into `commands`, or `null` for a compound redirect.

## Failure modes and edge cases

| Input | Observable result |
|---|---|
| `""`, `"   "` | no commands, no errors, `unknown: false` (TypeScript's answer) |
| `git push origin main; echo "unterminated` | both commands reported, `errors` non-empty, every `exitProves` false, `runs_git_subcommand(push) == Yes` |
| `)`, `if` | no command, errors, `unknown: true` |
| `"$(".repeat(10_000) + "node -e x" + ")".repeat(10_000)` | no stack overflow on a 2 MiB test thread. `unknown: true` with a `nesting` error, so a gate asks rather than allows |
| `$((1+1+…))` × 100,000, `true && … && true` × 100,000 | iterative, no overflow. The first is not unknown, and the second keeps every command |
| More than 1 MiB of UTF-16 units | not parsed. `unknown`, with the TypeScript oversize message |
| 1 MiB of `${` | cancelled at `PARSE_BUDGET`. `unknown` with a `parser: … budget` error |
| `bash -c "$CMD"`, `curl … \| bash`, recursion past depth 4 | an unknown command, `argv: [None]` |
| `$g push`, `git $(echo push)` | `runs_git_subcommand` is `Unknown` |
| Non-UTF-8 input | impossible through `&str`. The fuzz target skips invalid UTF-8, as the host JSON layer already decoded it |
| Concurrent calls | `analyze` keeps no global or thread-local state. Each call owns its `Parser` and deadline, so calls on several threads are independent |
| `node -\<newline>e x`, `[ -f "$x" ]`, `coproc git push`, `cat <<EOF \| bash` | TypeScript's answers (Mapping notes) |
| A crash inside tree-sitter's C code | `catch_unwind` cannot catch it. The #412 launcher maps the signal to exit 2, so the action is blocked, fail closed. Fuzzing is the guard |

## Acceptance criteria

- **AC-1:** Every case in `fixtures/shell/bats-parity.json` (186) and `fixtures/shell/issue-283.json` (49) gives TypeScript's answer, `expected` when present and `bash` otherwise, through the crate API:
  - `runs_git_subcommand(…) == Yes` for `is_git_push` and `is_git_commit`;
  - `write_targets` `path ?? text` for `bash_write_targets`;
  - the `bashCommandsDecide` rule over `matches_rule` and `text` for `bash_commands_decide`;
  - the first push's `-C` chain replayed with real `git rev-parse` in real repositories for `push_target_root`;
  - the checked-out branch, or else the first push's destination, for `push_target_branch`;
  - `commit_messages`, `commands`, `exit_proves`, and `latency` within `maxMs`.
- **AC-2:** For every one of the 203 inputs of `unbash-baseline.json`, Rust's projected analysis equals `fixtures/shell/analysis.json`'s `expect`, or its `rust.expect` when a `rust.reason` records an intended difference. TypeScript reproduces every `expect`, and the fixture's inputs equal the baseline's in order. Every intended difference is listed in `docs/shell-analysis.md` with its reason.
- **AC-3:** Malformed input still reports the commands read. `git push origin main; echo "unterminated` and `git push origin main` followed by a newline and `echo 'x` both report the push (`Yes`) with errors, and `)` and `if` are `unknown`.
- **AC-4:** `analyze` on 10,000 nested `$(…)` around `node -e x`, on a 2 MiB-stack thread, returns `unknown: true` with a nesting error and no overflow. The `bashCommandsDecide` rule from AC-1, with deny `node -e`, then answers `unknown`, never `allow`. The 100,000-term arithmetic and `&&` chains return without overflow.
- **AC-5:** `cargo +nightly fuzz build` builds both targets. A local run of each target for at least 10 minutes finds no crash, panic or timeout. The `fuzz` job runs both targets in PR CI under the `typescript` aggregate, and `fuzz.yml` schedules longer runs. The package matches #455's `fuzz/clean` fixture layout, and `cargo xtask guardrails` passes. The first scheduled run can only happen after merge, because GitHub runs schedules from the default branch. Until then the PR job and the local 10-minute runs are the evidence, and the PR says so.
- **AC-6:** In a release build, `analyze` plus `runs_git_subcommand`, `push_targets` and `write_targets` have p99 ≤ 100 µs over the 203 fixture inputs. Parse and walk are timed together with these helpers, which is stricter than parse and walk alone, as `bench:shell` times them (`cargo test --release -p toolu-shell --test latency`, run in the CI `rust` job). A debug or instrumented build asserts only a 100× smoke ceiling (10 ms), so that the coverage run on a loaded host cannot flake.
- **AC-9:** Every behaviour test of `packages/toolu-core/src/shell/__tests__/shell-{argv,git,parse,rules,walk,writes}.test.ts` has a Rust test with the same inputs and expectations: 57 tests in total. The exceptions are the TypeScript-only source scan in `shell-walk.test.ts` and `shell-event.test.ts` (the per-event cache, Non-Goal 2). The probed constructs (line continuation, `[ … ]`, `coproc`, heredoc continuations) have Rust tests with TypeScript's answers.
- **AC-7:** The issue's scenarios hold:
  - `git status && bash -c "rm -rf x" | head` yields `git status`, then `rm -rf x` (origin `shell`, depth 1), then `head`;
  - `git push origin main` plus an unterminated quote is still a push;
  - `echo x >.env` has write target `.env`.

  An input of 1,048,577 `a` characters is `unknown` with the TypeScript oversize message. 1 MiB of `${` is `unknown` within about 2 × `PARSE_BUDGET`.
- **AC-8:** `cargo xtask gate` passes with no exemption: coverage of `toolu-shell` ≥ 90%, `cargo deny` clean with the new dependencies, guardrails, layers and docs. `bun run test` keeps passing, except the known host-environment failures that are also on `origin/main` (comemory be52369e). `rust-musl` builds with tree-sitter on both musl targets in CI.

## Acceptance evidence

| AC | Real input | Expected | Check |
|---|---|---|---|
| AC-1 | `fixtures/shell/bats-parity.json`, `issue-283.json`; real git repositories under a temp dir | every case equal | `cargo test -p toolu-shell --test fixture_cases` |
| AC-2 | `fixtures/shell/analysis.json` (203 cases) | Rust projection = `expect`/`rust.expect`; TS projection = `expect`; every `rust.reason` appears in `docs/shell-analysis.md` (asserted by the TS test) | `cargo test -p toolu-shell --test analysis_fixture`; `bun test packages/toolu-core/src/shell/__tests__/analysis-fixture.test.ts` |
| AC-3 | the four malformed lines | commands plus errors; `unknown` | `cargo test -p toolu-shell --test malformed` |
| AC-4 | generated 10,000-level and 100,000-term inputs | no overflow; `unknown` for nesting | `cargo test -p toolu-shell --test limits` |
| AC-5 | fixture-seeded corpus plus random bytes | no crash | `cargo +nightly fuzz run analyze -- -max_total_time=600` and the same for `nested`; the CI `fuzz` job log |
| AC-6 | the 203 inputs × 200 rounds | p99 ≤ 100 µs | `cargo test --release -p toolu-shell --test latency -- --nocapture` |
| AC-7 | the scenario lines | as stated | `cargo test -p toolu-shell --test scenarios` |
| AC-8 | the workspace | gate green | Locally: `cargo xtask gate`, `bun run test:conventions`, the touched TypeScript suites, `check:ci-paths`, `test:docs`, and a static x86_64 musl test binary built with `musl-gcc`. The full `bun run test` is compared against an `origin/main` baseline on this host. In CI: `gate` (the full `bun run test`), `rust`, `rust-musl` on both targets and `fuzz`, all green before `ready` (babysit) |
| AC-9 | the TypeScript unit-test inputs, plus the probed constructs | the same expectations | `cargo test -p toolu-shell --lib` (the `src/tests/*_test.rs` files) |

## Documentation impact

- `docs/shell-analysis.md` gets a Rust section: the crate, the parser decision, the limits (`MAX_NESTING`, `PARSE_BUDGET`), fuzzing, and the list of intended differences.
- `docs/rust-quality-bar.md` § Fuzzing: cargo-fuzz is the chosen fuzzer, with the CI jobs.
- `fixtures/shell/README.md` and `fixtures/README.md` describe `analysis.json`.
- `AGENTS.md`: a Key files row for `crates/core/shell`, and the CI table gains `fuzz` and the scheduled `fuzz.yml`. AGENTS.md is the docs-sync surface.
- No `SKILL.md`, command or config surface changes.

## Open Questions

None blocking.
- brush-parser re-admission is a possible later `chore(gates):` PR. It does not block this one, and the owner is the epic.
- Fuzz durations of 60 s per PR and 30 min scheduled are defaults. CI time per PR is tracked by #410.

## Spec review

**Status:** Approved (2026-10-06, epic worker). Jev, on whether each issue criterion is covered by an AC with a runnable check: parity 0.88, differential 0.90, malformed 0.88, nesting 0.85, fuzz 0.95, latency 0.81, scenarios 0.97.

- Architecture: 🟡 should-fix (fixed). The walk table assumed `[ … ]` was a command, `(( … ))` a separate node and `coproc` a node. A tree-sitter-bash probe showed `test_command`, `compound_statement` and a plain command. It also showed heredoc continuations nested in `heredoc_redirect`, and that backslash-newline splits a word. Added the rows and the Mapping notes, each with TypeScript's measured answer.
- Acceptance criteria: 🟡 should-fix (fixed). The TypeScript unit tests had no Rust counterpart. Added AC-9.
- AC-4: 🔵 consider (fixed). "Nor allows the command" now names the decision rule's `unknown` verdict.
- AC-6: 🔵 consider (fixed). It now states that the timed span includes parse and walk.
- AC-5: 🔵 consider (fixed). The scheduled run cannot happen before merge. The spec now says so, and the PR will.
- Plan review, evidence (fixed): AC-8's local and CI evidence are now separate, and AC-2's documented differences are machine-checked.
