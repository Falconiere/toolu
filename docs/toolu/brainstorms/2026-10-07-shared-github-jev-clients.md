# Shared GitHub and Jev clients, cross-plugin traits — Brainstorm

**Date:** 2026-10-07
**Issue:** Falconiere/toolu#460 (epic #402)

## Capsule

- **Outcome:** two core crates, `crates/core/github` (`toolu-github`) and
  `crates/core/jev` (`toolu-jev-client`), give pr-babysit, the epic engine and
  `toolu jev` one GitHub REST/GraphQL client and one Jev client. Two traits in
  `toolu-engine`, `BabysitTick` and `StatusSnapshot`, carry plugin logic across
  plugin crates. Their owners implement them, and `crates/cli` passes the
  owner's implementation to the consumer's `run` when it registers the
  namespace, so no plugin crate depends on another.
- **Material defaults/non-goal:** no consumer calls either client yet (#430,
  #433, #435, #447 do). The trait implementations return a typed "not ported"
  error naming #433 and #445 until those ports land. No `toolu` verb is added.
- **Repository evidence:** `tooling/conventions/guardrails/rust/layers.json`
  already places `github` and `jev` at the engine layer and lists the rule
  crates; `crates/cli/src/registry.rs` registers namespaces as plain
  `fn(&ArgMatches, &Ctx) -> Outcome`; `plugins/jev/hooks/src/jev.ts` and
  `plugins/pr-babysit/hooks/src/babysit/gh.ts` hold the behaviour to keep.
- **Risk:** the token or the Jev key leaking through a `Debug` print (today
  `toolu_http::Auth` and `toolu_http::Client` derive `Debug` over the bearer
  text and the whole environment); the request key order the Jev model sees.
- **Handoff:** spec.

## Axes

| Axis | Default | Evidence | Risk |
|---|---|---|---|
| Trait injection | Each consumer's `run` takes only the trait object it needs; `crates/cli/src/links.rs` holds the wrappers the registry names | The registry already uses plain function pointers, and placeholder runs ignore their arguments (`_matches`) | The epic and statusline placeholders ignore the injected object until #434 and #431 |
| Owner implementations | `Err(LinkError::NotPorted { issue })` | The `planned` placeholder convention (`toolu_runtime::namespace::Planned`) | None: callers already treat an error as an attention item |
| Transport | `toolu_http::Client::send`: extra request headers in; status, headers and body out for every status | ureq and rustls belong to `toolu-http` only (rule 14) | The existing helpers must keep their status-to-error behaviour |
| Token sources | `GH_TOKEN`, then `gh auth token`; re-read once on `401` | Issue text; `gh auth token` itself honours `GITHUB_TOKEN` | A missing `gh` is a typed error, never a panic |
| Retry | One-shot: `PB_GH_ATTEMPTS`, `PB_GH_BACKOFF`, `PB_GH_TIMEOUT` and `gh.ts`'s classes; scheduled: one attempt, rate limits returned with their wait | `gh.ts:50-55`; the engine checks every 3 minutes and never backs off (#447) | A long `retry-after` must not block a one-shot call: over 60 s it returns `RateLimited` |
| Cost | REST: 0 points for `304`, else 1, plus the `x-ratelimit-*` headers; GraphQL: `data.rateLimit.cost` when the query selects it, plus the headers | GitHub documents that a `304` is free; the engine design measured it on PR #454 | A GraphQL query without `rateLimit` reports no points |
| Jev key order | Questions and state held as `toolu_runtime::json::ordered::Ordered`, printed as `JSON.stringify` text | serde_json's `preserve_order` pulls `foldhash`, which cargo-deny rejects; the TypeScript client keeps option order (`__proto__` test); `Ordered` already keeps document order and collapses repeated keys as JavaScript does (Jev chose raw JSON text at 0.99; the spec review found `Ordered`, which keeps that choice's order and adds `JSON.stringify` parity) | Structured instructions and unknown question fields pass through untouched, as in TypeScript; integer-like keys keep insertion order where JavaScript moves them first |

## Alternatives rejected

- **A `Links` struct and an `Action::Linked` registry variant** (Jev: 0.01):
  every linked namespace would receive every trait object, and the registry
  gains a variant for two callers.
- **A process-global registry filled by `main`** (Jev: 0.00): hidden state,
  and a library reading it before `main` fills it fails at run time instead of
  compile time.
- **Spawning the Bun bundles from the trait implementations** (Jev: 0.00):
  brings back the Bun dependency #440 removes.
- **Carrying headers and body in `Error::HttpStatus`** (Jev: 0.01): `304` is
  not an error, and the helpers' callers would see a larger error type for
  nothing.
- **`GITHUB_TOKEN` as a separate source** (Jev: 0.03): `gh auth token` already
  reads it, and the issue names two sources.
- **Typed Jev questions only** (Jev: 0.01): `ask` accepts structured
  instructions and criteria (`plugins/jev/skills/jev/SKILL.md`).
- **Raw JSON text (`serde_json` `raw_value`)**: keeps order, but sends a
  repeated key twice and the reader's whitespace and number text, where
  `jev.ts` sends `JSON.stringify` text; `Ordered` matches that.
- **`toolu-state` as a GitHub dependency:** the caller keeps the ETag, so the
  client has no state to store; cargo-machete rejects an unused dependency.

## Open risks

- The release `toolu` binary does not link the clients until their first
  consumer (#430), so the 4 MiB size budget of #417 is measured again there;
  this issue measures the current binary only.
