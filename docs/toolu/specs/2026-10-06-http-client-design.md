# Small HTTPS client — Design

**Date:** 2026-10-06   **Status:** Approved   **Author:** Codex   **Topic:** Falconiere/toolu#417

## Problem

The Rust Jev and GitHub clients need one small synchronous HTTPS transport.
Today `crates/core/http` is a placeholder, so later crates cannot make timed,
bounded, proxy-aware requests or test them without TypeScript fixtures.

## Non-Goals

1. Porting `@toolu/core/rest` or implementing GitHub/Jev endpoints (#460).
2. Async execution, streaming uploads, cookies, or general-purpose CA management.
3. Preserving authorization across redirects; redirect requests drop it.

## Architecture

`toolu-http` depends on `toolu-runtime::env::Env`, `ureq` 3 and rustls with
`ring`. It builds a blocking agent for each request scheme from a supplied
environment snapshot. HTTPS prefers `HTTPS_PROXY`/`https_proxy`, HTTP prefers
`HTTP_PROXY`/`http_proxy`, then either uses `ALL_PROXY`/`all_proxy`; explicit
scheme settings outrank the fallback. A missing proxy means direct transport.
Requests have a nonzero global deadline and a bounded response body. Ureq
follows at most five redirects and drops `Authorization` on each hop; HTTP
errors are examined after bounded body reading. A test-only CA field
provides exact trusted roots to the test fixture; production uses WebPKI roots.

`crates/http-test-support` is a workspace crate consumed as a dev-dependency.
It supplies a real in-process rustls server and CONNECT proxy, generated test
certificate, request capture, and fixed response routes. It imports rustls
through a `toolu-http` `test-support` feature so the HTTP crate remains the
only direct TLS capability owner. The feature is absent from release builds.

## Interfaces / Schema

- `Config { timeout: Duration, max_body_bytes: usize, max_redirects: u32,
  test_root_ca_der: Option<Vec<u8>> }`, with a 30-second timeout and 1 MiB body
  default. `test_root_ca_der` is deliberately named and documented for tests.
- `Client::new(config: Config, env: &Env) -> Result<Client, Error>` takes the
  environment snapshot and validates nonzero timeout/body limit.
- `Auth::{None, Basic { username, password }, Bearer(String)}` is passed to
  `get_json<T>`, `post_json<B,T>`, `put_json<B,T>`, and `get_bytes` methods.
  JSON methods serialize requests and deserialize bounded responses with
  `serde`; `get_bytes` returns bounded bytes after redirects.
- `Error::{InvalidConfig, HttpStatus(u16), Timeout, BodyTooLarge,
  Encode, Decode, Transport}` distinguishes caller, server, deadline, size,
  JSON, and network failures. It implements `Display` and `std::error::Error`.
- The fixture starts on ephemeral loopback ports, exposes its proxy URL,
  certificate DER, URL for `api.example.test`, captured requests, and a
  controlled shutdown on drop. Its routes can return status/body/headers,
  redirect, or deliberately hold a response for timeout tests.

## Failure modes and edge cases

- Zero timeout or body limit, malformed proxy URL, or invalid test CA returns
  `InvalidConfig` before any request. Empty bearer/basic values are allowed as
  caller data. Proxy variables with empty values count as absent.
- An HTTP 4xx/5xx becomes `HttpStatus(code)` after the response is bounded;
  oversized success or error responses become `BodyTooLarge`.
- A timeout at connect, TLS, redirect, or body read becomes `Timeout`.
  Connection, TLS, invalid URL, and too-many-redirect errors become
  `Transport`.
- An empty JSON response becomes `Decode`; an empty byte download succeeds.
- Every redirect drops `Authorization`, including a change of hostname,
  scheme, or port. Redirect count exhaustion is `Transport`.
- Concurrent calls use independent request state; no mutable process
  environment or shared fixture port is required.

## Acceptance criteria

- **AC-1:** GET, POST, and PUT against the fixture return decoded JSON;
  request capture proves methods, JSON bodies, basic and bearer headers.
- **AC-2:** An HTTPS request for `api.example.test` goes through a real
  `HTTPS_PROXY` CONNECT endpoint on the fixture's ephemeral port and trusts
  only the supplied test CA.
- **AC-3:** Timeout, oversized body, HTTP error, malformed JSON, and failed
  transport produce their distinct typed errors from real fixture behavior.
- **AC-4:** A redirected byte download reaches a second origin, and its
  captured request has no `Authorization` header.
- **AC-5:** The static musl release-profile transport test links `toolu-http`
  and makes TLS requests with `ring`. A separate stripped release probe that
  links the production client stays at or below 4 MiB; the current `toolu`
  release binary also stays within 4 MiB. `cargo tree -p toolu-http` has
  neither Tokio nor Hyper. The production CLI does not yet link this crate;
  #460 must repeat its final linked-binary measurement.
- **AC-6:** The full Rust quality gate passes with the reusable fixture
  crate registered and no gate-data exception.

## Acceptance evidence

| AC | Real input and expected observation | Boundary case | Check |
|---|---|---|---|
| AC-1 | Loopback TLS server receives all methods and JSON/auth headers; caller receives decoded objects | Empty auth text | `cargo test -p toolu-http` |
| AC-2 | CONNECT proxy receives `api.example.test:<fixture-port>`; supplied CA validates its TLS certificate | Same call without CA fails TLS | `cargo test -p toolu-http` |
| AC-3 | Delayed, large, 404, malformed, and refused loopback responses map to named error variants | Error body over limit | `cargo test -p toolu-http` |
| AC-4 | 302 to a different loopback TLS origin returns bytes; target capture lacks authorization | Changed port with same hostname | `cargo test -p toolu-http` |
| AC-5 | Release-profile musl transport test makes HTTPS calls through the fixture; stripped linked client probe and current CLI are each static and ≤4 MiB | Future CLI linkage belongs to #460 | `cargo test --release --target x86_64-unknown-linux-musl -p toolu-http --test transport`; build both binaries; `file`; `stat`; `cargo tree -p toolu-http` |
| AC-6 | Gate exits 0 on this branch | New crate layout and coverage | `cargo xtask gate` |

## Documentation impact

Document the client configuration, proxy precedence, redirect policy, error
contract, and fixture use in `crates/core/http/README.md`. Update
`docs/resource-budgets.md` only if a new measurement is needed; its 4 MiB
limit cannot change in this product PR.

## Open Questions

None. `ring`, explicit environment selection, and the stricter redirect policy
were chosen from the issue, gates, and library behavior. The binary budget is
an acceptance check, not an unresolved design choice.
