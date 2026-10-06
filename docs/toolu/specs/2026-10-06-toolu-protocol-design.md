# toolu-protocol: host payloads, events, decisions and encoders — Design

**Date:** 2026-10-06   **Status:** Draft   **Author:** epic worker (Claude Code)   **Topic:** #413. The bottom core crate gets the raw host payloads, `NormalizedEvent`, `Decision` and its precedence, the per-host output encoders with `ask` degradation, and `run_hook`, the fail-closed hook main.

Brainstorm: `docs/toolu/brainstorms/2026-10-06-toolu-protocol.md`. Builds on #407, #455 (the quality bar), #408 (the shared fixtures), #412 (`launcher`, `event::is_enforcing`) and #442 (`exit::Exit`, `host::Host`).

## Problem

Every Rust port in epic #402 (#418 to #424, then #430 to #433) reads a host's hook stdin, decides, and writes that host's hook stdout. If each port wrote its own payload structs, encoder and error handling, the ports would drift from the TypeScript bytes the hosts rely on today. A port that panicked would exit 101, which Claude Code treats as a non-blocking error, so a crashed `PreToolUse` gate would allow the tool call. `toolu-protocol` has no such types yet. It holds only the launcher, the exit codes and the host names.

## Non-Goals

1. No hook is ported, and the `toolu` binary does not change. `run_hook` gets its first production caller in #418. toolu's native `session-start` (#412) keeps its own `systemMessage` path.
2. No host detection, roots or config. Those live in `toolu-runtime` (#414). The caller passes the host and the project roots in.
3. OpenCode payloads get raw types only. Mapping them to `NormalizedEvent` needs the OpenCode tool-name map (`mapToolCall`, `bash` to `Bash`, …), which #462 ports. `Payload::normalize` returns `None` for OpenCode.
4. No portable-core v1 bridge envelope type. `fixtures/portable-core/protected-files-pre.json` has one consumer (`protected-dispatch.ts`), which turns it into a Claude payload. The test does the same.
5. No registry-module contract or dispatcher merge, such as jq-formatted `finalAsk`. Both belong to #418. `run_hook` passes a dispatcher's bytes through unchanged (`Reply::Raw`).
6. Some fields are polymorphic, such as an object `prompt`. Reading them with jq's exact text is the hook port's job (#424), done from the payload.
7. No gate data changes: `[workspace.lints]`, `clippy.toml`, `rules.json` and the jscpd limits stay as they are.

## Architecture

**Layering.** Everything is pure except `run_hook` and `stdin.rs`, which touch stdio. `toolu-protocol` owns that capability (`rules.json`). The normal dependencies stay `serde` and `serde_json`. `jsonschema` is a dev-dependency (workspace), used only by tests.

**Raw payloads are open.** One struct per host family: `HookPayload` serves Claude Code and Codex (Codex adds `turn_id` and `model`), plus `CursorPayload`, `HermesPayload` and `OpencodePayload`. All fields are optional and unknown keys are kept in `rest` (`#[serde(flatten)]`). A scalar string field is a `LenientString`: a JSON string reads as itself, and any other value reads as absent. That is TypeScript's `text(value, fallback)`. The fields the fixtures show to be polymorphic stay `serde_json::Value`: `source`, `session_event`, `event`, `prompt`, `tool_input` (Cursor sends MCP input as a JSON string), `tool_response`, `tool_output`, … Any JSON object parses. Anything else is an error that the hook handles.

**Normalization.** `Payload::normalize(event, roots)` ports `toolEvent` (`dispatch-context.ts`) and extends it to the session events:
- `sessionId` is the session id, or `"unknown"`. `cwd` is the payload's, else the project root. `projectRoot` and `worktree` come from `Roots`. `toolCallId` and `toolName` default to `"unknown"`.
- `toolInput` is the input object, or `{}`.
- `tool/post` carries `tool_response ?? tool_output`.
- A pre-tool payload whose tool is `Bash` or `Shell` and that has a non-empty string command is `shell/pre`. Any other pre-tool payload is `tool/pre`.
- `SessionStart` reads `source // session_event // event`, skipping null and false (session-start.ts). `"resume"`, `"clear"` and `"compact"` map to `session/resume`, `session/clear` and `compaction`. Anything else is `session/start`.
- `prompt` is the prompt string. A null, false or absent prompt is `""`. Any other value is its compact JSON text.
- `permission/evaluate` takes the tool name as `permission`.
- Cursor: the session id is `session_id`, else `conversation_id`. `beforeShellExecution` becomes the tool `Shell` with input `{"command": …}`. A string `tool_input` that holds a JSON object is that object.
- Hermes: the call id is `extra.tool_call_id`, and the prompt is `extra.user_message`.

**Strict internal types.** `NormalizedEvent` and `Decision` reject unknown fields. Each deserializes through one flat wire struct with `deny_unknown_fields`, then a `TryFrom` that requires the fields of its `type` (`kind` for `Decision`) and rejects the fields of every other type. So the wire keeps TypeScript's flat camelCase shape, the Rust enums nest a shared `Session` and `Tool`, and nothing is flattened. Zod's `min(1)` becomes `Text`, a non-empty `String` newtype.

**Decision and precedence.** `Decision` has six kinds: `Allow`, `Ask`, `Deny`, `Advisory`, `Block` (`post_block` on the wire) and `RuntimeFailure { reason, code }`. `merge` ports `mergeDecisions`. The ranks are runtime_failure 6, deny 5, post_block 5, ask 4, advisory 3 and allow 1. The first decision of the highest rank wins, and an empty list is allow.

**Encoders** port `host-encode.ts` line for line: `supports_ask`, `degrade_ask` (guardrail ask becomes deny, judgement ask becomes advice), the normalization step (runtime_failure becomes a deny on blocking events and advice elsewhere; a residual ask becomes a deny before an action and advice after; deny or block becomes `post_block` on `tool/post` and advice on context-only events), then the Claude/Codex, Cursor, Hermes and OpenCode outputs. Output JSON is built from `Serialize` structs in TypeScript's key order, because `serde_json::Map` sorts keys. Every line is `serde_json::to_string` plus `"\n"`, which is `JSON.stringify(value) + "\n"`.

**Shared golden.** `fixtures/host/encode.json` lists every host × host event × the six standard decisions, the `ask` of each gate class through `degrade_ask`, string-escaping cases, and the four events a host lacks. Each case carries its exact expected output. `packages/toolu-core/src/host/__tests__/host-encode-fixture.test.ts` checks `encodeDecision` against every case, and the Rust test checks `encode`. The file was captured once from the TypeScript encoder; there is no generator in the test path (`fixtures/README.md`). It is registered in `fixtures/index.json`.

**run_hook.** It reads stdin, then calls the closure with the payload text. The closure returns a `Reply`: a `Decision` to encode for (host, event), or `Raw` output (stdout, stderr, exit) that a dispatcher has already merged. `run_hook` writes the result and returns the `ExitCode`. Reading, the closure and encoding all run inside `catch_unwind`. A once-installed panic hook stays silent on a thread that is inside `run_hook` (a thread-local flag) and calls the previous hook on every other thread. The panic message then goes into toolu's line, not into Rust's report. Any failure ends by the event's class:

| Class | Events | Output | Exit |
|---|---|---|---|
| enforcing | `tool/pre`, `shell/pre`, `permission/evaluate` | stderr `blocked: <line>` | 2 |
| post | `tool/post`, `session/unload` | stderr `<line>` | 2 |
| context | `session/start`, `prompt`, `pre_compact` | stdout `{"systemMessage":"<line>"}` | 0 |

`<line>` is `toolu <native event> hook failed: <message>`, or `toolu <native event> hook panicked: <message>`. The native event is the host's name for the event, or the canonical slug when the host has none. On an enforcing event, a failed stdout write also exits 2. A `Raw` exit other than `Success` ends as 2. So `run_hook` returns only 0 or 2, never 101.

**Reused:** `exit::Exit`, `host::Host`, `stdin::read_all`, the workspace `jsonschema`, the `#[path = "helpers/…"]` test wiring of `crates/cli/tests`, and `readCaseFile` (`@toolu/conformance/harness/json-cases`) for the TypeScript consumer.

## Interfaces / Schema

```rust
// text.rs
#[serde(try_from = "String", into = "String")] pub struct Text(String);   // non-empty
impl Text { pub fn new(text: impl Into<String>) -> Result<Text, EmptyText>; pub fn as_str(&self) -> &str }

// event.rs (ENFORCING_EVENTS and is_enforcing unchanged)
pub enum HostEvent { SessionStart, SessionUnload, Prompt, PreCompact, PermissionEvaluate, ToolPre, ShellPre, ToolPost }
impl HostEvent { pub const ALL: [HostEvent; 8]; pub fn slug(self) -> &'static str }   // TS HOST_EVENTS order
pub enum EventKind { SessionStart, SessionResume, SessionClear, SessionUnload, Prompt, PreCompact,
                     Compaction, PermissionEvaluate, ToolPre, ToolPost, ShellPre }   // serde: the slugs
impl EventKind { pub fn slug(self) -> &'static str }

// native.rs
pub fn native_event(host: Host, event: HostEvent) -> Option<&'static str>;
pub fn canonical_event(host: Host, native: &str) -> Option<HostEvent>;   // first row, then Cursor aliases
pub fn hosts_for_native(native: &str) -> Vec<Host>;                       // Host::ALL order

// decision.rs
pub enum FailureCode { Timeout, Spawn, Parse, Truncated, Cancelled, Nonzero }
pub enum Decision { Allow, Ask { reason: Text }, Deny { reason: Text }, Advisory { message: Text },
                    Block { reason: Text }, RuntimeFailure { reason: Text, code: FailureCode } }
pub enum GateClass { Guardrail, Judgement }
pub fn merge(decisions: &[Decision]) -> Decision;
// wire: {"kind":"allow"} {"kind":"ask","reason":…} {"kind":"advisory","message":…}
//       {"kind":"post_block","reason":…} {"kind":"runtime_failure","reason":…,"code":"nonzero"}

// normalized.rs
pub struct Session { pub session_id: Text, pub cwd: Text, pub project_root: Text, pub worktree: Text }
pub struct Tool { pub call_id: Text, pub name: Text, pub input: Map<String, Value> }
pub enum NormalizedEvent {
  SessionStart(Session), SessionResume(Session), SessionClear(Session), SessionUnload(Session),
  Prompt { session: Session, prompt: String }, PreCompact(Session), Compaction(Session),
  PermissionEvaluate { session: Session, permission: Text },
  ToolPre { session: Session, tool: Tool }, ToolPost { session: Session, tool: Tool, output: Option<Value> },
  ShellPre { session: Session, tool: Tool, command: Text },
}
impl NormalizedEvent { pub fn kind(&self) -> EventKind; pub fn session(&self) -> &Session; pub fn tool(&self) -> Option<&Tool> }
// wire: {"type":"shell/pre","sessionId","cwd","projectRoot","worktree","toolCallId","toolName","toolInput","command"}

// payload.rs, payload/{hook,cursor,hermes,opencode}.rs
pub struct LenientString(Option<String>);   impl LenientString { pub fn as_str(&self) -> Option<&str> }
pub struct HookPayload { session_id, transcript_path, cwd, hook_event_name, permission_mode, model, turn_id,
  tool_name, tool_use_id, trigger, custom_instructions, reason: LenientString,
  tool_input, tool_response, tool_output, prompt, source, session_event, event: Option<Value>,
  rest: Map<String, Value> }
pub struct CursorPayload { conversation_id, generation_id, model, hook_event_name, cursor_version, user_email,
  transcript_path, cwd, session_id, tool_name, tool_use_id, command, mcp_server_name, file_path: LenientString,
  workspace_roots, tool_input, tool_output, sandbox, output, edits, prompt, attachments: Option<Value>, rest }
pub struct HermesPayload { hook_event_name, tool_name, session_id, cwd, profile: LenientString,
  tool_input, extra: Option<Value>, rest }
pub struct OpencodePayload { input: OpencodeInput, output: Option<Value>, rest }   // the SDK's (input, output)
pub struct OpencodeInput { tool, session_id /* sessionID */, call_id /* callID */, agent, message_id /* messageID */: LenientString,
  args: Option<Value>, rest }
pub enum Payload { Claude(HookPayload), Codex(HookPayload), Cursor(CursorPayload), Hermes(HermesPayload), Opencode(OpencodePayload) }
pub fn parse(host: Host, text: &str) -> Result<Payload, serde_json::Error>;
pub struct Roots { pub project_root: Text, pub worktree: Text }
impl Payload { pub fn normalize(&self, event: HostEvent, roots: &Roots) -> Option<NormalizedEvent> }   // normalize.rs

// encode.rs, encode/{hook,cursor}.rs
pub fn supports_ask(host: Host, event: HostEvent) -> bool;
pub fn degrade_ask(host: Host, event: HostEvent, decision: Decision, class: GateClass) -> Decision;
pub enum Encoded { Command(String), Callback(Callback) }        // Command: the exact stdout ("" when silent)
pub struct Callback { pub action: CallbackAction, pub message: Option<String> }
pub enum CallbackAction { Continue, Throw }
impl Callback { pub fn json(&self) -> String }                   // {"kind":"callback","action":…,"message":…}
pub struct EncodeError { pub host: Host, pub event: HostEvent }  // "cursor has no native event for permission/evaluate"
pub fn encode(host: Host, event: HostEvent, decision: &Decision) -> Result<Encoded, EncodeError>;

// hook.rs, hook/panic.rs
pub enum Reply { Decision(Decision), Raw(Raw) }   impl From<Decision> for Reply
pub struct Raw { pub stdout: String, pub stderr: String, pub exit: Exit }
pub struct HookError(String);   impl HookError { pub fn new(message: impl Into<String>) -> HookError }   // Display, Error
pub struct Io<R, O, E> { pub stdin: R, pub stdout: O, pub stderr: E }
pub fn run_hook<F>(event: HostEvent, host: Host, hook: F) -> ExitCode
  where F: FnOnce(&str) -> Result<Reply, HookError>;   // real stdio
pub fn run_hook_io<R: Read, O: Write, E: Write, F>(io: Io<R, O, E>, event: HostEvent, host: Host, hook: F) -> ExitCode;
```

An OpenCode `Reply::Decision` is written as `Callback::json()` plus `"\n"` on stdout with exit 0. That is the line the stdio shim (#437) will read.

**`fixtures/host/encode.json`:** `{ "version": 1, "cases": [{ "name", "host", "event", "decision", "gateClass"?, "expect": {"stdout": "<bytes>"} | {"callback": "<JSON.stringify(output)>"} | {"error": "<message>"} }] }`.

**Tests.** Unit tests sit in `src/tests/<module>_test.rs`, plus `src/payload/tests/` and `src/encode/tests/` for the submodules (rule 6). The black-box tests are:
- `tests/payload_fixtures.rs`: AC-1;
- `tests/encode_fixture.rs`: AC-2 and AC-3;
- `tests/run_hook.rs`: AC-5. It has `[[test]] harness = false` and its own `main`; it runs itself as a child with real stdio.
- `tests/helpers/repo.rs`: reads the repository fixtures.

## Failure modes and edge cases

- **Stdin not UTF-8, or unreadable:** a `HookError` ("the hook payload could not be read: …"), then the class table.
- **Empty or non-JSON stdin:** `run_hook` passes the text to the closure, so the hook keeps today's behaviour. For example, invalid JSON is startup for session-start. `parse` returns `Err` for the closure to handle.
- **JSON that is not an object** (`[]`, `7`): `parse` returns `Err`. **A wrong-typed scalar** (`"session_id": 7`) reads as absent, then falls back as in TypeScript. **JSON null** on a `Value` field reads as absent, like TypeScript's `??`.
- **An unknown field:** a payload keeps it in `rest`. A `NormalizedEvent` or `Decision` fails to parse. The same applies to another type's field (`command` on `tool/pre`), an empty `Text`, an unknown `type`, `kind` or `code`, and a missing required field.
- **A decision for an event the host lacks** (Cursor `permission/evaluate`; Hermes `pre_compact` and `permission/evaluate`; OpenCode `permission/evaluate`): `encode` returns `EncodeError`. Inside `run_hook`, that is an internal failure handled by the class table.
- **A panic:** a `&str` or `String` payload becomes the message. Any other payload becomes "a panic with a non-text payload". The panic report stays quiet only on the `run_hook` thread. A panic elsewhere still prints through the previous hook. A panic is never re-raised.
- **An abort** (`panic = "abort"`, a double panic, out of memory): `catch_unwind` cannot catch it. The launcher (#412) maps an enforcing event's abnormal exit to 2. The release profile keeps `panic = "unwind"`.
- **A closed stdout:** on an enforcing event, the write error becomes `blocked: toolu <event> hook could not write its output: <error>` on stderr, with exit 2. On other events it is ignored and the exit stands. A stderr write error is ignored.
- **A `Raw` reply with exit `Failure`, `Usage`, …:** exit 2. Its stdout and stderr are written as given.
- **Concurrent `run_hook` calls** on several threads: the quiet flag is per thread, and the hook is installed once (`Once`). No restore race.

## Acceptance criteria

- **AC-1:** Every payload fixture deserializes for its host. The payload fixtures are:
  - every JSON `stdin` in `fixtures/gates/*.json`;
  - every tool-call descriptor in `fixtures/gates/*.json` and `fixtures/ast-grep/nudge.json`, rendered as `tools/toolu-conformance/src/harness/fixtures.ts` renders it: the Claude and Codex envelopes, plus Cursor for `PreToolUse`;
  - every `payload` in `fixtures/ast-grep/savings.json`;
  - the requests in `fixtures/opencode/permission-evaluate.json`;
  - `fixtures/portable-core/protected-files-pre.json`, rendered as `protected-dispatch.ts` renders it.

  The non-JSON stdin cases parse to `Err`, never a panic. Normalizing the parsed payloads gives the kinds TypeScript implies. Lifecycle `{"source":"compact"}` is `compaction`, `{"source":false,"event":"resume"}` is `session/resume`, and `{"source":7}` is `session/start`. A `Bash` descriptor with a command is `shell/pre`, a `PostToolUse` descriptor is `tool/post`, and Cursor `beforeShellExecution` is `shell/pre` with tool `Shell`.
- **AC-2:** For hosts `claude` and `codex`, every `Command` output in `fixtures/host/encode.json` for an event with a schema validates against its `fixtures/codex-hook-schemas/<event>.command.output.schema.json`. Those events are `tool/pre`, `shell/pre`, `tool/post`, `session/start`, `prompt` and `permission/evaluate`. Empty outputs are silent success and are skipped.
- **AC-3:** The TypeScript `encodeDecision` and the Rust `encode` reproduce every case of `fixtures/host/encode.json` byte for byte. That covers 36 host/event pairs × 6 decisions, 72 gate-class `ask` cases, the escaping cases and the 4 wiring errors. In the issue scenario, a Codex `tool/pre` `ask` from a judgement gate encodes to `{"hookSpecificOutput":{"hookEventName":"PreToolUse","additionalContext":"confirm .env write"}}\n`, and the TypeScript side produces the same bytes.
- **AC-4:** A Claude payload with an unknown top-level field (`"future_field": 1`) parses and keeps it in `rest`. A `NormalizedEvent` or `Decision` document with an unknown field fails to parse. So do an empty `reason`, a `command` on `tool/pre`, and an unknown `type`, `kind` or `code`. Every valid document round-trips through serde to equal JSON.
- **AC-5:** A real child process calls `run_hook` with a closure that panics with `boom`, on real stdio:
  - **`PreToolUse`:** exit 2, stdout empty, stderr exactly `blocked: toolu PreToolUse hook panicked: boom\n`.
  - **`PostToolUse`:** exit 2, stderr exactly `toolu PostToolUse hook panicked: boom\n`.
  - **`SessionStart`:** exit 0, stdout exactly `{"systemMessage":"toolu SessionStart hook panicked: boom"}\n`, stderr empty.

  A Claude `PreToolUse` deny from the same child prints the encoded deny and exits 0. A panic outside `run_hook` in the child is still reported by Rust's default hook (exit 101).
- **AC-6:** With in-memory streams, a `HookError`, an unreadable payload and an encode wiring error each follow the class table for all eight host events: `session/unload` exits 2, and `prompt` and `pre_compact` exit 0 with a `systemMessage`. A `Raw` reply is written unchanged, and its non-success exit becomes 2. A closed stdout on `tool/pre` exits 2.
- **AC-7:** `merge` matches `mergeDecisions` on the TypeScript precedence cases (`policy.test.ts`). Deny beats ask beats advisory beats allow. Runtime failure beats deny. Deny and `post_block` tie, so the first wins. An empty list is allow.
- **AC-8:** `native_event`, `canonical_event` and `hosts_for_native` give the answers of `host-events.test.ts`. That includes every round trip, the Cursor aliases, and an unknown or foreign name giving `None` or an empty list. `supports_ask` and `degrade_ask` give the answers of `host-encode.test.ts`.
- **AC-9:** In `cargo metadata`, `toolu-protocol`'s normal dependencies are exactly `serde` and `serde_json`.
- **AC-10:** `cargo xtask gate`, `bun run tooling/src/check-fixture-inventory.ts` and `bun test packages/toolu-core/src/host/__tests__` pass.

## Acceptance evidence

| AC | Real input | Expected | Boundary | Check |
|---|---|---|---|---|
| AC-1 | `fixtures/gates/*.json`, `fixtures/ast-grep/{nudge,savings}.json`, `fixtures/opencode/permission-evaluate.json`, `fixtures/portable-core/protected-files-pre.json` | Every payload parses, and normalized kinds as listed | `{"source":7}`, object and numeric prompts, empty stdin, `{not json` | `cargo test -p toolu-protocol --test payload_fixtures` |
| AC-2 | `fixtures/host/encode.json`, `fixtures/codex-hook-schemas/*.json` | No schema errors | Silent outputs skipped, `PreCompact` and `SessionEnd` have no schema | `cargo test -p toolu-protocol --test encode_fixture` |
| AC-3 | `fixtures/host/encode.json` | Exact bytes on both sides | Escaping case: quote, backslash, newline, tab, U+0001, U+007F, é, 😀, U+2028, `</script>` | `cargo test -p toolu-protocol --test encode_fixture`; `bun test packages/toolu-core/src/host/__tests__/host-encode-fixture.test.ts` |
| AC-4 | Claude `PreToolUse` payload plus `future_field`; `NormalizedEvent` and `Decision` JSON | Open parses, strict fails | Empty `Text`, a foreign field, a missing field | `cargo test -p toolu-protocol` (`normalized_test`, `decision_test`, `payload_test`) |
| AC-5 | The `run_hook` test binary as its own child | Exit codes and exact streams | A panic outside `run_hook` gives 101 | `cargo test -p toolu-protocol --test run_hook` |
| AC-6 | Byte streams; a `Write` that fails | The class table | Every `HostEvent`; `Raw` with `Exit::Usage` | `cargo test -p toolu-protocol hook` |
| AC-7 | The cases of `policy.test.ts` | The same winners | An empty list; ties | `cargo test -p toolu-protocol decision` |
| AC-8 | The cases of `host-events.test.ts` and `host-encode.test.ts` | The same answers | Unknown names, Cursor aliases | `cargo test -p toolu-protocol native encode` |
| AC-9 | `cargo metadata --format-version 1 --no-deps` | Normal deps `serde`, `serde_json` | Dev-deps excluded | `cargo metadata --format-version 1 --no-deps \| jq '.packages[] \| select(.name=="toolu-protocol") \| [.dependencies[] \| select(.kind==null) \| .name]'` |
| AC-10 | The repository | All green | — | `cargo xtask gate`; `bun run tooling/src/check-fixture-inventory.ts`; `bun test packages/toolu-core/src/host/__tests__` |

## Documentation impact

- `AGENTS.md` **Key files**: a row for `crates/core/protocol`. It names the payload, event, decision and encode modules and `run_hook` with its class table, and the shared encode fixture.
- `fixtures/README.md`: a row in the case table for `host/encode.json`, and a line describing its records.
- `fixtures/index.json`: the new `host-encode` suite.
- `crates/core/protocol/src/lib.rs`: the crate doc lists the new modules.
- `docs/portable-core.md` is unchanged. It documents the TypeScript contract, which the Rust port follows, and the port changes no behaviour.

## Open Questions

None blocking. Two decisions are recorded here because no human answers questions in this run:
- **SessionEnd exits 2 on failure.** The issue's "only" list names the three context events that exit 0. SessionEnd cannot block, so exit 2 only shows the line to the user (Jev 0.98).
- **The normalized `prompt` of a non-string value is its compact JSON text.** jq would print pretty text; the prompt port (#424) reads the payload's `prompt` itself.
