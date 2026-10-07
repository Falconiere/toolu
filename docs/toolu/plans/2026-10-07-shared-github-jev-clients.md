# Shared GitHub and Jev clients, cross-plugin traits — Plan

**Date:** 2026-10-07   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-07-shared-github-jev-clients-design.md   **Topic:** #460. Core GitHub and Jev clients over a `send` API in `toolu-http`; `BabysitTick` and `StatusSnapshot` in `toolu-engine`, implemented by their owners and passed by `crates/cli`; layer fixtures.

## Evidence and approach

The spec decides the design. The brainstorm records the Jev calls:
- per-consumer trait parameter: 0.99;
- `NotPorted` owner implementations: 1.00;
- the `send` API: 0.99;
- `GH_TOKEN` then `gh auth token`: 0.97;
- the epic `next` step: 0.51.

The spec review replaced raw JSON with `toolu_runtime::json::ordered::Ordered`.

**Sources to port:**
- `plugins/jev/hooks/src/jev.ts` (`delay`, `retryable`, `post`, the key checks)
  and `jev/response.ts` (`project`);
- `plugins/jev/hooks/src/jev/parse.ts`: the choice/score bounds and
  `structured()`, for the builders and `State::structured`;
- `plugins/pr-babysit/hooks/src/babysit/gh.ts` (`ghRun` defaults and
  `ghClassify`).

**Cases:** `plugins/jev/hooks/src/__tests__/jev.test.ts`, minus the argv-only
cases.

**Reused:**
- `toolu_runtime::{env::Env, process::{run, Spec}, json::ordered::Ordered}`;
- `toolu_http` (extended);
- the `crates/http-test-support` fixture (extended);
- the `harness = false` self-exec precedent `crates/core/protocol/tests/run_hook.rs`;
- `fixtures/guardrails/rust/layers/violating` as the template for the new
  layer fixtures, registered in `crates/xtask/tests/fixtures_tools.rs` as
  `fixture::check("layers", "<case>")`.

**Constraints:**
- `cargo` must be the rustup proxy, so every check prefixes
  `PATH="$HOME/.cargo/bin:$PATH"`; `/usr/bin/cargo` is a system cargo.
- Rule 14: no `std::process::Command`, `std::env` or stdio in `src` outside
  their owners. Tests may use them, because the capability scan reads `src`
  only.
- 300 code lines per file, 50 per function, a wired test file per module with
  a function, and no `pub` item unused outside its crate and tests.
- `docs/toolu` is gitignored; design docs are force-added (`git add -f`), as
  in #413–#417.
- Comemory `be52369e`: `bun run test` has host-only failures on this root box
  that also fail on `origin/main`, so CI is the authority for those.

**Probes (2026-10-07):**
- gh 2.101 prints a `hosts.yml` `oauth_token` with `GH_CONFIG_DIR`, and exits 1
  on invalid YAML before any keyring lookup.
- ureq 3 returns the 3xx response with `max_redirects(0)`, and adds its
  `User-Agent` only when the request has none.
- The `x86_64-unknown-linux-musl` std and `musl-gcc` are installed.

## Workstream summary

1. Transport split and `send`, then fixture sequences.
2. Engine traits, then owners and cli wiring.
3. Crate scaffolds, layer fixtures and registration.
4. GitHub token, then calls and retries, then the leak test.
5. Jev client.
6. The epic attention item over the real token error.
7. Docs, then the full gate and size check.

## Steps (machine-readable)

