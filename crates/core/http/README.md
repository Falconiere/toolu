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
Authorization**, including one to another origin. `Error` distinguishes
configuration, HTTP status, timeout, body cap, JSON encoding/decoding, and
transport failures. An oversized HTTP error body reports `BodyTooLarge`
before `HttpStatus`.

`Config::test_root_ca_der` replaces the trusted roots with one DER certificate
for loopback tests only. Production leaves it unset and uses the bundled
WebPKI roots. Rust tests use `toolu-http-test-support` for a generated
certificate, ephemeral TLS origins, and a real CONNECT proxy; see that
crate's README.
