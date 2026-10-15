# Shared parity fixtures

These committed JSON files are the contracts the Rust code reproduces. Most were captured from the bash and TypeScript implementations that the Rust crates replaced (epic #402, #440). Add or edit a case here; the Rust tests load the files at runtime. There is no fixture generator in the test path. `index.json` lists every case name per suite, but no script checks it any more: each Rust replay asserts the number of cases it ran, names a mismatching case, and lists any case it deliberately skips with a reason ([docs/testing.md](../docs/testing.md)).

## Case files and counts

Every new named case file below has `{ "version": 1, "cases": [...] }`. Each case has a unique `name`. A suite's family schema validates its other fields before execution. The counts are runnable case records; the two lifecycle suites share one file.

| Suite | File | Cases | Replayed by |
|---|---|---:|---|
| SessionStart lifecycle | `gates/lifecycle.json` | 50 | `crates/cli/tests/golden_lifecycle.rs` |
| UserPromptSubmit lifecycle | `gates/lifecycle.json` | 67 | `crates/cli/tests/golden_lifecycle.rs` |
| Pre-tool modules A | `gates/pre-tool-modules-a.json` | 109 | `crates/cli/tests/golden_gates.rs` |
| Pre-tool modules B | `gates/pre-tool-modules-b.json` | 110 | `crates/cli/tests/golden_gates.rs` |
| Pre-tool modules C | `gates/pre-tool-modules-c.json` | 180 | `crates/cli/tests/golden_gates.rs` |
| Pre-tool edit/shell corpus | `gates/pretool-corpus.json` | 33 | `crates/cli/tests/golden_gates.rs` |
| Post-tool corpus | `gates/posttool-corpus.json` | 16 | `crates/core/protocol/tests/payload_fixtures.rs` |
| TypeScript quality | `quality/ts.json` | 120 | `crates/cli/tests/golden_quality.rs` |
| Python quality | `quality/python.json` | 96 | `crates/cli/tests/golden_quality.rs` |
| Rust quality | `quality/rust.json` | 119 | `crates/cli/tests/golden_quality.rs` |
| Ast-grep nudge | `ast-grep/nudge.json` | 38 | `crates/ast-grep/src/tests/`, `crates/cli/tests/ast_grep_hook.rs` |
| Ast-grep savings | `ast-grep/savings.json` | 35 | `crates/ast-grep/src/tests/savings_test.rs` |
| Ast-grep report | `ast-grep/report.json` | 6 | `crates/ast-grep/src/tests/report_test.rs` |
| Shared quality runner | `quality/runner.json` | 16 | `crates/core/engine/tests/quality_{fixture,run_fixture,scan_fixture}.rs` |
| Host roots | `host/root.json` | 18 | `crates/core/runtime/tests/host_roots_fixture.rs` |
| Host output encoders | `host/encode.json` | 241 | `crates/core/protocol/tests/encode_fixture.rs` |
| Config resolution | `config/expected.json` | 35 | `crates/core/runtime/tests/config_fixture.rs` |
| State | `state/cases.json` | 77 | `crates/core/state/tests/{state_cases,diff_cases,io_cases,gate_cases,interleave}.rs` |
| State gate-file bytes | `state/gate-bytes.json` | 16 | `crates/core/state/tests/gate_bytes.rs` |
| State edit records | `state/edit-records.json` | 42 | `crates/core/state/tests/edit_records.rs` |
| Dispatch walk | `dispatch/cases.json` | 50 | `crates/core/engine/tests/dispatch_fixture.rs` |
| Plan ledger and verdict | `ledger/cases.json` | 27 | `crates/cli/tests/ledger_cases.rs` |
| Statusline states | `statusline/cases.json` | 75 | `crates/cli/tests/golden_statusline.rs` |
| Epic hosts | `epic/hosts.json` | 61 | `crates/epic-orchestrator/src/tests/hosts_test.rs` |

## Case fields and setup

`gates/lifecycle.json` stores the hook event, host, stdin, optional setup, and expected decision/output. The three `gates/pre-tool-modules-*.json` files store a module, host, tool call and expected gate decision. The pre-tool corpus records edit/shell tool fixtures and expected host outcomes; the post-tool corpus records post-tool payloads and checks. Both corpora use `setupByHost` when Claude and Codex need different real files or registry state. The golden captures moved to `gates/*-golden.json` and retain their recorded `base` and case-keyed `cases` fields.

`quality/{ts,python,rust}.json` records each project's files, committed files, ordered edit steps, environment variant, expected block/advisory/silent result, and case-specific golden deviation. `quality/rust-excluded.json` names two prior Bats cases that are intentionally not replayed. `quality/runner.json` has `run`, `edit`, and `scan` records: quality decisions and gate status, edited-file checks, and real ast-grep results plus bounded executable failure probes. The captures are `quality/{ts,python,rust}-golden.json`.

`ast-grep/{nudge,savings,report}.json` records hook payloads and expected output. Savings cases have `setup` before registry startup and `payloadSetup` after it, preserving the original timing. Named deviations live on the affected case. `ast-grep/golden.json` keeps the captured nudge and savings output and its `base` commit.

`host/root.json` holds ordered host-root calls and expected values, with real Git sandboxes for project-root cases. `host/encode.json` was captured once from `encodeDecision`. It holds:

- every host × host event × the six standard decisions;
- an `ask` from each gate class, passed through `degradeAsk`;
- seven string-escaping cases;
- the four events a host lacks.

Each case's `expect` is the exact `stdout`, or the wiring `error`. `crates/core/protocol/tests/encode_fixture.rs` reproduces it byte for byte. `state/cases.json` has discriminated schema, gate-file, I/O, branch, diff, concurrency and public-package cases. `state/gate-bytes.json` (#415) was captured once from the TypeScript gate-file writers: record and clear sequences with the exact file bytes, clear outcome and drop log after each step. `state/edit-records.json` (#415) holds edit payloads with the kind, records and printed text TypeScript's normalizer gives them. The Rust crate reproduces them byte for byte. `dispatch/cases.json` (#418) was captured once from the TypeScript dispatcher. Each case holds a `phase` (`pre` or `post`), a `host` (`claude` or `codex`) and the `stdin`: an object sent as `JSON.stringify` prints it, or raw text. Optional fields are a project `config`, static `builtins` (a `decision` or a `throws` message), and `registry` files: `.sh` bodies run with bash, `.js` ESM sources (a `.js` module never runs now). `installed` holds the specs for Claude's install record or a ready Codex snapshot, `installedRaw` a raw install record, and `continuePostBlocks` turns on block collection. `expect` holds the exact `stdout`, `stderr` and `exitCode`. `$PROJECT`, `$HOME`, `$CODEX`, `$CONFIG` and `$LIB` expand to sandbox paths in strings, module bodies and `expect`, but never inside a longer name such as `$PROJECT_ROOT`. The Rust engine reproduces every case byte for byte. `ledger/cases.json` (#421) was captured once from the Bun bundles `plan-ledger.js` and `verdict.js`. Each case starts from a repository on `feat/x` one commit ahead of `main`, then runs its `steps` in order: `write` a file, run a `sh` script, or `run` a CLI (`plan-ledger` or `verdict`) with its argv, an optional `cwd` and `env`, and the expected `exitCode`, `stdout`, `stderr` and branch `ledger`. `$PROJECT` and `$HOME` expand to the sandbox; recorded output replaces sandbox paths with those tokens, and timestamps, durations and hashes with placeholders. `crates/cli/tests/ledger_cases.rs` replays the file against `toolu ledger`. `statusline/cases.json` has renderer, status report, Jev readiness, SessionStart and setup cases; its `actions` or `steps` are bounded real file/Git/plugin actions.

Pre-tool corpus `settings` keys are filenames within the sandbox settings directory. Statusline action `path`, `cwd`, `root`, and `target` fields use tagged `$path` values. Mutating actions reject paths that cross an existing symlink, including the read-only jscpd checkout link.

The case files accept ordered `setup` operations `write`, `remove`, `mkdir`, `symlink`, `chmod`, `git`, `config`, and `push-waiver-pend`. A setup write or symlink path is confined to the disposable sandbox. `$PROJECT`, `$ROOT`, `$HOME`, and `$HOST_STATE` expand only in path fields or tagged `{ "$path": "..." }` and `{ "$template": "..." }` values. Ordinary strings, including shell variables and code samples, stay literal. `$REPO` is allowed only as a tagged symlink target inside this checkout for a real jscpd fixture; it cannot address a write or read. Unknown operations, tokens, duplicate names, and escaping paths fail before a hook runs. Each family parser adds its own bounded fields and rejects malformed records.

## Existing contract trees

- `shell/bats-parity.json` and `shell/issue-283.json` retain the command inputs and Bash/TypeScript outcomes that `crates/core/shell/tests/fixture_cases.rs` replays. `shell/parser-errors.json` adds two malformed inputs. `shell/unbash-baseline.json` records `{ "version": 1, "parser": "unbash@4.0.11", "cases": [{ "input": ..., "result": ... }] }` for all 203 distinct inputs. `analysis_fixture.rs` checks that `analysis.json` covers every input of this baseline. `shell/analysis.json` holds the removed TypeScript analyzer's analysis of the same 203 inputs, and `crates/core/shell/tests/analysis_fixture.rs` reproduces it. See [shell/README.md](shell/README.md).
- `config/*.json` are complete `toolu.config.json` envelopes, including valid examples and fail-closed inputs for the loader tests in `crates/core/runtime/src/config/tests/`. `config/expected.json` is the loader golden. See [config/README.md](config/README.md).
- `portable-core/protected-files-pre.json` is a protocol v1 pre-tool event for the protected-file gate tests (`crates/cli/tests/pre_tool_gates.rs`).
- `gate-coverage/inventory.json` is an array of hook registrations with source path, plugin, event, matcher and command/module identity, used by the gate's coverage steps.
- `codex-hook-schemas/*.schema.json` are JSON Schemas for the five Codex hook command outputs, read by the launcher tests.
- `epic/epic248-graph.json`, `epic/epic248-body.md`, `epic/epic203-body.md` and `epic/pr290-rollup.json` are tracker captures the `toolu epic` tests read (moved from the deleted epic scripts in #435). `epic/herdr-calls.json` records the herdr 0.9.3 (protocol 22) method names and param keys every `herdr::Call` must match.
- `debug/` holds the captured outputs `toolu debug` parses (see [debug/PROVENANCE.md](debug/PROVENANCE.md)).