```json
[
  {
    "id": "http-send",
    "title": "toolu-http: split lib.rs into auth.rs (Auth, header value, redacting Debug), error.rs (Error, ureq/io mapping), send.rs (Method, Request, Response::header, forbidden authorization/proxy-authorization/cookie names, Client::send returning every status); helpers rebuilt on send with unchanged behaviour; Client gets a Debug without env values",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-http \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t --lib auth::tests:: && t --lib send::tests:: && t --lib error::tests:: && t --test transport && t --test send && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy",
    "ac_refs": [
      "AC-9"
    ],
    "paths": [
      "crates/core/http/",
      "crates/http-test-support/",
      "Cargo.lock"
    ],
    "input": "fixture routes: 404 with x-test header and body, 304 empty, 200 JSON; headers [(\"Authorization\",\"x\")], [(\"COOKIE\",\"a=b\")], [(\"Proxy-Authorization\",\"y\")]; Auth::Bearer(\"secret-bearer\"), Auth::Basic{u,\"secret-pass\"}; Env with HTTPS_PROXY=http://user:proxypass@127.0.0.1:1",
    "model": "inherit"
  },
  {
    "id": "fixture-sequences",
    "title": "toolu-http-test-support: reply.rs (Reply with dropped()), routes.rs (per-path VecDeque: served in order, the last repeats; route = one-reply sequence), Fixture::sequence; the server closes a dropped reply's connection after reading the request without recording it",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-http-test-support \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t --lib reply::tests:: && t --lib routes::tests:: && t --lib server::tests:: && t --lib tests:: && o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-http 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy",
    "ac_refs": [
      "AC-9",
      "AC-4"
    ],
    "depends_on": [
      "http-send"
    ],
    "paths": [
      "crates/http-test-support/",
      "crates/core/http/"
    ],
    "input": "sequence [500, 200] then a third request (200 repeats); route replaced by sequence; [dropped, 200] seen by a toolu_http client: first a Transport error, then 200, requests() length 1",
    "model": "inherit"
  },
  {
    "id": "engine-traits",
    "title": "toolu-engine: LinkError (NotPorted{issue}, Failed) with Display; babysit.rs (TickRequest, TickDecision, TickReport, BabysitTick: Send + Sync); status.rs (StatusSnapshot: Send + Sync over Roots and a dir); serde_json and toolu-runtime deps",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-engine \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t --lib tests:: && t --lib babysit::tests:: && t --lib status::tests:: && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy",
    "ac_refs": [
      "AC-7"
    ],
    "paths": [
      "crates/core/engine/",
      "Cargo.lock"
    ],
    "input": "a test implementation of each trait held as &dyn and Box<dyn> across a thread; LinkError::NotPorted{issue: 433} and Failed(\"boom\") displayed",
    "model": "inherit"
  },
  {
    "id": "owners-and-links",
    "title": "pr-babysit Tick and hub status::Snapshot return NotPorted (433, 445); epic-orchestrator run takes &dyn BabysitTick and gains babysit.rs (Next, next); statusline run takes &dyn StatusSnapshot; crates/cli/src/links.rs (BABYSIT_TICK, STATUS_SNAPSHOT, epic, statusline wrappers) named by the registry; pr-babysit, hub, epic-orchestrator, statusline and cli depend on toolu-engine",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p toolu-pr-babysit tick_is_not_ported && t -p toolu-hub snapshot_is_not_ported && t -p toolu-epic-orchestrator --lib babysit::tests:: && t -p toolu-epic-orchestrator && t -p toolu-statusline && t -p toolu-cli --bin toolu links::tests:: && t -p toolu-cli && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask check-layers && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy --only unused-pub --only jscpd",
    "ac_refs": [
      "AC-7",
      "AC-1"
    ],
    "depends_on": [
      "engine-traits"
    ],
    "paths": [
      "Cargo.lock",
      "Cargo.toml",
      "crates/",
      "crates/cli/",
      "crates/core/engine/",
      "crates/epic-orchestrator/",
      "crates/pr-babysit/",
      "crates/statusline/",
      "crates/toolu/",
      "docs/cli/",
      "tooling/conventions/guardrails/rust/"
    ],
    "input": "TickRequest{repo: \"Falconiere/toolu\", number: 460}; test ticks answering KeepGoing, Success, Escalate and Failed; the real toolu binary running `toolu epic` and `toolu statusline`; the commands --json snapshot",
    "model": "inherit"
  },
  {
    "id": "scaffold-and-layers",
    "title": "Create crates/core/github (toolu-github) and crates/core/jev (toolu-jev-client) as members with LAYER and smoke tests; folders.json core += github, jev; layers_check_test count 21 -> 23; fixtures layers/clean-trait-wiring and layers/violating-epic-babysit with fixtures_tools.rs cases",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t -p xtask --test fixtures_tools layers_ && t -p xtask --test fixture_coverage && t -p xtask --bin xtask layers_check && t -p toolu-github --test smoke && t -p toolu-jev-client --test smoke && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask check-layers && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask guardrails && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy",
    "ac_refs": [
      "AC-1"
    ],
    "depends_on": [
      "owners-and-links"
    ],
    "paths": [
      "Cargo.lock",
      "Cargo.toml",
      "crates/",
      "crates/core/github/",
      "crates/core/jev/",
      "crates/xtask/",
      "fixtures/guardrails/rust/layers/",
      "tooling/conventions/guardrails/rust/",
      "tooling/conventions/guardrails/rust/folders.json"
    ],
    "input": "fixture workspaces overlaid on fixtures/guardrails/rust/base: engine core crate + pr-babysit implementing its trait + epic-orchestrator consuming it + cli linking both (passes); epic-orchestrator depending on pr-babysit (fails with the plugin-to-plugin message)",
    "model": "inherit"
  },
  {
    "id": "github-token",
    "title": "toolu-github: error.rs, token.rs (Token with redacting Debug, Source, TokenError, resolve from GH_TOKEN then gh auth token via toolu_runtime::process with a 10 s deadline, malformed check, redact over every token read); Config with API_URL, one_shot/scheduled; Client::new validates api_url and Retry",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-github \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t --lib token::tests:: && t --lib error::tests:: && t --lib tests:: && t --test token && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy",
    "ac_refs": [
      "AC-6",
      "AC-2",
      "AC-8"
    ],
    "depends_on": [
      "scaffold-and-layers",
      "fixture-sequences"
    ],
    "paths": [
      "Cargo.lock",
      "Cargo.toml",
      "crates/",
      "crates/core/github/",
      "tooling/conventions/guardrails/rust/"
    ],
    "input": "real gh with GH_CONFIG_DIR holding hosts.yml oauth_token; hosts.yml holding invalid YAML; PATH without gh; GH_TOKEN=\"a b\" and \"a\\nb\"; api_url http://example.com and https://api.github.com/; Retry{attempts: 0}",
    "model": "inherit"
  },
  {
    "id": "github-calls",
    "title": "toolu-github: policy.rs (PB_GH_ATTEMPTS/BACKOFF/TIMEOUT parsing, classification, wait from retry-after or x-ratelimit-reset, max_wait), cost.rs, call.rs (attempt loop, 401 re-read once, headers, path resolution against api_url), rest.rs (get, rest, Rest, Fresh::json), graphql.rs (data, errors[], rateLimit.cost)",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-github \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t --lib policy::tests:: && t --lib cost::tests:: && t --lib call::tests:: && t --lib rest::tests:: && t --lib graphql::tests:: && t --test rest && t --test graphql && t --test retry && t --test token && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy --only unused-pub --only jscpd",
    "ac_refs": [
      "AC-2",
      "AC-8"
    ],
    "depends_on": [
      "github-token"
    ],
    "paths": [
      "Cargo.lock",
      "Cargo.toml",
      "crates/",
      "crates/core/github/",
      "crates/http-test-support/",
      "tooling/conventions/guardrails/rust/"
    ],
    "input": "fixture sequences: 200+ETag then 304; [401, 200] after a hosts.yml rewrite; 401 with GH_TOKEN; 403 retry-after 1 then 200; 403 retry-after 120; 403 x-ratelimit-remaining 0 reset now+3600; [502, 200] and three 502s; 404; 403 plain and 403 'API rate limit exceeded'; GraphQL 200 with rateLimit.cost 3 and x-ratelimit-resource graphql; GraphQL errors[]; a 304 with no ETag sent; paths https://api.github.com.evil.example/x and //evil.example/x; api_url http://api.example.test; a 9 MiB body against an 8 MiB cap; a 302 to the second origin; GraphQL 200 without data; Retry{attempts:0}; PB_GH_BACKOFF=\"0 0 0\" except the measured waits; a same-origin pagination link <api_url>/repos/o/r/pulls?page=2 that succeeds",
    "model": "inherit"
  },
  {
    "id": "github-token-leak",
    "title": "toolu-github tests/token_leak.rs (harness = false): the parent runs itself as a child with a runtime sentinel; the child drives success, 304, 401 twice, 5xx exhaustion, rate limit, refused connection, failed gh and malformed token, prints every result/error with {} and {:?}, appends each Cost and error as JSON lines to a journal file, then panics with the client and last error; the parent asserts the positive control and the sentinel's absence from stdout, stderr and the journal. main and its helpers are clippy-clean non-test code: output through write_all on stdout/stderr, ExitCode, ? propagation, and the panic via assert!(cond, msg) as in crates/core/protocol/tests/run_hook.rs; no println!, panic!, unwrap, expect or indexing",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-github --test token_leak 2>&1) && printf '%s' \"$o\" | grep -q 'token_leak: ok' && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy --only unused-pub --only jscpd",
    "ac_refs": [
      "AC-3"
    ],
    "depends_on": [
      "github-calls"
    ],
    "paths": [
      "Cargo.lock",
      "Cargo.toml",
      "crates/",
      "crates/core/github/",
      "tooling/conventions/guardrails/rust/"
    ],
    "input": "sentinel toolu-sentinel-<pid>-<nanos> in GH_TOKEN and in hosts.yml; fixture replies 200, 304, 401, 401, 502x3, 403 retry-after 120; a refused loopback port; invalid hosts.yml; GH_TOKEN with an inner space; a 502 whose JSON message echoes the token; after rotation a 401 whose message echoes the old token; the sentinel inside the invalid hosts.yml",
    "model": "inherit"
  },
  {
    "id": "jev-client",
    "title": "toolu-jev-client: error.rs, question.rs (State text/structured, Question noul/choice/score with bounds, Questions single/parse over Ordered), retry.rs (delay, retryable, attempt loop, JEV_TIMEOUT incl. 0 and try_from_secs_f64), reply.rs (response.ts validation, Number.isInteger usage), lib.rs (Config, Jev::from_env/ask, redacting Debug, max_redirects 0)",
    "check": "t() { o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-jev-client \"$@\" 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed'; }; t --lib question::tests:: && t --lib retry::tests:: && t --lib reply::tests:: && t --lib error::tests:: && t --lib tests:: && t --test cases && t --test retries && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy --only unused-pub --only jscpd",
    "ac_refs": [
      "AC-4",
      "AC-3"
    ],
    "depends_on": [
      "scaffold-and-layers",
      "fixture-sequences"
    ],
    "paths": [
      "Cargo.lock",
      "Cargo.toml",
      "crates/",
      "crates/core/jev/",
      "crates/http-test-support/",
      "tooling/conventions/guardrails/rust/"
    ],
    "input": "the jev.test.ts replies: noul 0.92 envelope, choice billing/tech, score 1.5 with legend, ask urgent, __proto__ choice, malformed bodies, invalid distributions, negative usage, 401 {\"error\":\"bad key\"}, [408, 529, 200], 429 retry-after 61, [dropped, 200], JEV_TIMEOUT=0; keys unset and \"bad\\nkey\"; 256 options, 11 levels, 1 option, 1 level; retry-after-ms 1500; an oversized body; ask payloads {}, [] and 'not json'; an ask choice with criteria \"x\"; noul with an empty side; empty instructions; usage 1e30 and 3.0",
    "model": "inherit"
  },
  {
    "id": "epic-attention",
    "title": "epic-orchestrator: a test BabysitTick that builds a real toolu_github::Client with GH_TOKEN unset and a failing gh, surfaces the error as LinkError::Failed, and babysit::next returns Attention naming GH_TOKEN and gh auth token (toolu-github as a dev-dependency)",
    "check": "o=$(PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -q -p toolu-epic-orchestrator 2>&1) && printf '%s' \"$o\" | grep -Eq 'test result: ok\\. [1-9][0-9]* passed' && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask check-layers && PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --only fmt --only clippy",
    "ac_refs": [
      "AC-6"
    ],
    "depends_on": [
      "github-token",
      "owners-and-links"
    ],
    "paths": [
      "Cargo.lock",
      "Cargo.toml",
      "crates/",
      "crates/core/github/",
      "crates/epic-orchestrator/",
      "tooling/conventions/guardrails/rust/"
    ],
    "input": "GH_CONFIG_DIR with an invalid hosts.yml and PATH holding the real gh; PATH without gh",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "READMEs for crates/core/github and crates/core/jev; crates/core/http/README.md (send, Response, forbidden headers, redacting Debug); crates/http-test-support/README.md (sequence, dropped); AGENTS.md Key files rows for toolu-github, toolu-jev-client and the engine traits with crates/cli/src/links.rs",
    "check": "set -e; for f in crates/core/github/README.md crates/core/jev/README.md; do test -s \"$f\"; done; grep -q 'gh auth token' crates/core/github/README.md; grep -q 'PB_GH_ATTEMPTS' crates/core/github/README.md; grep -q 'TYPESAFE_API_KEY' crates/core/jev/README.md; grep -q 'JEV_TIMEOUT' crates/core/jev/README.md; grep -q 'send' crates/core/http/README.md; grep -q 'sequence' crates/http-test-support/README.md; grep -q 'toolu-github' AGENTS.md; grep -q 'toolu-jev-client' AGENTS.md; grep -q 'links.rs' AGENTS.md; PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask check-markdown-cli",
    "ac_refs": [
      "AC-5"
    ],
    "depends_on": [
      "github-token-leak",
      "jev-client",
      "epic-attention"
    ],
    "paths": [
      "crates/core/github/README.md",
      "crates/core/jev/README.md",
      "crates/core/http/README.md",
      "crates/http-test-support/README.md",
      "AGENTS.md"
    ],
    "input": "the shipped crate surfaces; docs/cli/commands.json for the Markdown–CLI drift check",
    "model": "inherit"
  },
  {
    "id": "full-gate",
    "title": "Full Rust gate, musl size check, docs suite and the workspace tests",
    "check": "set -e; export PATH=\"$HOME/.cargo/bin:$PATH\"; cargo xtask gate --base origin/main --title 'feat(core): shared GitHub and Jev clients and cross-plugin traits (#460)'; bun run check:http-musl; bun run test:docs",
    "ac_refs": [
      "AC-5",
      "AC-1"
    ],
    "depends_on": [
      "docs"
    ],
    "paths": [
      "AGENTS.md",
      "Cargo.lock",
      "Cargo.toml",
      "crates/",
      "docs/cli/",
      "fixtures/guardrails/",
      "tooling/conventions/guardrails/rust/"
    ],
    "input": "this branch against origin/main",
    "model": "inherit"
  }
]
```

