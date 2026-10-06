# Shared parity fixtures

These committed JSON files are the test inputs for the current TypeScript implementation and the Rust ports in epic #402. Add or edit a case here; the TypeScript suites load the files at runtime. There is no fixture generator in the test path. `bun run tooling/src/check-fixture-inventory.ts` compares every case name with `index.json`, and each golden harness checks that captures cover exactly its cases.

## Case files and counts

Every new named case file below has `{ "version": 1, "cases": [...] }`. Each case has a unique `name`. A suite's family schema validates its other fields before execution. The counts are runnable case records before and after extraction; the two lifecycle suites share one file. The statusline count excludes six generic CLI publishing checks supplied by `publishedCliSuite`.

| Suite | File | Before → after | TypeScript consumer | Rust consumer |
|---|---|---:|---|---|
| SessionStart lifecycle | `gates/lifecycle.json` | 50 → 50 | `plugins/toolu/hooks/src/__tests__/session-start-cases.ts` | #402 / #409 |
| UserPromptSubmit lifecycle | `gates/lifecycle.json` | 67 → 67 | `plugins/toolu/hooks/src/__tests__/user-prompt-submit-cases.ts` | #402 / #409 |
| Pre-tool modules A | `gates/pre-tool-modules-a.json` | 109 → 109 | `plugins/toolu/hooks/src/__tests__/pre-tool-modules-a-cases.ts` | #402 / #409 |
| Pre-tool modules B | `gates/pre-tool-modules-b.json` | 110 → 110 | `plugins/toolu/hooks/src/__tests__/pre-tool-modules-b-cases.ts` | #402 / #409 |
| Pre-tool modules C | `gates/pre-tool-modules-c.json` | 180 → 180 | `plugins/toolu/hooks/src/__tests__/pre-tool-modules-c-cases.ts` | #402 / #409 |
| Pre-tool edit/shell corpus | `gates/pretool-corpus.json` | 33 → 33 | `tools/toolu-conformance/src/harness/pretool-corpus.ts` | #402 / #409 |
| Post-tool corpus | `gates/posttool-corpus.json` | 16 → 16 | `tools/toolu-conformance/src/harness/posttool-corpus.ts` | #402 / #409 |
| TypeScript quality | `quality/ts.json` | 120 → 120 | `plugins/ts-quality/hooks/src/__tests__/cases.ts` | #426–#429 |
| Python quality | `quality/python.json` | 96 → 96 | `plugins/python-quality/hooks/src/__tests__/cases.ts` | #426–#429 |
| Rust quality | `quality/rust.json` | 119 → 119 | `plugins/rust-quality/hooks/src/__tests__/cases.ts` | #426–#429 |
| Ast-grep nudge | `ast-grep/nudge.json` | 38 → 38 | `plugins/ast-grep/hooks/src/__tests__/cases-nudge.ts` | #426–#429 |
| Ast-grep savings | `ast-grep/savings.json` | 35 → 35 | `plugins/ast-grep/hooks/src/__tests__/cases-savings.ts` | #426–#429 |
| Ast-grep report | `ast-grep/report.json` | 6 → 6 | `plugins/ast-grep/hooks/src/__tests__/cases-report.ts` | #426–#429 |
| Shared quality runner | `quality/runner.json` | 16 → 16 | `packages/toolu-core/src/quality/__tests__` | #459 |
| Host roots | `host/root.json` | 18 → 18 | `packages/toolu-core/src/host/__tests__/host-roots.test.ts` | #414 |
| Host output encoders | `host/encode.json` | new, 299 | `packages/toolu-core/src/host/__tests__/host-encode-fixture.test.ts` | #413 (`crates/core/protocol/tests/encode_fixture.rs`) |
| Config resolution | `config/expected.json` | new, 35 | `packages/toolu-core/src/config/__tests__/config-fixture.test.ts` | #414 (`crates/core/runtime/tests/config_fixture.rs`) |
| State | `state/cases.json` | 77 → 77 | `packages/toolu-core/src/state/__tests__` | #415 |
| Statusline states | `statusline/cases.json` | 83 → 83 | `plugins/statusline/hooks/src/__tests__` | #431 |
| OpenCode permission/evaluate | `opencode/permission-evaluate.json` | 17 → 17 | `tools/toolu-opencode/src/adapter/__tests__` | #462 |
| OpenCode lifecycle tests | `opencode/lifecycle-events.json` | 2 → 2 tests, 11 events | `tools/toolu-opencode/src/lifecycle/__tests__` | #462 |

The original 13 exported arrays contain 979 names. The six newly extracted families add 213 names, #413's host encoder suite adds 299 more, and #414's config resolution suite adds 35, for 1,526 indexed cases in 21 suite groups. OpenCode's lifecycle file records all 11 event outcomes across its two tests. The inventory checks names and counts, so swapping or dropping a case fails.

