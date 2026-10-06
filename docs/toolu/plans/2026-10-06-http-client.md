# Small HTTPS client — Plan

**Date:** 2026-10-06   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-06-http-client-design.md   **Topic:** Falconiere/toolu#417

## Evidence and approach

The approved spec and `docs/toolu/brainstorms/2026-10-06-http-client.md`
settle the transport. `crates/core/http` is a placeholder. `toolu-runtime`
already supplies the `Env` snapshot, and `check-layers` owns the TLS capability
in `toolu-http`. `docs/resource-budgets.md` fixes the 4 MiB size limit. Ureq 3
documents explicit `ring` provider configuration, redirect authorization policy,
timeouts, bounded reads, and a supplied proxy.

## Workstream summary

Implement the bounded client, then the reusable TLS/proxy fixture and real
transport tests, document the API, and verify static musl linkage, size, and
the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "client",
    "title": "Implement the public JSON and byte client, typed errors, explicit proxy selection, ring TLS, bounded reads, timeout and redirect policy",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-http --lib",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "paths": ["crates/core/http/", "crates/core/runtime/src/env.rs", "Cargo.toml", "Cargo.lock"],
    "input": "Env snapshots with HTTPS_PROXY, HTTP_PROXY, ALL_PROXY and lowercase variants; malformed proxy, zero timeout/body limit, redirect auth policy; unit tests exercise configuration and error mapping",
    "model": "inherit"
  },
  {
    "id": "fixture",
    "title": "Create a workspace test-support crate with a real loopback rustls server, generated CA and CONNECT proxy, exposed through a toolu-http dev-dependency",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-http-test-support",
    "ac_refs": ["AC-1", "AC-2", "AC-4", "AC-6"],
    "depends_on": ["client"],
    "paths": ["crates/http-test-support/", "crates/core/http/", "Cargo.toml", "Cargo.lock", "tooling/conventions/guardrails/rust/folders.json"],
    "input": "Two ephemeral loopback TLS origins with a generated certificate, a CONNECT proxy tunnelling the requested origin, captured HTTP requests, normal and delayed responses, and controlled shutdown",
    "model": "inherit"
  },
  {
    "id": "transport",
    "title": "Prove JSON GET/POST/PUT, both auth schemes, proxy and CA trust, timeout, status, body cap, malformed JSON, and cross-origin download redirect against the fixture",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo test -p toolu-http --test transport",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "depends_on": ["fixture"],
    "paths": ["crates/core/http/", "crates/http-test-support/", "Cargo.toml", "Cargo.lock"],
    "input": "Real HTTPS requests through CONNECT to api.example.test on an ephemeral port; refused connection, no test CA, delayed body, 404 with large body, 302 to same hostname on a distinct port without Authorization",
    "model": "inherit"
  },
  {
    "id": "docs",
    "title": "Document the public transport and fixture contracts, proxy precedence, redirect policy and typed errors",
    "check": "test -s crates/core/http/README.md && test -s crates/http-test-support/README.md && PATH=\"$HOME/.cargo/bin:$PATH\" cargo doc -p toolu-http -p toolu-http-test-support --no-deps",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "depends_on": ["transport"],
    "paths": ["crates/core/http/", "crates/http-test-support/", "Cargo.toml", "Cargo.lock"],
    "input": "The implemented public API and fixture setup; error and proxy examples from the passing transport tests",
    "model": "inherit"
  },
  {
    "id": "musl-size",
    "title": "Build/run the TLS transport test on static musl, build a release-profile client probe and the CLI, measure both against 4 MiB, and inspect the dependency tree",
    "check": "bun run check:http-musl",
    "ac_refs": ["AC-5"],
    "depends_on": ["transport"],
    "paths": ["crates/core/http/", "crates/http-test-support/", "crates/cli/", "Cargo.toml", "Cargo.lock", "docs/resource-budgets.md", "tooling/src/check-http-musl.ts"],
    "input": "Static musl release test makes a real HTTPS request to the fixture; stripped CLI and linked client probe sizes are measured in bytes; dependency tree is inspected",
    "model": "inherit"
  },
  {
    "id": "gate",
    "title": "Run full Rust quality gate and affected TypeScript workspace checks",
    "check": "PATH=\"$HOME/.cargo/bin:$PATH\" cargo xtask gate --base origin/main && bun run test",
    "ac_refs": ["AC-6"],
    "depends_on": ["docs", "musl-size"],
    "input": "Whole branch with real loopback and proxy tests, 85% crate coverage, static build, no capability or layout exemptions",
    "model": "inherit"
  }
]
```

## Critical files

- `crates/core/http/Cargo.toml`, `src/*.rs`, `src/tests/*_test.rs`,
  `tests/transport.rs`, `README.md`, `tooling/src/check-http-musl.ts`.
- `crates/http-test-support/Cargo.toml`, `src/*.rs`, `src/tests/*_test.rs`,
  `README.md`.
- `Cargo.toml`, `Cargo.lock`, `tooling/conventions/guardrails/rust/folders.json`.

## Verification

The transport suite uses real TLS sockets and a CONNECT proxy. It observes the
server's captured methods, headers and bodies, a no-CA TLS failure, an actual
deadline, and an auth-free redirected request. Musl runs the same suite and
`file` confirms static linkage. Release sizes and `cargo tree` cover the
resource and dependency constraints. `cargo xtask gate` covers format, clippy,
coverage, layers, folders, unused public items, and documentation. `bun run
test` covers workspace drift from the new member.

## Plan review

The `pl_check_ac_refs` helper is unavailable in this shell, so a direct
JSON/Markdown check confirmed exact AC-1 through AC-6 coverage and nonempty
step checks. The initial musl/size step omitted its linked probe; it now runs
`bun run check:http-musl`, which must build and measure that probe as well
as the CLI. Jev judged the revised ledger's criterion coverage strong (0.97).
**Status: Approved.**

## Deviations

The host's `/usr/bin/cargo` is Rust 1.93.1 while `rust-toolchain.toml` requires
1.99.0. All ledger Cargo checks now put the installed rustup proxy first on
`PATH`; `check-http-musl.ts` does the same. This changes the runner environment,
not the approved behavior or acceptance criteria.

The TypeScript final-removal gate rejects tracked shell files, so the musl
check moved from a Bash helper to a Bun tooling script with the same real
transport test, static linkage, size, and dependency checks.

The required rustls stack adds ISC, BSD-3-Clause, and CDLA-Permissive-2.0
dependencies. `AGENTS.md` requires gate-data changes in a separate
`chore(gates)` PR. PR #487 updated `deny.toml` and merged as `b63d3bf1`; this
issue branch was rebased onto it without editing gate data.

This root-run host also has DAC and ptrace capabilities that defeat existing
permission-denial tests. Dropping `cap_dac_override`, `cap_dac_read_search`, and
`cap_sys_ptrace` makes all affected tests pass. Two earlier full Bun runs each
had one unrelated process or latency test fail under shared-machine load; the
same tests passed in isolation. Final verification runs the full Bun gate with
those capabilities dropped.

## Delivery

Commit and push the scoped branch after affected tests; run final ledger
`run docs/toolu/plans/2026-10-06-http-client.md --verify`, pre-push review,
and verdict `overall: ready`. Fetch and rebase if main moved, then rerun
affected checks. Push a conventional branch commit, open a PR against `main`
with `Closes Falconiere/toolu#417` and `Part of Falconiere/toolu#402`, report
`pr-open`, report `babysit`, and invoke `pr-babysit:babysit`.