## Critical files

- `crates/core/http/src/{lib,auth,send,error}.rs` and
  `crates/core/http/src/tests/{lib,auth,send,error}_test.rs`;
  `crates/core/http/tests/send.rs`; `crates/core/http/README.md`.
- `crates/http-test-support/src/{lib,server,reply,routes}.rs` and
  `crates/http-test-support/src/tests/{reply,routes}_test.rs`;
  `crates/http-test-support/README.md`.
- `crates/core/engine/{Cargo.toml,src/lib.rs,src/babysit.rs,src/status.rs,src/tests/*}`.
- `crates/core/github/`: `Cargo.toml`, `README.md`,
  `src/{lib,token,policy,cost,call,rest,graphql,error}.rs`, `src/tests/*`,
  `tests/{smoke,token,rest,graphql,retry,token_leak}.rs`, `tests/helpers/*`.
- `crates/core/jev/`: `Cargo.toml`, `README.md`,
  `src/{lib,question,retry,reply,error}.rs`, `src/tests/*`,
  `tests/{smoke,cases,retries}.rs`, `tests/helpers/*`.
- The owner and consumer crates:
  - `crates/pr-babysit/{Cargo.toml,src/lib.rs,src/tests/lib_test.rs}`;
  - `crates/toolu/{Cargo.toml,src/status.rs,src/tests/status_test.rs}`;
  - `crates/epic-orchestrator/{Cargo.toml,src/lib.rs,src/babysit.rs,src/tests/*}`;
  - `crates/statusline/{Cargo.toml,src/lib.rs,src/tests/lib_test.rs}`.