## Case fields and setup

`gates/lifecycle.json` stores the hook event, host, stdin, optional setup, and expected decision/output. The three `gates/pre-tool-modules-*.json` files store a module, host, tool call and expected gate decision. The pre-tool corpus records edit/shell tool fixtures and expected host outcomes; the post-tool corpus records post-tool payloads and checks. Both corpora use `setupByHost` when Claude and Codex need different real files or registry state. The golden captures moved to `gates/*-golden.json` and retain their recorded `base` and case-keyed `cases` fields.

`quality/{ts,python,rust}.json` records each project's files, committed files, ordered edit steps, environment variant, expected block/advisory/silent result, and case-specific golden deviation. `quality/rust-excluded.json` names two prior Bats cases that intentionally have no TypeScript equivalent. `quality/runner.json` has `run`, `edit`, and `scan` records: quality decisions and gate status, edited-file checks, and real ast-grep results plus bounded executable failure probes. The captures are `quality/{ts,python,rust}-golden.json`.

`ast-grep/{nudge,savings,report}.json` records hook payloads and expected output. Savings cases have `setup` before registry startup and `payloadSetup` after it, preserving the original timing. Named deviations live on the affected case. `ast-grep/golden.json` keeps the captured nudge and savings output and its `base` commit.

`host/root.json` holds ordered host-root calls and expected values, with real Git sandboxes for project-root cases. `host/encode.json` was captured once from `encodeDecision`. It holds:

- every host × host event × the six standard decisions;
- an `ask` from each gate class, passed through `degradeAsk`;
- seven string-escaping cases;
- the four events a host lacks.

Each case's `expect` is the exact `stdout`, the `JSON.stringify`ed OpenCode `callback`, or the wiring `error`. The TypeScript and Rust encoders must both reproduce it byte for byte. `state/cases.json` has discriminated schema, gate-file, I/O, branch, diff, concurrency and public-package cases. `statusline/cases.json` has renderer, status report, Jev readiness, SessionStart and setup cases; its `actions` or `steps` are bounded real file/Git/plugin actions. `opencode/permission-evaluate.json` has mapping, decision and evaluate integration cases. `opencode/lifecycle-events.json` maps the eleven supported, deferred or unsupported events and checks the pinned SDK hook names.

Pre-tool corpus `settings` keys are filenames within the sandbox settings directory. Statusline action `path`, `cwd`, `root`, and `target` fields use tagged `$path` values. Mutating actions reject paths that cross an existing symlink, including the read-only jscpd checkout link.

The shared harness `tools/toolu-conformance/src/harness/json-cases.ts` accepts ordered `setup` operations `write`, `remove`, `mkdir`, `symlink`, `chmod`, `git`, `config`, and `push-waiver-pend`. A setup write or symlink path is confined to the disposable sandbox. `$PROJECT`, `$ROOT`, `$HOME`, and `$HOST_STATE` expand only in path fields or tagged `{ "$path": "..." }` and `{ "$template": "..." }` values. Ordinary strings, including shell variables and code samples, stay literal. `$REPO` is allowed only as a tagged symlink target inside this checkout for a real jscpd fixture; it cannot address a write or read. Unknown operations, tokens, duplicate names, and escaping paths fail before a hook runs. Each family parser adds its own bounded fields and rejects malformed records.

## Existing contract trees

- `shell/bats-parity.json` and `shell/issue-283.json` retain the command inputs and Bash/TypeScript outcomes used by `packages/toolu-core/src/shell/__tests__`. `shell/parser-errors.json` adds two malformed inputs. `shell/unbash-baseline.json` records `{ "version": 1, "parser": "unbash@4.0.11", "cases": [{ "input": ..., "result": ... }] }` for all 203 distinct inputs. `bun run tooling/src/check-unbash-baseline.ts` checks exact coverage and parser output for #416. See [shell/README.md](shell/README.md).
- `config/*.json` are complete `toolu.config.json` envelopes, including valid examples and fail-closed inputs for `packages/toolu-core/src/config/__tests__`. `config/expected.json` is the loader golden both implementations reproduce. See [config/README.md](config/README.md).
- `portable-core/protected-files-pre.json` is a protocol v1 pre-tool event for the protected-file conformance suite (#409).
- `gate-coverage/inventory.json` is an array of hook registrations with source path, plugin, event, matcher and command/module identity, used by the gate coverage checker.
- `codex-hook-schemas/*.schema.json` are JSON Schemas for the five Codex hook command outputs, read by the host/launcher schema test.

The tooling-only overlays `tooling/fixtures/conventions`, `tooling/fixtures/guardrails`, and `tooling/fixtures/pr-babysit-herdr-smoke` stay with their TypeScript tooling until #439. They are not Rust parity contracts in #408.
