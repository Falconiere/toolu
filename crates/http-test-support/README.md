# toolu-http-test-support

This workspace crate provides a reusable in-process HTTPS and CONNECT proxy
fixture for Rust tests. It is used as a dev-dependency of `toolu-http` and can
be reused by #430, #435, #447, and #460 (`toolu-github` and `toolu-jev-client`
use it). It opens ephemeral loopback ports,
generates a certificate for `api.example.test`, and captures decrypted HTTP
requests and CONNECT authorities. Dropping `Fixture` stops its listeners.

Use `Fixture::start()`, register exact path responses with `route(path,
Reply::new(status, body))`, and pass `root_ca_der()` to
`Config::test_root_ca_der`. Put `proxy_url()` in a
`toolu-runtime::env::Env` snapshot as `HTTPS_PROXY`, then request `url(path)`.
`second_url(path)` uses the same hostname on another port to test a genuine
cross-origin redirect. `Reply::header` adds a response header and
`Reply::delayed` exercises deadlines. `sequence(path, replies)` answers a path's
requests with its replies in order, then repeats the last one; `route` is a
one-reply sequence and an empty sequence removes the route.
`Reply::dropped()` reads the request, records nothing and closes the connection
without a response. The proxy half-closes toward the client as soon as the
origin closes, so the client sees the drop at once. `requests()` and
`connects()` return captured evidence.

The fixture uses `toolu-http`'s `test-support` feature for rustls types so
the HTTP crate remains the sole direct TLS dependency owner under the layer
gate. Neither the feature nor the fixture is linked by the release CLI.