- `crates/cli/{Cargo.toml,src/links.rs,src/registry.rs,src/main.rs,src/tests/links_test.rs}`.
- Workspace registration:
  - `Cargo.toml`, `Cargo.lock`;
  - `tooling/conventions/guardrails/rust/folders.json`;
  - `crates/xtask/src/tests/layers_check_test.rs`,
    `crates/xtask/tests/fixtures_tools.rs`;
  - `fixtures/guardrails/rust/layers/{clean-trait-wiring,violating-epic-babysit}/**`.
- `AGENTS.md`.

## Verification

End to end, the real loopback HTTPS fixture and CONNECT proxy, the real `gh`,
and the real `toolu` binary prove AC-2 to AC-4 and AC-6 to AC-9. Both pass
and fail are checked:
- every retry class;
- every token source failure;
- a foreign host;
- forbidden headers;
- the jev.test.ts failure cases.

AC-1 is proved by `check-layers` on this repository and on the two fixture
workspaces. AC-5 is proved by `cargo xtask gate` (fmt, clippy, guardrails,
layers, reach, unused-pub, jscpd, coverage floors, deny, machete, docs-cli,
cli-compat, markdown-cli) with no gate-data change, plus
`bun run check:http-musl`.

Documentation is synchronized in `docs` and checked there. Delivery follows
the execution reference:
1. A scoped `feat(core): …` commit.
2. `plan-ledger run <plan> --verify`.
3. `toolu-review:review` with version-2 state over every changed file.
4. `verdict.js status` reporting `overall: ready`.
5. Push, the PR (`Closes Falconiere/toolu#460`, `Part of Falconiere/toolu#402`)
   and `pr-babysit:babysit`.

## Deviations

None yet. Accepted in advance: a `bun run test:docs` failure in `full-gate` that also fails on a clean `origin/main` worktree on this host (comemory `be52369e`); CI is then the authority.
