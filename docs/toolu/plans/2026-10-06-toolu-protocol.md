# toolu-protocol: host payloads, events, decisions and encoders — Plan

**Date:** 2026-10-06   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-06-toolu-protocol-design.md   **Topic:** #413. Rust payloads, `NormalizedEvent`, `Decision`, encoders and `run_hook` in `crates/core/protocol`.

## Evidence and approach

The spec decides the design, and the brainstorm records the Jev calls. The port sources are:
- `packages/toolu-core/src/host/host-encode.ts` and `host-events.ts`;
- `packages/toolu-core/src/decision/decision.ts`, `policy/policy.ts` and `events/events.ts`;
- `packages/toolu-core/src/dispatch/dispatch-context.ts` (`toolEvent`);
- `plugins/toolu/hooks/src/session-start.ts` (`sessionEvent`);
- `plugins/toolu/hooks/src/pre-tools/hook-main.ts`.

The existing crate already holds `exit::Exit`, `host::Host`, `stdin::read_all`, `event::is_enforcing` and `launcher`. The test wiring follows `crates/cli/tests` (`#[path = "helpers/…"]`) and `crates/cli/tests/helpers/schema.rs` (`jsonschema`). The TypeScript side of the shared fixture uses `readCaseFile` from `@toolu/conformance/harness/json-cases`, as `host-roots.test.ts` does.

Prototype findings, from a scratch crate on 1.99.0:
- `deny_unknown_fields` on an internally tagged enum does not reject extras on a unit variant. That is why `Decision` and `NormalizedEvent` deserialize through a flat strict wire struct with `TryFrom`.
- A lenient `deserialize_with` works next to `#[serde(flatten)] rest`.
- serde_json escapes control characters as `\u00XX`, leaves DEL, U+2028 and non-ASCII raw, and writes struct fields in declaration order. Those are `JSON.stringify`'s bytes.

`fixtures/host/encode.json` was captured once from the TypeScript encoder by a scratch script outside the repository: 299 cases, 238 `stdout`, 57 `callback` and 4 `error`. That script is not in the test path.

`cargo` on this host must be the rustup proxy (`$HOME/.cargo/bin`); `/usr/bin/cargo` is 1.93. Every check therefore sets `PATH`.

## Workstream summary

