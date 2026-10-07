# toolu-github

`toolu-github` is the GitHub REST and GraphQL client that pr-babysit (#433),
the epic engine's actions (#435) and its GitHub checks (#447) share. It sits at
the `engine` layer and depends on `toolu-runtime` and `toolu-http` only.

## Token

`Client::new(Config, &Env)` reads the token once. It uses `GH_TOKEN`, else the
output of `gh auth token`, run through `toolu_runtime::process` with the `Env`
snapshot as its whole environment and a 10-second deadline. `gh` itself honours
`GITHUB_TOKEN` and its `hosts.yml`.

- With neither source, the error names both: ``no GitHub token: GH_TOKEN is
  unset and `gh auth token` failed: <gh's reason>``.
- A token with whitespace or a control character inside it is refused before
  any request.
- The token goes out as `Authorization: Bearer`.
- On a `401` it is read again once. The request is retried only if the token
  read differs from the one refused, so a fixed `GH_TOKEN` gives
  `Error::Unauthorized` after one request. Threads sharing a client and refused
  together all retry with the new token.
- The token is never written to disk. `Client`'s `Debug` shows only the API URL
  and the retry policy.
- Every token the client has read is redacted from error messages taken from a
  response.

## Calls

- `get(path, etag)` and `rest(&RestRequest)` send a REST request. An `ETag`
  makes it conditional (`If-None-Match`); an unchanged resource answers
  `Rest::NotModified`.
- A path starts with `/`, or with the API URL and `/` (a pagination link). Any
  other path, `//host` included, is `Error::Config` with no request sent, and so
  is a path or `ETag` with a space, a control character or non-ASCII text, or a
  path the URI parser refuses (`<`, a backtick). Percent-encode those. The API URL must be an `https://` origin.
- `graphql(query, variables)` posts to `<api>/graphql` and returns the body's
  `data`. A non-empty `errors[]` is `Error::GraphQl`, whose messages are
  redacted.
- Redirects are followed without `Authorization` (toolu-http). Replies are
  capped at 8 MiB.

Every `Reply` carries the final response's `Cost`. Its `points` are 0 for a
REST `304` (GitHub does not count it) and 1 for any other REST reply. For
GraphQL they are the `rateLimit { cost }` the query selected, if it did. `rate`
holds the `x-ratelimit-*` headers. `Cost` serializes for the engine's journal.

## Retry policies

- `Config::one_shot(&Env)` keeps pr-babysit's gh semantics
  (`plugins/pr-babysit/hooks/src/babysit/gh.ts`):
  - `PB_GH_ATTEMPTS`: attempts, default 3;
  - `PB_GH_BACKOFF`: the wait after each failed attempt, default `2 4 8`, and
    2 s past the list;
  - `PB_GH_TIMEOUT`: seconds per attempt, default 60.

  An invalid value is an `Error::Config` that names the variable.
  - Transient: 5xx, 429, timeouts and network errors.
  - A 403 is transient when it is a rate limit, and otherwise permanent like
    every other 4xx.
  - A rate limit waits for an integer `retry-after`, or for
    `x-ratelimit-reset` when `x-ratelimit-remaining` is 0. A wait over 60 s
    returns `Error::RateLimited` without sleeping.
- `Config::scheduled()` is for the engine's fixed 3-minute checks. It makes one
  attempt, and returns a rate limit with its wait instead of sleeping.

## Tests

The tests run against `toolu-http-test-support`'s real TLS origin and CONNECT
proxy. `gh` must be on `PATH`: the token tests run the real `gh` (verified with
2.101) against a temporary `GH_CONFIG_DIR` whose `hosts.yml` they write, rewrite
and break. A missing `gh` fails these tests instead of skipping them.

`tests/token_leak.rs` has no libtest harness. It runs itself as a child with a
sentinel token, drives every path, and finds the sentinel nowhere in the
child's stdout, stderr, final panic message or journal.
