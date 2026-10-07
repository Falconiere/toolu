# Shared GitHub and Jev clients, cross-plugin traits — Design

**Date:** 2026-10-07   **Status:** Approved   **Author:** Claude   **Topic:** Falconiere/toolu#460

## Problem

pr-babysit (#433), the epic engine's actions and GitHub checks (#435, #447)
each need a GitHub REST/GraphQL client, and `toolu jev` (#430), pr-babysit and
the engine each need the Jev client. Writing them per plugin would break the
zero-clone rule, and pr-babysit's tick (used by the engine) and the hub's status
snapshot (used by statusline) would need plugin-to-plugin edges, which
`check-layers` rejects. Brainstorm:
`docs/toolu/brainstorms/2026-10-07-shared-github-jev-clients.md`.

## Non-Goals

1. Calling either client from a plugin crate or adding a `toolu` verb: #430,
   #433, #435 and #447 do that.
2. Porting the babysit tick (#433), `toolu status` (#445), the statusline
   (#431) or the engine loop, journal and attention queue (#434, #450).
3. GitHub Enterprise URLs, webhooks, pagination helpers, or GraphQL
   rate-limit retries (`gh.ts` classifies a GraphQL error as permanent too).
4. Changing gate data: `layers.json` already places `github` and `jev` at the
   engine layer and lists the rule crates the hub links.
5. Printing Jev answers (`--raw`, compact output) and mapping errors to exit
   codes. #430 does that; if it needs the curl-style transport codes (6, 7,
   55, 56), #430 adds a transport kind to `toolu_http::Error`.
6. `toolu-state` as a GitHub dependency. The issue lists it, but the caller
   keeps the ETag, so the client stores nothing, and cargo-machete rejects an
   unused dependency.

## Architecture

**Transport (`crates/core/http`).**
- `Client::send(&Request) -> Result<Response, Error>` sends extra request
  headers and returns the status, headers and bounded body for every status.
  Only invalid configuration, timeout, body cap and transport failures are
  errors.
- `send` rejects the header names `authorization`, `proxy-authorization` and
  `cookie` (any case) with `InvalidConfig`. Credentials travel only through
  `Auth`, which redirects drop. The caller owns `Content-Type` in `send`.
- The JSON and byte helpers are rebuilt on `send` and keep their behaviour:
  they set `Content-Type` themselves, and a status of 400 or more becomes
  `HttpStatus` after the body cap.
- `Auth` and `Client` get hand-written `Debug` that never prints a credential
  or an environment value.
- `lib.rs` already has 257 of its 300 code lines, so the crate splits into:
  - `lib.rs`: `Config`, `Client::new`, the helpers, agent and body reading;
  - `auth.rs`: `Auth`, its header value and its `Debug`;
  - `send.rs`: `Method`, `Request`, `Response`, header lookup and checks,
    `Client::send`;
  - `error.rs`: `Error` and the ureq/io mapping.

  Each module has its `tests/<module>_test.rs`.

**GitHub (`crates/core/github`, package `toolu-github`, depends on
`toolu-runtime`, `toolu-http`, serde, serde_json).**

*Token.* `Client::new(Config, &Env)` reads the token once: `GH_TOKEN`, else
`gh auth token`, run through `toolu_runtime::process` with the snapshot as its
whole environment and a 10-second deadline.
- It is sent as `Authorization: Bearer`.
- On a `401` it is re-read once; the request is retried only if the new token
  differs.
- The token lives behind a `Mutex`, which is not held while `gh` runs.
- Every token the client has read is redacted from every message it builds.
- `process::Spec` and `process::Output` are never formatted, because their
  derived `Debug` carries the environment and stdout.

*Calls.*
- REST takes an optional ETag and returns `NotModified` or `Fresh`.
- GraphQL posts `{query, variables}` to `<api>/graphql`. Its reply data is the
  body's `data` member; a non-empty `errors[]` is an error.
- Every reply carries the final response's `Cost` and the number of attempts.

*Retry policies.*
- `Config::one_shot(&Env)` keeps `gh.ts`: `PB_GH_ATTEMPTS` (default 3),
  `PB_GH_BACKOFF` ("2 4 8") and `PB_GH_TIMEOUT` (60 s per attempt).
  - Transient: 5xx, 429, a rate-limited 403, timeouts and network errors.
  - Permanent: every other 4xx, and the body cap.
- `Config::scheduled()` serves the engine's fixed 3-minute checks: one
  attempt; a rate limit is returned with its wait and never slept.

*Modules.* Each has a unit-test file, and the GitHub retry loop does not
resemble Jev's:
- `lib.rs`: `Config`, `Client`;
- `token.rs`: sources, validation, redaction;
- `policy.rs`: `Retry`, the `PB_GH_*` parsing, classification and wait;
- `cost.rs`: `Cost` and `RateLimit` from headers;
- `call.rs`: the attempt loop, request headers, URL resolution;
- `rest.rs`;
- `graphql.rs`;
- `error.rs`.

**Jev (`crates/core/jev`, package `toolu-jev-client`, depends on
`toolu-runtime`, `toolu-http`, serde_json).**
- `Jev::from_env(&Env, Config)` reads `TYPESAFE_API_KEY` and `JEV_TIMEOUT`.
- It posts `{state, model, questions}` to
  `https://api.typesafe.ai/v1/systemone` with `jev.ts`'s retries and no
  redirect following (`max_redirects` 0).
- State and questions are `toolu_runtime::json::ordered::Ordered`, which keeps
  JavaScript key order, collapses a repeated key to its first position and last
  value, and prints `JSON.stringify` text. The request body is therefore the
  text `jev.ts` sends, without a new serde feature.
- The reply is checked exactly as `response.ts` checks it and returned typed,
  with the raw body kept for the CLI.
- Modules, each with a unit-test file:
  - `lib.rs`: `Config`, `Jev`, the key;
  - `question.rs`: `State`, `Question`, `Questions`;
  - `retry.rs`: the delay, retryable statuses and the attempt loop;
  - `reply.rs`: `Reply`, `Answer` and the validation;
  - `error.rs`.

**Traits (`crates/core/engine`).**
- `BabysitTick` and `StatusSnapshot` (both `Send + Sync`) with their request
  and response types, sharing one `LinkError`. Files: `lib.rs`
  (`LinkError`), `babysit.rs`, `status.rs`.
- Until their ports, `toolu_pr_babysit::Tick` returns
  `NotPorted { issue: 433 }` and `toolu_hub::status::Snapshot` returns
  `NotPorted { issue: 445 }`.
- `toolu_epic_orchestrator::run` takes a `&dyn BabysitTick`, and
  `toolu_statusline::run` a `&dyn StatusSnapshot`. Their placeholders ignore it,
  as they ignore `matches` today.
- `crates/cli/src/links.rs` holds the two implementations as consts and the two
  wrapper functions the registry names.
- `toolu_epic_orchestrator::babysit::next` is the engine step that turns a tick
  into `KeepGoing`, `MergeQueue` or `Attention`. Any error becomes `Attention`,
  never a panic (Jev: 0.51 for this step against 0.40 for a core-level
  conversion and 0.09 for deferring the attention half).

**Fixtures.**
- `crates/http-test-support` gains per-path reply sequences (`routes.rs`) and
  a reply that drops the connection (`reply.rs`).
- `fixtures/guardrails/rust/layers` gains a passing trait-wiring workspace and
  a failing epic-orchestrator → pr-babysit edge.

## Interfaces / Schema

```rust
// toolu_http
pub enum Method { Get, Post, Put, Patch, Delete }
pub struct Request<'a> { pub method: Method, pub url: &'a str, pub auth: &'a Auth,
  pub headers: &'a [(&'a str, &'a str)], pub body: Option<&'a [u8]> }
pub struct Response { pub status: u16, pub headers: Vec<(String, String)>, pub body: Vec<u8> }
impl Response { pub fn header(&self, name: &str) -> Option<&str> } // case-insensitive, first
impl Client { pub fn send(&self, request: &Request<'_>) -> Result<Response, Error> }

// toolu_http_test_support
impl Reply { pub fn dropped() -> Reply } // read the request, record nothing, close without a response
impl Fixture { pub fn sequence(&self, path: &str, replies: Vec<Reply>) -> Result<(), Error> }
// served in order; the last reply repeats once the others are used; `route` is a one-reply sequence
```

```rust
// toolu_github
pub const API_URL: &str = "https://api.github.com";
pub struct Config { pub api_url: String, pub retry: Retry, pub http: toolu_http::Config }
impl Config { pub fn one_shot(env: &Env) -> Result<Config, Error>; pub fn scheduled() -> Config }
pub struct Retry { pub attempts: u32, pub backoff: Vec<Duration>, pub max_wait: Duration }
pub struct Client; // Debug: api_url and policy only
impl Client {
  pub fn new(config: Config, env: &Env) -> Result<Client, Error>;
  pub fn get(&self, path: &str, etag: Option<&str>) -> Result<Reply<Rest>, Error>;
  pub fn rest(&self, request: &RestRequest<'_>) -> Result<Reply<Rest>, Error>;
  pub fn graphql(&self, query: &str, variables: &serde_json::Value) -> Result<Reply<serde_json::Value>, Error>;
}
pub struct RestRequest<'a> { pub method: Method, pub path: &'a str,
  pub body: Option<&'a serde_json::Value>, pub etag: Option<&'a str> }
pub enum Rest { NotModified, Fresh(Fresh) }
pub struct Fresh { pub status: u16, pub etag: Option<String>, pub headers: Vec<(String, String)>, pub body: Vec<u8> }
impl Fresh { pub fn json<T: DeserializeOwned>(&self) -> Result<T, Error> }
pub struct Reply<T> { pub data: T, pub cost: Cost, pub attempts: u32 }
#[derive(Serialize)] pub struct Cost { pub points: Option<u64>, pub rate: RateLimit }
#[derive(Serialize)] pub struct RateLimit { pub resource: Option<String>, pub limit: Option<u64>,
  pub remaining: Option<u64>, pub used: Option<u64>, pub reset: Option<u64> }
pub enum Source { Env, Gh }
pub enum TokenError { Unavailable { gh: String }, Malformed(Source) }
pub enum Error { Token(TokenError), Config(String), Unauthorized,
  RateLimited { status: u16, retry_after: Option<Duration> },
  Status { status: u16, message: String }, GraphQl(Vec<String>),
  Transport(toolu_http::Error), Decode(String) }
```

- **Request headers:** `Accept: application/vnd.github+json`,
  `X-GitHub-Api-Version: 2022-11-28`, `User-Agent: toolu/<version>`,
  `If-None-Match` when an ETag is given, and `Content-Type: application/json`
  with a body.
- **`api_url`:** must be `https://` and is trimmed of a trailing `/`. Anything
  else → `Error::Config` from `Client::new`. The fixture origin is
  `https://api.example.test:<port>`, so no exception is needed.
- **Request path:** starts with a single `/` (not `//`), or with `api_url` +
  `/` (a pagination link). Anything else → `Error::Config`, with no request
  sent: `https://api.github.com.evil.example/x`, `//evil.example/x`, or a
  relative path. A `/` path is appended to `api_url` as a string, never
  through a URL join, which would treat `//host` as a new host. An
  `api_url/…` link is used as is.
- **`Cost`:**
  - `points` describes the final response: REST 0 for `304`, else 1; GraphQL
    `data.rateLimit.cost` when the query selects it.
  - `rate` comes from that response's
    `x-ratelimit-{resource,limit,remaining,used,reset}` headers.
- **Wait:** `retry-after` when it is an integer of seconds (an HTTP date is
  ignored). Otherwise, with `x-ratelimit-remaining: 0`, `x-ratelimit-reset`
  minus now. Otherwise this attempt's backoff (2 s when the list is shorter).
  A wait over `max_wait` (60 s one-shot, 0 scheduled) returns `RateLimited`
  without sleeping.
- **`TokenError::Unavailable`** displays ``no GitHub token: GH_TOKEN is unset
  and `gh auth token` failed: <reason>``. The reason is gh's last non-empty
  stderr line, `exited <code>` without one, the spawn error, `timed out`, or
  `printed no token`.

```rust
// toolu_jev_client
pub const ENDPOINT: &str = "https://api.typesafe.ai/v1/systemone";
pub const DEFAULT_MODEL: &str = "jev-latest";
pub struct Config { pub endpoint: String, pub pause: Duration, pub test_root_ca_der: Option<Vec<u8>> }
pub struct Jev; // Debug: endpoint and timeout only
impl Jev { pub fn from_env(env: &Env, config: Config) -> Result<Jev, Error>;
  pub fn ask(&self, state: &State, model: &str, questions: &Questions) -> Result<Reply, Error> }
pub struct State;
impl State { pub fn text(text: &str) -> State; pub fn structured(text: &str) -> State }
pub struct Question;
impl Question {
  pub fn noul(instructions: &str, yes: Option<&str>, no: Option<&str>) -> Question;
  pub fn choice(instructions: &str, options: &[(&str, Option<&str>)]) -> Result<Question, Error>;
  pub fn score(instructions: &str, levels: &[&str]) -> Result<Question, Error> }
pub struct Questions;
impl Questions { pub fn single(id: &str, question: Question) -> Questions;
  pub fn parse(text: &str) -> Result<Questions, Error> }
pub struct Reply { pub model: String, pub usage: Usage, pub answers: Vec<(String, Answer)>, pub body: String }
pub struct Usage { pub input_tokens: u64, pub output_tokens: u64 }
pub enum Answer { Noul { noul: f64 },
  Choice { choice: String, probabilities: BTreeMap<String, f64>, confidence: f64 },
  Score { score: f64, legend: BTreeMap<String, serde_json::Value>,
          probabilities: BTreeMap<String, f64>, confidence: f64 } }
pub enum Error { MissingKey, KeyLineBreak, InvalidQuestion(String),
  Http { status: u16, body: String }, Timeout, Transport(toolu_http::Error), InvalidResponse }
```

- **Body:** `{"state":…,"model":"…","questions":{…}}`, the `Ordered` text,
  ids in order.
- **Headers:** `Authorization: Bearer <key>`, and `Content-Type` and `Accept`
  both `application/json`.
- **Builders:**
  - every builder rejects empty `instructions` with `InvalidQuestion` (the
    TypeScript CLI shows usage and sends nothing);
  - noul: criteria `{"true":…,"false":…}` with only the non-empty sides, and
    omitted when neither is given (TypeScript skips an empty side);
  - choice: 2 to 255 options, empty key rejected, criteria in option order (a
    repeated key keeps its first position and last value, as `jev.ts`'s
    `defineProperty` does);
  - score: 2 to 10 levels, criteria is the level array.
- **`State::structured`** keeps a JSON object or array as JSON and any other
  text (`123`, `true`, invalid JSON) as a string, like `jev.ts`'s
  `structured()`.
- **`Questions::parse`** takes a non-empty JSON object. A question with an
  unknown or missing `type` is sent unchanged, as in TypeScript.
- **Retries:** 3 attempts.
  - A non-2xx retries when it is 408, 429 or ≥ 500 and its delay is defined.
  - The delay, in pause units, is the larger of 2^(attempt-1) and:
    - `retry-after` when it is all digits: undefined if it has 3 or more
      digits, otherwise its value, and `retry-after-ms` is not consulted;
    - else `ceil(retry-after-ms / 1000)` when that is all digits: undefined if
      it has 9 or more digits;
    - else nothing.
  - A value over 60 makes the delay undefined, so the status is returned.
  - A transport error or timeout retries after 2^(attempt-1) units.
  - The last failure is `Http { status, body }`, `Timeout` or `Transport`.
    `BodyTooLarge` is not retried.
- **`JEV_TIMEOUT`:**
  - seconds per attempt through `Duration::try_from_secs_f64`;
  - default 60, which also covers negative, non-finite, non-numeric or
    unrepresentable values;
  - `0` means `send` is not called and the attempt is a `Timeout`;
  - an empty value is unset (the `Env` rule), where TypeScript's `Number("")`
    gives 0.
- **Usage counts:** accepted when `Number.isInteger` would accept them (`3`,
  `3.0`, `1e3`) and they are not negative. A count above `u64::MAX` (`1e30`)
  saturates, because TypeScript accepts it.

```rust
// toolu_engine
pub enum LinkError { NotPorted { issue: u32 }, Failed(String) }
pub struct TickRequest { pub repo: String, pub number: u64, pub state_file: PathBuf, pub now: Option<String> }
pub enum TickDecision { KeepGoing, Success, Escalate }
pub struct TickReport { pub decision: TickDecision, pub result: serde_json::Value }
pub trait BabysitTick: Send + Sync { fn tick(&self, request: &TickRequest) -> Result<TickReport, LinkError>; }
pub trait StatusSnapshot: Send + Sync {
  fn snapshot(&self, roots: &Roots, dir: &Path) -> Result<serde_json::Value, LinkError>; }

// toolu_epic_orchestrator
pub fn run(matches: &ArgMatches, ctx: &Ctx, tick: &dyn BabysitTick) -> Outcome;
pub mod babysit { pub enum Next { KeepGoing, MergeQueue, Attention(String) }
  pub fn next(tick: &dyn BabysitTick, request: &TickRequest) -> Next; }
// KeepGoing → KeepGoing; Success → MergeQueue; Escalate → Attention("babysit escalated o/r#n");
// Err(e) → Attention("babysit tick for o/r#n failed: {e}"). `result` is kept for the journal, not read.
// toolu_statusline
pub fn run(matches: &ArgMatches, ctx: &Ctx, status: &dyn StatusSnapshot) -> Outcome;
// crates/cli/src/links.rs
pub(crate) const BABYSIT_TICK: &dyn BabysitTick = &toolu_pr_babysit::Tick;
pub(crate) const STATUS_SNAPSHOT: &dyn StatusSnapshot = &toolu_hub::status::Snapshot;
```

The snapshot's tick flags of `babysit-tick.ts` (`--snapshot-in/out`,
`--page-size`, `--timeout`) are #433's to add to `TickRequest`. It is
workspace-internal, so no compatibility constraint applies.

## Failure modes and edge cases

**GitHub client.**
- **No token:** `GH_TOKEN` unset or empty, and `gh` missing, failing, timing
  out or printing nothing → `Error::Token(Unavailable { gh })` from
  `Client::new`.
- **Malformed token:** a value with a control character or inner whitespace →
  `Malformed(source)`, with no value in the message. No request is sent.
- **401:** re-read once; the re-read does not use up an attempt.
  - A second 401, the same token (always the case with `GH_TOKEN`), or a failed
    re-read → `Unauthorized`.
  - Both tokens are redacted from then on.
- **Rate limits:**
  - A 403/429 with an integer `retry-after`, or with
    `x-ratelimit-remaining: 0`, waits per the rule above or returns
    `RateLimited`.
  - A 403 whose body mentions "rate limit" is transient with backoff; any
    other 403 is permanent `Status`.
- **Exhausted retries:** the last error. `Status` carries the JSON body's
  `message` (or `""`), redacted. A network failure gives `Transport`.
- **Configuration errors**, each an `Error::Config` naming its variable or
  field:
  - `PB_GH_ATTEMPTS` that is not a positive integer;
  - a `PB_GH_BACKOFF` entry that is not a non-negative number;
  - `PB_GH_TIMEOUT` that is not a positive number;
  - a hand-built `Retry { attempts: 0 }`, rejected by `Client::new`.
- **A `304` without an ETag sent:** still `NotModified`.
- **Redirects:** followed with Authorization dropped (toolu-http). A
  redirected private resource then answers 404, which is permanent.
- **GraphQL:** a `200` with `errors[]` → `GraphQl(messages)`, permanent. A
  missing `data` → `Decode`.
- **Body size:** the default cap is 8 MiB, because GraphQL review-thread
  replies outgrow toolu-http's 1 MiB; over the cap → `Transport(BodyTooLarge)`,
  permanent.
- **Concurrency:** the token `Mutex` recovers a poisoned lock with
  `into_inner`. Calls share no other mutable state.
- `Error` does not carry the attempt count. The scheduled engine policy makes
  one attempt, and one-shot callers print the error.

**Jev client.**
- **Key:** unset → `MissingKey`, and a key with CR or LF → `KeyLineBreak`,
  both from `from_env` before any request.
- **Questions:** each of these → `InvalidQuestion` before any request:
  - a choice outside 2 to 255 options, or with an empty key;
  - a score outside 2 to 10 levels;
  - an `ask` payload that is not a non-empty JSON object.
- **Reply** → `InvalidResponse` when any of these holds:
  - it is not JSON;
  - `model` is missing or empty;
  - a usage count is missing, negative or not an integer;
  - the answer ids differ from the question ids;
  - an answer has the wrong type, or its question's type is unknown;
  - a choice question's `criteria` is not an object, or a score question's is
    not an array (an `ask` payload is sent as given);
  - a probability is outside [0, 1];
  - a distribution's keys differ, or its sum is off by 1e-6 or more;
  - a choice is outside its options;
  - a score is outside [0, n-1], or its legend keys are not "0" to "n-1".
- **Integer-like keys:** a choice option named `1` keeps its insertion
  position. JavaScript moves integer-like keys first. The difference is
  documented in the README and changes only the order Jev sees.

**Traits.** The injected tick and snapshot return `NotPorted`, so
`babysit::next` gives `Attention("babysit tick for o/r#n failed: … not ported
yet (#433)")`. The `toolu epic` and `toolu statusline` outputs do not change.

## Acceptance criteria

- **AC-1:** `cargo xtask check-layers` on this repository passes, with
  pr-babysit, epic-orchestrator, jev, statusline and toolu built and no plugin
  crate depending on another plugin crate.
  - The `layers/clean-trait-wiring` fixture passes: engine traits, an owner
    implementation, a consumer, and a cli linking both.
  - The `layers/violating-epic-babysit` fixture fails with
    "crates/epic-orchestrator (toolu-epic-orchestrator, plugin crate) depends
    on toolu-pr-babysit …: a plugin crate may not depend on another plugin
    crate".
- **AC-2:** Against the loopback HTTPS fixture through its CONNECT proxy, with
  a real `gh` reading a temporary `hosts.yml`:
  - a GET with an ETag sends `If-None-Match` and returns `NotModified` with
    0 points, where the first GET returned 1 point and the ETag;
  - a `401` after `hosts.yml` changed retries with the new token and succeeds
    on request 2;
  - a `401` with the token unchanged returns `Unauthorized` after one request;
  - a `304` with no ETag sent is still `NotModified`;
  - a pagination link `<api_url>/repos/o/r/pulls?page=2` succeeds on the same
    origin;
  - a 403 with `retry-after: 1` succeeds on request 2, at least 1 s later;
  - `retry-after: 120` returns `RateLimited` after one request;
  - `Config::scheduled()` returns `RateLimited` on `retry-after: 1` after one
    request;
  - a GraphQL query selecting `rateLimit { cost }` reports that cost and the
    `graphql` resource;
  - `https://api.github.com.evil.example/x`, `//evil.example/x` and an
    `http://` `api_url` each return `Config` with no request sent.
- **AC-3:** A runtime-built sentinel token is used through every GitHub path:
  success, 304, 401 twice, 5xx exhaustion, a rate limit, a refused connection,
  a failed `gh` and a malformed token.
  - Redaction: a 5xx reply's JSON `message` echoes the token. After a rotation
    (401, then a new token), a 5xx reply's `message` echoes the old token. The
    `Status` error text contains neither token.
  - The failed-`gh` path puts the sentinel inside the invalid `hosts.yml`, so
    an echo in gh's stderr would be caught.
  - Positive control: the sentinel appears in the fixture's captured
    `Authorization`.
  - It appears nowhere in a child test process's stdout or stderr, where every
    result and error is printed with `{}` and `{:?}`, the client included.
  - It appears in no panic message carrying the client and the last error.
  - It appears in no JSON line of a journal file holding every call's `Cost`
    and error.
  - The Jev key gets the same check over every Jev error and `Debug` output.
- **AC-4:** The Jev client reproduces each request/response case of
  `plugins/jev/hooks/src/__tests__/jev.test.ts` that does not concern argv
  parsing:
  - the noul body and bearer header;
  - missing and line-break keys rejected before any request;
  - choice, score and ask criteria shapes;
  - the `__proto__` key order;
  - the model pin;
  - structured and scalar file state;
  - 1 option, 1 level, 256 options and 11 levels rejected before any request;
  - malformed success;
  - a missing answer, wrong type, bad distribution or negative usage;
  - a terminal `401` with its body;
  - `408`, then `529`, then success;
  - `retry-after: 61` returned without waiting;
  - a dropped connection retried;
  - `JEV_TIMEOUT=0` → `Timeout` after 3 attempts;
  - `Reply.body` equal to the raw reply text;
  - `retry-after-ms: 1500` retried; a body over toolu-http's default 1 MiB cap
    returns `Transport(BodyTooLarge)` after one request;
  - an `ask` payload that is empty, an array or invalid JSON rejected before
    any request;
  - an `ask` choice whose `criteria` is a string sent, then `InvalidResponse`;
  - a noul with an empty side omitting it, and empty instructions rejected.
- **AC-5:** `cargo xtask gate` passes with the two new crates registered and no
  gate-data change, including the duplication step (0 clones in
  `crates/**/src`) and the coverage floors. `bun run check:http-musl` passes
  (static musl probe and CLI ≤ 4 MiB).
- **AC-6:** With `GH_TOKEN` unset, `Client::new` returns
  `Error::Token(Unavailable)` whose message names `GH_TOKEN` and
  `gh auth token`, in both cases:
  - `gh auth token` fails on an invalid `hosts.yml`;
  - no `gh` is on `PATH`.

  A `BabysitTick` that surfaces that error through `LinkError::Failed` makes
  `babysit::next` return an `Attention` containing the `Unavailable` text,
  without a panic.
- **AC-7:** `crates/cli` passes pr-babysit's `Tick` to `toolu epic` and the
  hub's `Snapshot` to `toolu statusline`.
  - `babysit::next(links::BABYSIT_TICK, …)` returns the `Attention` naming
    #433.
  - `links::STATUS_SNAPSHOT` returns `NotPorted { issue: 445 }`.
  - `toolu epic` and `toolu statusline` keep their placeholder output and the
    `commands --json` snapshot.
- **AC-8:** The one-shot policy keeps `gh.ts` against the fixture.
  `PB_GH_BACKOFF="0 0 0"` is used everywhere except where a wait is measured:
  - a 502 then a 200 succeeds with `attempts == 2`;
  - three 502s with `PB_GH_ATTEMPTS=3` return `Status { 502 }` after 3
    requests;
  - a 404 and a non-rate 403 each return `Status` after 1 request;
  - a 403 whose body says "API rate limit exceeded" is retried;
  - `x-ratelimit-remaining: 0` with a reset 3600 s ahead returns
    `RateLimited` after 1 request;
  - `PB_GH_ATTEMPTS=0`, `PB_GH_BACKOFF=x` and `PB_GH_TIMEOUT=0` each return
    `Config` naming the variable;
  - GraphQL `errors[]` returns `GraphQl` after 1 request, and a reply without
    `data` returns `Decode`;
  - a malformed `GH_TOKEN` sends no request;
  - `Retry { attempts: 0 }` returns `Config` from `Client::new`;
  - a body over the cap returns `Transport(BodyTooLarge)` after 1 request;
  - a 302 to the second origin reaches it without `Authorization`.
- **AC-9:** `toolu_http::Client::send` against the fixture:
  - returns a 404 with its headers and body as a `Response`, and a 304 with an
    empty body;
  - finds headers case-insensitively;
  - rejects `Authorization`, `Proxy-Authorization` and `Cookie` headers before
    sending;
  - formats `Auth` and `Client` with `{:?}` without the credential or a proxy
    value.

  The existing `toolu-http` tests still pass unchanged.

## Acceptance evidence

| AC | Real input and expected observation | Boundary case | Check |
|---|---|---|---|
| AC-1 | This workspace's `cargo metadata`; fixture workspaces built from `base/` | Reverse edge of the existing pr-babysit → epic-orchestrator fixture | `cargo xtask check-layers`; `cargo test -p xtask --test fixtures_tools layers` |
| AC-2 | `toolu-http-test-support` TLS origin and CONNECT proxy; real `gh` with a temporary `GH_CONFIG_DIR` | Same-token 401; scheduled policy; foreign host | `cargo test -p toolu-github --test rest --test graphql --test token` |
| AC-3 | Sentinel `toolu-sentinel-<pid>-<nanos>` in `GH_TOKEN` and `hosts.yml` | Positive control in the captured header | `cargo test -p toolu-github --test token_leak` (`harness = false`, the `run_hook.rs` precedent); `cargo test -p toolu-jev-client` |
| AC-4 | Fixture replies copied from `jev.test.ts` | `retry-after: 61`, dropped connection, zero timeout | `cargo test -p toolu-jev-client` |
| AC-5 | This branch | jscpd, per-crate coverage, musl size | `cargo xtask gate`; `bun run check:http-musl` |
| AC-6 | `hosts.yml` holding invalid YAML; a `PATH` without `gh` | gh missing versus gh failing | `cargo test -p toolu-github --test token`; `cargo test -p toolu-epic-orchestrator` |
| AC-7 | The cli unit tests and the real `toolu` binary | Unchanged placeholder output and snapshot | `cargo test -p toolu-cli` |
| AC-8 | Fixture reply sequences | Exhaustion; invalid variables | `cargo test -p toolu-github --test retry` |
| AC-9 | Fixture origin | Forbidden header names | `cargo test -p toolu-http -p toolu-http-test-support` |

`gh` must be on `PATH` for the tests of `toolu-github` and
`toolu-epic-orchestrator` (AC-6). It is installed on the GitHub-hosted Linux and
macOS runners. A missing `gh` fails those tests; it does not skip them. The
behaviour was verified with gh 2.101 (2026-09-15). The fixture `hosts.yml` uses
the legacy single-account shape, with `oauth_token`, `user` and
`git_protocol`, which gh's multi-account migration reads.

## Documentation impact

- New `crates/core/github/README.md` and `crates/core/jev/README.md`:
  - token sources, retry policies, cost and the `gh` test prerequisite;
  - the key, retries, key order and the integer-key difference.
- `crates/core/http/README.md`: `send`, `Response`, forbidden headers and the
  redacting `Debug`.
- `crates/http-test-support/README.md`: sequences and dropped connections.
- `AGENTS.md` Key files: rows for `toolu-github`, `toolu-jev-client` and the
  engine traits with `crates/cli/src/links.rs`.
- Registration data, not gate data:
  - `folders.json` (`core` gains `github`, `jev`);
  - the root `Cargo.toml` members;
  - the crate count in `crates/xtask/src/tests/layers_check_test.rs`
    (21 → 23).
- No skill, command or `docs/cli/` change: no verb changes.

## Open Questions

None blocking.
- **Owner #430:** the release `toolu` links the clients only from their first
  consumer, `toolu jev`. Here, `bun run check:http-musl` measures a probe that
  links `toolu-http` plus the current CLI.
- **Owner #434:** the engine journal. AC-3 writes the `Cost` and error records
  the journal will append to a journal file.