Text, event and native names → decision → normalized event → payloads → normalization → encoders → shared encode fixture (Rust and TypeScript) → payload fixtures → `run_hook` → dependency and docs → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "events",
    "title": "Text newtype, HostEvent/EventKind and the native event-name tables with unit tests ported from host-events.test.ts",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-protocol --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t text::tests:: && t event::tests:: && t native::tests::",
    "ac_refs": [
      "AC-8"
    ],
    "paths": [
      "crates/core/protocol/",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "host-events.test.ts cases: PascalCase rows, Cursor camelCase and aliases, Hermes snake_case, OpenCode dotted names; boundary: unknown, empty and foreign names, an empty Text",
    "model": "inherit"
  },
  {
    "id": "decision",
    "title": "Decision, FailureCode, GateClass, strict wire and merge precedence with unit tests",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-protocol --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t decision::tests::",
    "ac_refs": [
      "AC-4",
      "AC-7"
    ],
    "depends_on": [
      "events"
    ],
    "paths": [
      "crates/core/protocol/",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "policy.test.ts and decision.test.ts cases; boundary: {\"kind\":\"allow\",\"x\":1}, an empty reason, an unknown kind or code, an empty list, deny/post_block ties",
    "model": "inherit"
  },
  {
    "id": "normalized",
    "title": "NormalizedEvent with Session/Tool, the flat strict wire (normalized/wire.rs) and TryFrom per type, with round-trip and rejection tests",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-protocol --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t normalized::tests:: && t normalized::wire::tests:: && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy --only guardrails --only unused-pub",
    "ac_refs": [
      "AC-4"
    ],
    "depends_on": [
      "events"
    ],
    "paths": [
      "crates/core/protocol/",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "events.test.ts documents for all 11 types; boundary: futureField, command on tool/pre, missing sessionId, empty toolName, missing toolInput, null toolOutput",
    "model": "inherit"
  },
  {
    "id": "payload",
    "title": "Open host payload structs (HookPayload, CursorPayload, HermesPayload, OpencodePayload), LenientString, Payload::parse",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-protocol --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t payload::tests:: && t payload::hook::tests:: && t payload::cursor::tests:: && t payload::hermes::tests:: && t payload::opencode::tests::",
    "ac_refs": [
      "AC-4"
    ],
    "depends_on": [
      "events"
    ],
    "paths": [
      "crates/core/protocol/",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "Claude PreToolUse payload with future_field; Cursor beforeMCPExecution with string tool_input; the Hermes docs envelope; OpenCode {input, output}; boundary: {\"session_id\":7}, a duplicate key, a lone surrogate, [] and 7",
    "model": "inherit"
  },
  {
    "id": "normalize",
    "title": "Payload::normalize into NormalizedEvent (toolEvent port, SessionStart source chain, Cursor mapping, Hermes envelope, OpenCode None)",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-protocol --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t normalize::tests::",
    "ac_refs": [
      "AC-1"
    ],
    "depends_on": [
      "normalized",
      "payload"
    ],
    "paths": [
      "crates/core/protocol/",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "Bash with a command, Shell, apply_patch; boundary: Bash with an empty or non-string command, tool_response vs tool_output vs null, sources compact/resume/clear/7/false+event",
    "model": "inherit"
  },
  {
    "id": "encode",
    "title": "supports_ask, degrade_ask, decision normalization and the Claude/Codex, Cursor, Hermes and OpenCode encoders with unit tests ported from host-encode.test.ts",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-protocol --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t encode::tests:: && t encode::hook::tests:: && t encode::cursor::tests:: && t encode::hermes::tests:: && t encode::opencode::tests:: && t encode::object::tests:: && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy --only guardrails --only unused-pub",
    "ac_refs": [
      "AC-8"
    ],
    "depends_on": [
      "decision"
    ],
    "paths": [
      "crates/core/protocol/",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "host-encode.test.ts cases; boundary: the fail-closed invariants (runtime failure, post_block before an action, residual ask) and the missing native events",
    "model": "inherit"
  },
  {
    "id": "encode-fixture",
    "title": "Shared fixtures/host/encode.json: TypeScript consumer test, Rust black-box byte test asserting 299/238/57/4 and Codex schema validation per event; index.json, README and inventory test count",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --test encode_fixture 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && bun test packages/toolu-core/src/host/__tests__/host-encode-fixture.test.ts tooling/src/__tests__/check-fixture-inventory.test.ts && bun run tooling/src/check-fixture-inventory.ts",
    "ac_refs": [
      "AC-2",
      "AC-3"
    ],
    "depends_on": [
      "encode"
    ],
    "paths": [
      "crates/core/protocol/",
      "Cargo.toml",
      "Cargo.lock",
      "fixtures/host/",
      "fixtures/codex-hook-schemas/",
      "fixtures/index.json",
      "fixtures/README.md",
      "packages/toolu-core/src/host/",
      "packages/toolu-core/src/decision/",
      "tools/toolu-conformance/src/harness/json-cases.ts",
      "tooling/src/check-fixture-inventory.ts",
      "tooling/src/__tests__/check-fixture-inventory.test.ts"
    ],
    "input": "fixtures/host/encode.json (299 cases) and fixtures/codex-hook-schemas/*.output.schema.json; boundary: the escaping cases, the 4 wiring-error cases, silent outputs skipped by schema validation, PreCompact/SessionEnd without a schema",
    "model": "inherit"
  },
  {
    "id": "payload-fixtures",
    "title": "Black-box test: every payload fixture deserializes for its host and normalizes to the expected kind, asserting per-family case counts",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --test payload_fixtures 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'",
    "ac_refs": [
      "AC-1"
    ],
    "depends_on": [
      "normalize"
    ],
    "paths": [
      "crates/core/protocol/",
      "Cargo.toml",
      "Cargo.lock",
      "fixtures/gates/",
      "fixtures/ast-grep/",
      "fixtures/quality/",
      "fixtures/opencode/",
      "fixtures/portable-core/",
      "tools/toolu-opencode/contract/captures/tool-calls.jsonl"
    ],
    "input": "fixtures/gates/*.json stdin, descriptors and bash commands; ast-grep nudge/savings; quality steps; opencode permission-evaluate requests; portable-core protected-files-pre; the 10 pinned-host OpenCode captures; boundary: {not json, empty stdin, {\"source\":7}, {\"source\":false,\"event\":\"resume\"}, object and numeric prompts",
    "model": "inherit"
  },
  {
    "id": "run-hook",
    "title": "run_hook/run_hook_io with Reply/Raw/HookError, the class table and the quiet panic hook; unit tests on in-memory streams and a harness=false real-process test",
    "check": "t() { PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-protocol --lib -- \"$@\" 2>&1 | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t hook::tests:: --skip payload:: --skip encode:: && t hook::panic::tests:: && PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --test run_hook && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy --only guardrails --only unused-pub",
    "ac_refs": [
      "AC-5",
      "AC-6"
    ],
    "depends_on": [
      "encode"
    ],
    "paths": [
      "crates/core/protocol/",
      "Cargo.toml",
      "Cargo.lock"
    ],
    "input": "a closure panicking with boom on PreToolUse, PostToolUse and SessionStart in a real child; a PreToolUse deny, a PostToolUse block and a SessionStart advisory written to a really closed stdout pipe; boundary: HookError (incl. in_part dispatcher), invalid UTF-8 stdin, Cursor permission/evaluate, a refusing Write, Raw with Exit::Usage, a panic outside run_hook (exit 101)",
    "model": "inherit"
  },
  {
    "id": "deps-docs",
    "title": "Dependency check (serde and serde_json only), crate doc, AGENTS.md key-files row, fixtures README totals",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo metadata --format-version 1 --no-deps | jq -e '[.packages[] | select(.name==\"toolu-protocol\") | .dependencies[] | select(.kind==null) | .name] | sort == [\"serde\",\"serde_json\"]' && row=$(grep -F 'crates/core/protocol/src/hook.rs' AGENTS.md) && for w in payload normalized decision encode run_hook host/encode.json; do printf '%s' \"$row\" | grep -qF \"$w\" || exit 1; done && for w in payload normalized decision encode run_hook; do grep -qE \"^//!.*$w\" crates/core/protocol/src/lib.rs || exit 1; done && grep -q 'host/encode.json' fixtures/README.md && grep -q '1,491' fixtures/README.md && grep -q '20 suite groups' fixtures/README.md",
    "ac_refs": [
      "AC-9"
    ],
    "depends_on": [
      "run-hook",
      "encode-fixture",
      "payload-fixtures"
    ],
    "paths": [
      "crates/core/protocol/Cargo.toml",
      "crates/core/protocol/src/lib.rs",
      "Cargo.lock",
      "AGENTS.md",
      "fixtures/README.md"
    ],
    "input": "cargo metadata of the workspace; boundary: a dev-dependency (jsonschema) must not count as a normal dependency",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full Rust gate and the TypeScript checks this change touches",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --base origin/main && bun run tooling/src/check-fixture-inventory.ts && bun test tooling/src/__tests__/check-fixture-inventory.test.ts packages/toolu-core/src/host/__tests__",
    "ac_refs": [
      "AC-10"
    ],
    "depends_on": [
      "deps-docs"
    ],
    "input": "the whole branch; boundary: coverage floor 90% for toolu-protocol, zero jscpd clones, a fixture case missing from index.json",
    "model": "inherit"
  }
]
```

## Critical files

- Create:
  - `crates/core/protocol/src/{text,native,decision,normalized,payload,normalize,encode,hook}.rs`, with `src/tests/{text,native,decision,normalized,payload,normalize,encode,hook}_test.rs`;
  - `src/normalized/wire.rs`, `src/payload/{hook,cursor,hermes,opencode}.rs`, `src/encode/{hook,cursor}.rs` and `src/hook/panic.rs`, each with its `tests/<module>_test.rs`;
  - `crates/core/protocol/tests/{payload_fixtures,encode_fixture,run_hook}.rs` and `tests/helpers/{repo,render}.rs`;
  - `fixtures/host/encode.json` and `packages/toolu-core/src/host/__tests__/host-encode-fixture.test.ts`.
- Modify:
  - `crates/core/protocol/src/{lib,event}.rs` and `src/tests/event_test.rs`;
  - `crates/core/protocol/Cargo.toml` (`serde`, `serde_json`, dev `jsonschema`, `[[test]] run_hook harness = false`) and `Cargo.lock`;
  - `fixtures/index.json`, `fixtures/README.md`, `tooling/src/__tests__/check-fixture-inventory.test.ts` (1192 to 1491) and `AGENTS.md`.

## Verification

`cargo xtask gate --base origin/main` is the whole Rust bar: fmt, clippy, guardrails, layers, coverage at 90% for this crate, jscpd, unused pub, docs-cli and cli-compat. Then:
- `bun test packages/toolu-core/src/host/__tests__`: the TypeScript side of the shared fixture;
- `bun run tooling/src/check-fixture-inventory.ts`;
- `bun run test`, the TypeScript gate. Its known environment failures on this root host (memory `be52369e`) are compared against a clean `origin/main` worktree before any of them is taken as a regression.

The real inputs are the committed fixtures and a real child process on real stdio. Failure and boundary checks are listed per step. The docs synced are `AGENTS.md`, `fixtures/README.md`, `fixtures/index.json` and the crate doc.

## Delivery

1. Scoped commits on `feat/413-toolu-protocol-host-payloads-events`: the crate, the fixture and its TypeScript consumer, then docs. Use a conventional subject; the PR is squash-merged.
2. `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/plan-ledger.js" run docs/toolu/plans/2026-10-06-toolu-protocol.md --verify` over the whole branch diff.
3. `toolu-review:review`, recording the version 2 push-review state.
4. `bun "$TOOLU_PLUGIN_ROOT/hooks/dist/verdict.js" status` must report `overall: ready`.
5. Fetch and rebase on `origin/main` if it moved, then re-run the affected checks.
6. Push and open the PR against `main`. Its title is `feat(protocol): host payloads, events, decisions and output encoders (#413)`, and its body starts with `Closes Falconiere/toolu#413` and `Part of Falconiere/toolu#402`.
7. Report `pr-open`, then `babysit`, and hand off to `pr-babysit:babysit`.

`TOOLU_PLUGIN_ROOT` is `/root/.claude/plugins/cache/toolu/toolu/7.11.0`, the enabled user install that holds `plan-ledger.js` and `verdict.js`.

## Plan review

- **Round 1: Needs changes.** Three should-fixes:
  - the `--lib` filters matched zero tests;
  - the loops over fixtures could pass with no iterations;
  - the step `paths` were under-declared.

  Considers covered the shared helper paths, the docs checks, an earlier clippy and guardrails run, the boundary inputs and the critical-files list. Fixes:
  - anchored per-module filters that require at least one passing test;
  - tests that assert case counts;
  - each Rust step declares the whole crate plus `Cargo.toml` and `Cargo.lock`, and `gate` declares no paths;
  - per-word docs greps;
  - `gate --only fmt clippy guardrails unused-pub` on `normalized`, `encode` and `run-hook`;
  - boundary inputs on every step.
- **Round 2: Needs changes.** One should-fix: `hook::tests::` is a substring of `payload::hook::tests::` and `encode::hook::tests::`. Fix: the helper passes every argument through, and `run-hook` adds `--skip payload:: --skip encode::`. A consider was also taken: the crate-doc check now matches `//!` lines.
- **Round 3: Approved.** Both fixes were confirmed under sh, and the run-hook and deps-docs checks are red until their work lands.

## Deviations

- **The encoders write JSON with an ordered writer (`encode/object.rs`), not `Serialize` structs.** `serde_json::to_string` returns a `Result` that cannot fail for these shapes. The quality bar forbids `unwrap`, and falling back to `""` would turn an impossible error into a silent allow. The writer escapes every string through `serde_json::Value`'s infallible `Display`, so the bytes are the same.
- **`encode/hermes.rs`, `encode/opencode.rs` and `encode/object.rs` are their own modules, each with a rule-7 test file.** This keeps `src/tests/encode_test.rs` under the 300-line limit. The `encode` step's check now requires each module's tests too.
