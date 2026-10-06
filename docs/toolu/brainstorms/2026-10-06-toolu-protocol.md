# toolu-protocol: host payloads, events, decisions and encoders — brainstorm (#413)

**Date:** 2026-10-06   **Mode:** Delivery, Full path (the bottom crate's public types, the fail-closed hook wrapper)

## Capsule

- **Outcome:** `crates/core/protocol` gains every type that crosses a hook's process boundary: the raw stdin payload of each host, the `NormalizedEvent` vocabulary and the mapping into it, `Decision` with TypeScript's merge precedence, the per-host output encoders with `ask` degradation, and `run_hook`, the `catch_unwind` hook main that never exits 101. The crate keeps `serde` and `serde_json` as its only dependencies.
- **Material defaults / non-goals:** no hook is ported and the `toolu` binary does not change. The ports (#418 to #424) call `run_hook`. Host detection and the project roots stay in `toolu-runtime` (#414). They reach the crate as arguments. OpenCode payloads get raw types, but their mapping to `NormalizedEvent` needs the OpenCode tool-name map, which #462 owns. The portable-core v1 bridge envelope (`fixtures/portable-core/protected-files-pre.json`) has no live consumer, so it gets no Rust type. Its one consumer turns it into a Claude payload, and the test does the same.
- **Repository evidence:** `packages/toolu-core/src/host/host-encode.ts` (encoders, `supportsAsk`, `degradeAsk`), `host-events.ts` (native names), `decision/decision.ts`, `policy/policy.ts` (`mergeDecisions`), `events/events.ts`, `dispatch/dispatch-context.ts` (`toolEvent`, the lenient payload reads), `plugins/toolu/hooks/src/pre-tools/hook-main.ts` (exit 2 on a dispatcher error), `docs/portable-core.md` (the boundary rules: host envelopes pass extras through), and `fixtures/gates/lifecycle.json` (stdin with `source: 7`, `prompt: {}` and similar values that must parse).
- **Risk:** encoder byte parity depends on serde's key order and string escaping. The shared golden fixture checks both implementations. The payload structs could miss a field a later port needs, but every unknown field is kept, so a port reads it from `rest` until it is named.
- **Handoff:** spec.

## Axes and decisions

| Axis | Decision | Evidence | Jev |
|---|---|---|---|
| Payload typing | Open structs, one per host family. Scalar strings use a lenient deserializer: a non-string reads as absent, which is TypeScript's `text(value, fallback)`. The fields the fixtures show to be polymorphic (`source`, `prompt`, `tool_input`, `tool_response`, …) stay `serde_json::Value`. Unknown keys are kept in `rest` (`flatten`). No JSON object is rejected. | TS never validates host payloads (`dispatch-context.ts`); the lifecycle fixtures include `{"source":7}`, `{"prompt":42}` and an object prompt; Cursor sends MCP `tool_input` as a string | `typed_lenient` 0.98 |
| Strict internal types | `NormalizedEvent` and `Decision` use `deny_unknown_fields`, with a non-empty string newtype for zod `min(1)`. Shared context fields are repeated, not flattened. | Issue scenario: a strict internal type must reject an unknown field. toolu itself is their only producer. | `strict` 0.72 |
| Decision kinds | allow, ask, deny, advisory, block (`post_block` on the wire) and `runtime_failure`. | `mergeDecisions` ranks `runtime_failure` first, and the encoders turn it into a deny or advice | keep 0.83 |
| Differential test | A committed `fixtures/host/encode.json` holds host, event, decision and an optional gate class, with the exact expected output. A Bun test checks `encodeDecision` against every case, and a Rust test checks the Rust encoder against the same cases. | The committed-fixture pattern of #408; no Rust test spawns Bun today | `shared_golden` 0.86 |
| Panic output | A process-wide panic hook, installed once, stays silent only on a thread that is inside `run_hook`, and calls the previous hook everywhere else. The panic message goes into the toolu line. | The default report adds `thread … panicked at …` lines to the stderr the model reads on exit 2. A swap-and-restore hook races with parallel tests. | `quiet_hook` 0.95 |
| SessionEnd failure | Exit 2 with the line on stderr and no `blocked:` prefix, as the issue's "only" list says | The issue's error rule; SessionEnd cannot block on Claude Code | `exit_2` 0.98 |
| Hook reply | The closure returns a `Decision` to encode, or raw output (stdout, stderr, exit) for the dispatchers that merge module output byte for byte | `dispatch.ts` returns `{stdout, stderr, exitCode}`, with exit 2 when a module exits 2 | — |
| Real-process panic test | A `harness = false` integration test runs itself as a child. The child calls `run_hook` on real stdio and returns its `ExitCode`. | Only `crates/cli` may build a binary; `src/main.rs` is allowed only in cli and xtask | — |

## Rejected alternatives

- **Value-backed payloads** (every field `Option<Value>`): no typed access, and every consumer would re-implement the same reads.
- **Strictly typed payloads:** `{"source":7}` and the other committed lifecycle inputs would fail to parse, against the acceptance criteria and today's behaviour.
- **Lenient internal types:** no protocol type would satisfy the strict-type scenario.
- **Spawning Bun from cargo tests:** a second toolchain inside the Rust unit gate, and a skip-or-fail question whenever Bun is absent.
- **Leaving the default panic hook:** a multi-line Rust report ahead of the reason the model sees.
- **Wiring `run_hook` into the `toolu` binary now:** the one native hook (toolu `session-start`) prints a `systemMessage`, not an encoded decision, and every other hook is ported by its own issue.
