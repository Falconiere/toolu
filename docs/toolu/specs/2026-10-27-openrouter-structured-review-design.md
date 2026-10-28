# OpenRouter structured review calls — Design

**Date:** 2026-10-27   **Status:** Approved   **Author:** Codex   **Topic:** Shared OpenRouter client and review finding normalization

## Problem

The native code-review pipeline needs a reusable OpenRouter call that cannot mistake malformed or truncated model output for a clean review. It must keep source under the specified privacy routing, bound network work, and expose cost and error facts to orchestration.

## Non-Goals

1. Prompt construction, Jev judgment, anchoring, verdict state, streaming, and tool calling belong to #666, #668, #669, and #676.
2. No spend ceiling or fallback to providers outside the required routing set.
3. No live OpenRouter call in tests.

## Architecture

Add `crates/core/openrouter` in the top core layer, depending on `toolu-http` and `toolu-runtime`. Its `Client` reads `OPENROUTER_API_KEY` and optional `OPENROUTER_MODEL` from the supplied `Env`. `complete_structured` sends a non-streaming chat request through `toolu_http::Client::send`, recreating that transport with the remaining time on each attempt. The whole call has one caller-supplied `Instant` deadline, including retry waits. Responses are bounded by `code-review`'s `HTTP_RESPONSE_MAX_BYTES` or an equivalent core constant. No key or response body enters error `Display`/`Debug` or usage logs.

The code-review crate owns `model.rs` and a committed strict JSON schema for `{findings:[...]}`. It parses complete JSON first, then scans only fully closed finding objects in a truncated array. Each candidate is locally validated; syntactic normalization lowercases known enum values and accepts exact decimal line strings. Whitespace in quotes and suggestions is preserved as evidence. Missing or blank claim, quote, path, line, severity, or category is dropped. No finding text is generated. The result carries `partial` and `dropped` so #666 treats incomplete coverage as blocked. #668 remains responsible for grounding every finding in the diff.

## Interfaces / Schema

- `toolu_openrouter::Client::from_env(&Env, Config) -> Result<Client, Error>`; `Config` holds endpoint, test CA, and retry pause. Default endpoint is `https://openrouter.ai/api/v1/chat/completions`; default model is `deepseek/deepseek-v4-pro-0813`.
- `Client::complete_structured(&StructuredRequest, deadline: Instant) -> Result<Completion, Error>`. `StructuredRequest` holds messages, schema name and JSON value, reasoning effort, and max tokens. The call always sends `response_format: {type:"json_schema", json_schema:{name, strict:true, schema}}`, `reasoning:{effort,exclude:true}`, `provider:{require_parameters:true,zdr:true,quantizations:["fp8","bf16","fp16"]}`, and `stream:false`.
- `Completion` carries raw visible content, `finish_reason`, served `model` and `provider`, `Usage {prompt_tokens,completion_tokens,reasoning_tokens,cost_usd:Option<f64>}`. Missing cost stays `None`; the serving provider is recorded when returned.
- `Error` distinguishes missing key (`Unconfigured`), malformed key, authentication, rate limit with `Retry-After`, no eligible provider, upstream status, timeout, invalid envelope, empty content with finish reason, and reasoning-only truncation with finish reason. A non-streaming HTTP 200 embedded provider error is an error.
- The model schema requires one object with `findings` array; each finding has `path`, `line`, `end_line`, `side`, `severity`, `category`, `claim`, `quote`, optional `suggestion` and `prior_id`. `additionalProperties:false`; severity is `blocker|high|medium|low|nit`, side `new|old`. No model verdict field.
- `code_review::model::parse(&Completion) -> Result<ModelOutput, ModelError>` returns complete, locally valid findings plus `partial` and dropped count; `ModelError` includes the finish reason. A valid empty `findings` array is clean only when the complete response finished normally.
- Model and transport errors map to a failed report when #666 wires the pipeline. Pre-send configuration failures use `ErrorKind::Unconfigured`; sent-call failures and malformed model output use `ErrorKind::Outage`. The `error` verdict includes the finish reason for empty or truncated content and retains observed billed usage.
- Change `code_review::contract::Usage.cost_usd` and the committed CLI report JSON schema from mandatory numeric zero to nullable cost. Raise the report schema version to 2 because version 1 required a number. A missing OpenRouter cost remains `null` through serialization; reported zero remains numeric zero.

