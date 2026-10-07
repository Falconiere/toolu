# toolu-jev-client

`toolu-jev-client` is the Jev HTTP client that `toolu jev` (#430), pr-babysit
(#433) and the epic engine (#435) share. It sits at the `engine` layer and
depends on `toolu-runtime` and `toolu-http` only. The package is not
`toolu-jev`, because that name belongs to the jev plugin's crate.

## Configuration

- **Key:** `Jev::from_env(&Env, Config)` reads `TYPESAFE_API_KEY`. Unset is
  `Error::MissingKey` and a line break is `Error::KeyLineBreak`, both before any
  request. The key is sent as `Authorization: Bearer`, and neither `Jev`'s
  `Debug` nor any error prints it. An error body that echoes it is redacted.
- **Timeout:** `JEV_TIMEOUT` is the per-attempt deadline in seconds, 60 by
  default. A negative, non-numeric or unrepresentable value also gives 60. `0`
  makes every attempt time out without connecting. An empty value counts as
  unset (the `Env` rule), whereas TypeScript reads it as 0.

## Requests

`ask(&State, model, &Questions)` posts `{state, model, questions}` to
`https://api.typesafe.ai/v1/systemone` without following redirects.

The state and questions are `toolu_runtime::json::ordered::Ordered`, so the body
is the `JSON.stringify` text `jev.ts` sends: keys in order, and a repeated key
in its first place with its last value.

- **Building questions:**
  - `Question::noul` takes optional true and false sides; an empty one is left
    out.
  - `Question::choice` takes 2 to 255 options, kept in order.
  - `Question::score` takes 2 to 10 levels, lowest first.
  - `Questions::single` puts one question under an id.
- **`ask` payloads:** `Questions::parse` takes a non-empty JSON object and sends
  it as given. Structured instructions, structured criteria and unknown fields
  pass through.
- **State:** `State::text` sends a string. `State::structured` keeps a JSON
  object or array and sends any other text as a string.

One difference from TypeScript: a choice option whose key looks like an integer
(`"1"`) keeps its place, where JavaScript would move it first.

## Retries

A call makes up to three attempts, as `plugins/jev/hooks/src/jev.ts` does.

- **Retried statuses:** 408, 429 and 5xx, after a delay in pause units
  (`Config::pause`, 1 s):
  - an all-digit `retry-after` of at most 2 digits;
  - otherwise an all-digit `retry-after-ms` of at most 8 digits, rounded up;
  - otherwise 2^(attempt-1);
  - and never less than 2^(attempt-1).

  A delay over 60 units, or a longer header, returns the status at once.
- **Transport failures and timeouts** retry after 2^(attempt-1) units.
- **Final errors:** `Error::Http { status, body }`, `Error::Timeout` or
  `Error::Transport`. A body over the 1 MiB cap is not retried.

## Replies

The reply is checked as `jev/response.ts` checks it, and anything else is
`Error::InvalidResponse`:

- a non-empty model;
- non-negative integer usage counts (a count above `u64::MAX` saturates);
- one answer per question id, of the question's type;
- probabilities in [0, 1];
- choice and score distributions over exactly the options or levels, summing
  to 1 within 1e-6;
- a choice among the options;
- a score within the levels, with their legend.

`Reply` holds the typed answers in question order, plus the raw body, which
the CLI prints unchanged.

## Tests

`tests/cases.rs` and `tests/retries.rs` replay the request and response cases
of `plugins/jev/hooks/src/__tests__/jev.test.ts` that do not concern argv. They
run against `toolu-http-test-support`'s real TLS origin and CONNECT proxy, with
a 10 ms pause unit.
