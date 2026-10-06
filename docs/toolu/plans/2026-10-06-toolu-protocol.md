# toolu-protocol: host payloads, events, decisions and encoders — Plan

**Date:** 2026-10-06   **Status:** Draft   **Spec:** docs/toolu/specs/2026-10-06-toolu-protocol-design.md   **Topic:** #413. Rust payloads, `NormalizedEvent`, `Decision`, encoders and `run_hook` in `crates/core/protocol`.

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
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --lib -- text event native",
    "ac_refs": ["AC-8"],
    "paths": ["crates/core/protocol/src/text.rs", "crates/core/protocol/src/event.rs", "crates/core/protocol/src/native.rs", "crates/core/protocol/src/lib.rs", "crates/core/protocol/src/tests/text_test.rs", "crates/core/protocol/src/tests/event_test.rs", "crates/core/protocol/src/tests/native_test.rs"],
    "input": "host-events.test.ts cases: PascalCase rows, Cursor camelCase and aliases, Hermes snake_case, OpenCode dotted names, unknown and foreign names",
    "model": "inherit"
  },
  {
    "id": "decision",
    "title": "Decision, FailureCode, GateClass, strict wire and merge precedence with unit tests",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --lib -- decision",
    "ac_refs": ["AC-4", "AC-7"],
    "depends_on": ["events"],
    "paths": ["crates/core/protocol/src/decision.rs", "crates/core/protocol/src/tests/decision_test.rs", "crates/core/protocol/src/text.rs"],
    "input": "policy.test.ts and decision.test.ts cases; {\"kind\":\"allow\",\"x\":1}; empty reason; unknown kind and code",
    "model": "inherit"
  },
  {
    "id": "normalized",
    "title": "NormalizedEvent with Session/Tool, the flat strict wire and TryFrom per type, with round-trip and rejection tests",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --lib -- normalized",
    "ac_refs": ["AC-4"],
    "depends_on": ["events"],
    "paths": ["crates/core/protocol/src/normalized.rs", "crates/core/protocol/src/normalized/", "crates/core/protocol/src/tests/normalized_test.rs"],
    "input": "events.test.ts documents for all 11 types; futureField, command on tool/pre, missing sessionId, empty toolName, missing toolInput, null toolOutput",
    "model": "inherit"
  },
  {
    "id": "payload",
    "title": "Open host payload structs (HookPayload, CursorPayload, HermesPayload, OpencodePayload), LenientString, Payload::parse",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --lib -- payload",
    "ac_refs": ["AC-4"],
    "depends_on": ["events"],
    "paths": ["crates/core/protocol/src/payload.rs", "crates/core/protocol/src/payload/", "crates/core/protocol/src/tests/payload_test.rs"],
    "input": "Claude PreToolUse payload with future_field; {\"session_id\":7}; a duplicate key; a lone surrogate; Cursor beforeMCPExecution with string tool_input; Hermes docs envelope with extra; OpenCode {input, output}; [] and 7",
    "model": "inherit"
  },
  {
    "id": "normalize",
    "title": "Payload::normalize into NormalizedEvent (toolEvent port, SessionStart source chain, Cursor and Hermes field mapping, OpenCode None)",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --lib -- normalize",
    "ac_refs": ["AC-1"],
    "depends_on": ["normalized", "payload"],
    "paths": ["crates/core/protocol/src/normalize.rs", "crates/core/protocol/src/tests/normalize_test.rs"],
    "input": "Bash with and without command, Shell, empty command, tool_response vs tool_output vs null, lifecycle sources compact/resume/clear/7/false+event",
    "model": "inherit"
  },
  {
    "id": "encode",
    "title": "supports_ask, degrade_ask, decision normalization and the Claude/Codex, Cursor, Hermes and OpenCode encoders with unit tests ported from host-encode.test.ts",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --lib -- encode",
    "ac_refs": ["AC-8"],
    "depends_on": ["decision"],
    "paths": ["crates/core/protocol/src/encode.rs", "crates/core/protocol/src/encode/", "crates/core/protocol/src/tests/encode_test.rs"],
    "input": "host-encode.test.ts cases including the fail-closed invariants",
    "model": "inherit"
  },
  {
    "id": "encode-fixture",
    "title": "Shared fixtures/host/encode.json: TypeScript consumer test, Rust black-box byte test and Codex schema validation; index.json and README registration",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --test encode_fixture && bun test packages/toolu-core/src/host/__tests__/host-encode-fixture.test.ts tooling/src/__tests__/check-fixture-inventory.test.ts && bun run tooling/src/check-fixture-inventory.ts",
    "ac_refs": ["AC-2", "AC-3"],
    "depends_on": ["encode"],
    "paths": ["fixtures/host/encode.json", "fixtures/index.json", "fixtures/README.md", "tooling/src/__tests__/check-fixture-inventory.test.ts", "tooling/src/check-fixture-inventory.ts", "fixtures/codex-hook-schemas/", "crates/core/protocol/tests/encode_fixture.rs", "crates/core/protocol/tests/helpers/", "crates/core/protocol/Cargo.toml", "packages/toolu-core/src/host/__tests__/host-encode-fixture.test.ts", "packages/toolu-core/src/host/host-encode.ts", "crates/core/protocol/src/encode.rs", "crates/core/protocol/src/encode/"],
    "input": "fixtures/host/encode.json (299 cases) and fixtures/codex-hook-schemas/*.output.schema.json",
    "model": "inherit"
  },
  {
    "id": "payload-fixtures",
    "title": "Black-box test: every payload fixture deserializes for its host and normalizes to the expected kind",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --test payload_fixtures",
    "ac_refs": ["AC-1"],
    "depends_on": ["normalize"],
    "paths": ["crates/core/protocol/tests/payload_fixtures.rs", "crates/core/protocol/tests/helpers/", "fixtures/gates/", "fixtures/ast-grep/nudge.json", "fixtures/ast-grep/savings.json", "fixtures/quality/", "fixtures/opencode/permission-evaluate.json", "fixtures/portable-core/protected-files-pre.json", "tools/toolu-opencode/contract/captures/tool-calls.jsonl", "crates/core/protocol/src/payload.rs", "crates/core/protocol/src/payload/", "crates/core/protocol/src/normalize.rs"],
    "input": "fixtures/gates/*.json stdin, descriptors and bash commands; ast-grep nudge/savings; quality steps; opencode permission-evaluate requests; portable-core protected-files-pre; the 10 pinned-host OpenCode captures",
    "model": "inherit"
  },
  {
    "id": "run-hook",
    "title": "run_hook/run_hook_io with Reply/Raw/HookError, the class table and the quiet panic hook; unit tests on in-memory streams and a harness=false real-process test",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --lib -- hook && PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-protocol --test run_hook",
    "ac_refs": ["AC-5", "AC-6"],
    "depends_on": ["encode"],
    "paths": ["crates/core/protocol/src/hook.rs", "crates/core/protocol/src/hook/", "crates/core/protocol/src/tests/hook_test.rs", "crates/core/protocol/tests/helpers/", "crates/core/protocol/tests/run_hook.rs", "crates/core/protocol/Cargo.toml", "crates/core/protocol/src/encode.rs", "crates/core/protocol/src/stdin.rs"],
    "input": "a closure panicking with boom on PreToolUse, PostToolUse and SessionStart in a real child process; a PreToolUse deny, a PostToolUse block and a SessionStart advisory written to a really closed stdout pipe; HookError (incl. in_part dispatcher), invalid UTF-8 stdin, Cursor permission/evaluate, a refusing Write and Raw with Exit::Usage in memory",
    "model": "inherit"
  },
  {
    "id": "deps-docs",
    "title": "Dependency check (serde and serde_json only), crate doc, AGENTS.md key-files row",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo metadata --format-version 1 --no-deps | jq -e '[.packages[] | select(.name==\"toolu-protocol\") | .dependencies[] | select(.kind==null) | .name] | sort == [\"serde\",\"serde_json\"]' && grep -q 'crates/core/protocol/src/hook.rs' AGENTS.md && grep -q 'host/encode.json' fixtures/README.md && grep -q '1,491' fixtures/README.md",
    "ac_refs": ["AC-9"],
    "depends_on": ["run-hook", "encode-fixture", "payload-fixtures"],
    "paths": ["crates/core/protocol/Cargo.toml", "crates/core/protocol/src/lib.rs", "AGENTS.md", "fixtures/README.md"],
    "input": "cargo metadata of the workspace",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Full Rust gate and the TypeScript checks this change touches",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --base origin/main && bun run tooling/src/check-fixture-inventory.ts && bun test tooling/src/__tests__/check-fixture-inventory.test.ts packages/toolu-core/src/host/__tests__",
    "ac_refs": ["AC-10"],
    "depends_on": ["deps-docs"],
    "paths": ["crates/core/protocol/", "fixtures/host/", "fixtures/index.json", "fixtures/README.md", "packages/toolu-core/src/host/", "AGENTS.md", "Cargo.toml", "Cargo.lock", "tooling/conventions/guardrails/rust/"],
    "input": "the whole branch",
    "model": "inherit"
  }
]
```

## Critical files

- Create:
  - `crates/core/protocol/src/{text,native,decision,normalized,payload,normalize,encode,hook}.rs`;
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