## Failure modes and edge cases

- Missing key or line-break key: fail before constructing a request. Endpoint override exists only for loopback tests, not from a repository file.
- 401/403 and other permanent 4xx stop; 408/429/5xx and transport failures retry at most three times within the same deadline. Honor numeric `Retry-After` up to 60 seconds; a longer wait returns the typed failure without sleeping through the review deadline.
- Empty choices, empty or null content, malformed envelope, HTTP 200 provider error, or unparseable model content never becomes an empty finding list. Include finish reason and reported billed usage in empty/truncation errors; do not echo content. Retry embedded 429/5xx errors within the same deadline.
- `finish_reason:length` with zero or near-zero visible tokens is a typed truncation error. Any salvaged finding or any dropped finding marks output partial. Even a syntactically valid array with `finish_reason:length` is partial.
- A malformed trailing finding does not discard earlier complete findings. Invalid individual findings are dropped and counted. If there is no valid finding and content was malformed, fail closed. An empty findings array is clean only with `finish_reason: stop` and no extra top-level fields; other finish reasons are errors.
- Absent or invalid cost is unknown. An absent provider is recorded as unknown, never fabricated.

## Acceptance criteria

- **AC-1:** With a key in `Env`, a loopback HTTPS server sees the strict schema, explicit reasoning effort, fixed privacy and precision routing, pinned model, and bounded response; an env model override changes only the model.
- **AC-2:** Loopback HTTPS 408/429/5xx and dropped connections retry within one deadline; auth and no-provider errors remain typed, and a missing key makes zero requests.
- **AC-3:** A valid response yields the observed provider, token counts, optional cost, content and finish reason; an absent cost remains unknown.
- **AC-4:** Complete valid findings parse; a truncated response salvages only complete valid findings; malformed entries are dropped and mark partial, with no fabricated fields.
- **AC-5:** Empty, unparseable, embedded-error, or reasoning-only responses produce typed errors carrying finish reason where available, and never a clean result.
- **AC-6:** The new crate is registered in workspace, layers and folders; focused crate tests and the full gate pass.

## Acceptance evidence

| AC | Real input / fixture | Expected result | Check |
| --- | --- | --- | --- |
| AC-1 | `http-test-support::Fixture` TLS origin inspecting request body | Exact required fields and model, no weakened routing | `cargo test -p toolu-openrouter` |
| AC-2 | Fixture response sequence 429 then 200, delayed response, 401, missing key | Retry count and deadline bounded; typed errors; no request without key | `cargo test -p toolu-openrouter` |
| AC-3 | Fixture JSON with and without `usage.cost` and `provider` | Exact observed usage; report contract serializes unknown cost as `null` | `cargo test -p toolu-openrouter && cargo test -p toolu-code-review` |
| AC-4 | Complete and truncated JSON strings containing valid and invalid finding objects | Only complete valid findings returned; `partial` and dropped count | `cargo test -p toolu-code-review` |
| AC-5 | Empty choices/content, garbage JSON, embedded provider error, `length` with all reasoning tokens | Typed errors and an `error` report with finish reason; no clean result | `cargo test -p toolu-openrouter && cargo test -p toolu-code-review` |
| AC-6 | New crate registration data and real workspace gate | Build and all guardrails green | `cargo xtask gate` |

## Documentation impact

Add `crates/core/openrouter/README.md` describing the public API and error/usage semantics. Update the report's `cost_usd` JSON schema in `crates/cli/src/commands.schema.json` and `docs/cli/commands.schema.json`; the `toolu code-review` CLI verb tree is unchanged.

## Open Questions

None. #666 consumes `ModelOutput`; #676 extends the shared core client.
