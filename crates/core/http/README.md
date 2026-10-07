# toolu-http

`toolu-http` is the blocking JSON and download transport shared by Rust Jev
and GitHub clients. It uses ureq 3 with rustls and the explicit `ring` crypto
provider. Every request has a whole-request timeout (default 30 seconds), a
response-body cap (default 1 MiB), and at most five redirects. JSON GET, POST,
and PUT decode with `serde`; `get_bytes` returns bounded download bytes.

Construct `Client::new(Config, &Env)` with a `toolu-runtime::env::Env`
snapshot. For HTTPS it selects `HTTPS_PROXY`, then `https_proxy`, then
`ALL_PROXY`, then `all_proxy`; HTTP selects its `HTTP_PROXY` and `http_proxy`
equivalents first. Empty values are absent. Without a selected proxy it
connects directly. A malformed selected proxy fails client construction.

`Auth::Basic` and `Auth::Bearer` set the initial `Authorization` header.
Redirects are followed for downloads, and **every redirected request drops
Authorization**, including one to another origin. `Config::max_redirects` of 0
returns a 3xx response as it is.

`Client::send(&Request)` is the lower-level call the GitHub and Jev clients use.
It takes a `Method`, a URL, an `Auth`, extra headers and an optional body. It
returns a `Response` with the status, the headers (names in lowercase,
`Response::header` compares without case) and the bounded body for **every**
status, so `304`, `401` and `retry-after` reach the caller. Only invalid
configuration, timeout, body cap and transport failures are errors. The caller
owns `Content-Type`. `send` refuses `Authorization`, `Proxy-Authorization` and
`Cookie` headers with `InvalidConfig`: credentials go through `Auth`, which
redirects drop. The JSON and byte helpers are built on `send` and turn a status
of 400 or more into `HttpStatus`.

`Auth` and `Client` have hand-written `Debug`: a password or bearer prints as
`<redacted>`, and a client shows its `Config` but no environment value. `Error` distinguishes
configuration, HTTP status, timeout, body cap, JSON encoding/decoding, and
transport failures. An oversized HTTP error body reports `BodyTooLarge`
before `HttpStatus`.

`Config::test_root_ca_der` replaces the trusted roots with one DER certificate
for loopback tests only. Production leaves it unset and uses the bundled
WebPKI roots. Rust tests use `toolu-http-test-support` for a generated
certificate, ephemeral TLS origins, and a real CONNECT proxy; see that
crate's README.
