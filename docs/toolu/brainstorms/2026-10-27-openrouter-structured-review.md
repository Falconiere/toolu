# OpenRouter structured review calls

**Date:** 2026-10-27
**Mode:** Delivery

## Outcome

One shared core OpenRouter client sends bounded, privacy-routed, strict-schema chat requests. The code-review crate accepts complete findings from valid or truncated responses, marks any incomplete result partial, and turns empty or unusable output into a typed error.

## Material choices

- Put credentials, HTTP errors, retry deadline, provider identity, and usage in `core/openrouter`. Keep the review findings schema and normalization in `code-review`; #676 can add streaming and tool calls to the shared client. Jev choice favored this boundary (A, confidence 1.0) on the issue and repository evidence.
- Use the existing `core/http` transport and `http-test-support` TLS fixture. No new HTTP stack or mocking crate.
- Send the pinned DeepSeek snapshot by default, with `OPENROUTER_MODEL` as an environment override. The same reasoning and provider routing applies to overrides.
- Treat partial salvage as a blocked review result until orchestration can account for unreviewed coverage. A model's claimed verdict is never trusted.

## Evidence and risks

Issue #665 and epic #660 require a shared crate, strict JSON schema, ZDR and precision routing, deadline-shared retries, and unknown cost when absent. `core/http` already bounds body and request time; `code-review/src/budget.rs` holds effort budgets; `code-review/src/contract.rs` holds the report shape. OpenRouter documentation confirms `response_format.json_schema.strict`, `reasoning.exclude`, `provider.require_parameters`, `provider.zdr`, quantization routing, and usage reasoning tokens. Risks are malformed partial JSON, provider errors embedded in HTTP 200, and losing the remaining deadline between attempts.

## Handoff

Write the concrete contract and loopback HTTPS acceptance cases in the spec.
