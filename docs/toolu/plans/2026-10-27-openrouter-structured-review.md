# OpenRouter structured review calls — Plan

**Date:** 2026-10-27   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-27-openrouter-structured-review-design.md   **Topic:** shared core client and review output parser (#665)

## Evidence and approach

`core/http` already provides bounded synchronous HTTPS, auth and a whole-request deadline; `core/jev` shows key handling and `http-test-support` use. `code-review/src/budget.rs` supplies effort-specific token limits; `code-review/src/contract.rs` holds the JSON report. Issue #665 and epic #660 fix the model, routing and error behavior. Build the shared client on `core/http`, then add the review-specific strict schema/parser, then synchronize report schema and documentation.

## Workstream summary

Core transport and registration → review parser and report contract → docs and full verification.

## Steps (machine-readable)

```json
[
  {
    "id": "client",
    "title": "Register core/openrouter and build typed bounded structured calls with shared-deadline retries and TLS fixture tests",
    "check": "cargo test -p toolu-openrouter && cargo test -p xtask the_repository_has_no_violation",
    "paths": ["crates/core/openrouter", "crates/core/http", "crates/http-test-support", "crates/xtask/src/tests/layers_check_test.rs", "Cargo.toml", "Cargo.lock", "tooling/conventions/guardrails/rust/layers.json", "tooling/conventions/guardrails/rust/folders.json"],
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-5", "AC-6"],
    "input": "Real loopback HTTPS fixture sequences: success, 429 then success, 401, missing key, delayed timeout, malformed envelope and embedded provider error",
    "model": "inherit"
  },
  {
    "id": "parser",
    "title": "Commit strict review schema, validate and salvage model findings, and keep missing cost nullable in report and schema",
    "check": "cargo test -p toolu-code-review && cargo test -p toolu-cli --test contract",
    "paths": ["crates/code-review", "crates/cli/src/commands.schema.json", "docs/cli/commands.schema.json", "crates/cli/tests"],
    "depends_on": ["client"],
    "ac_refs": ["AC-3", "AC-4", "AC-5"],
    "input": "Complete and truncated review JSON with valid, malformed and deviating findings; report with known zero and unknown cost",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document the shared client and synchronize agent-facing Markdown and CLI schema files",
    "check": "cargo xtask check-markdown-cli && cargo xtask check-cli-compat",
    "paths": ["."],
    "depends_on": ["parser"],
    "ac_refs": ["AC-3", "AC-6"],
    "input": "The committed README and report schema compared with the generated CLI contract and origin/main",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Run the full local quality gate on the whole branch",
    "check": "cargo xtask gate",
    "paths": ["."],
    "depends_on": ["docs"],
    "ac_refs": ["AC-6"],
    "input": "The complete branch, including all crates, docs and registration data",
    "model": "inherit"
  }
]
```

## Critical files

Create `crates/core/openrouter/{Cargo.toml,README.md,src/*.rs,src/tests/*_test.rs,tests/*.rs}`, `crates/code-review/src/model.rs`, `crates/code-review/src/model.schema.json`, and `crates/code-review/src/tests/model_test.rs`. Modify `Cargo.toml`, `Cargo.lock`, `tooling/conventions/guardrails/rust/{layers,folders}.json`, `crates/code-review/{Cargo.toml,src/lib.rs,src/contract.rs,src/tests/contract_test.rs}`, and both `commands.schema.json` copies. Update crate inventory tests if the new crate changes their expected count.

## Verification

TLS fixture checks actual request body, routing, key behavior, error mapping, retry count and wall-time bound. Parser cases check complete, truncated, deviating and garbage input with no invented fields. Contract tests check unknown cost stays `null`. The final gate covers registration, layers, clippy, docs, tests and coverage. Before push, rerun the gate and local xhigh code review on any new `origin/main` commits, per the user's delivery cycle.

## Deviations

- The xhigh review found that whitespace trimming changed evidence and produced false partial results. Model text now keeps quote and suggestion bytes; enum casing and exact decimal line strings remain the safe normalization boundary.
- Billed usage is retained on empty/truncated errors, and embedded transient 429/5xx errors retry within the shared deadline. The request omits an unrequested temperature parameter so `require_parameters` does not exclude providers unnecessarily.
- A second xhigh pass kept evidence whitespace while normalizing padded path identifiers and blank optional fields. Pre-send invalid configuration is reported as unconfigured; unusable billed completions retain their usage.
- The repository gate required colocated tests for every new function-bearing module and an integration consumer for public model output types. Those tests are part of the client/parser checks.

## Plan review

- The core registration step runs the real xtask layer inventory test and declares its count fixture.
- The documentation check reads agent Markdown across the repository, so its freshness path is the whole tree.
- Every AC has a real fixture or repository-wide gate check. The user-authorized delivery path is local gate, local xhigh code review, gate again, commit, rebase, rerun affected checks, push, resolve threads, and admin squash merge.
