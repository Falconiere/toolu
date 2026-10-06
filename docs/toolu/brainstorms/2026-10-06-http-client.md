# toolu-http transport — Brainstorm

**Date:** 2026-10-06
**Issue:** Falconiere/toolu#417

## Outcome

`toolu-http` provides bounded, timed JSON GET/POST/PUT and byte downloads over
rustls, including explicit proxy and authentication handling. A reusable Rust
dev-dependency fixture proves HTTPS, CONNECT proxying, and redirects in process.

## Decisions and evidence

- Use `ureq` 3 with the explicit rustls `ring` provider. The issue requires a
  static musl build and the measured binary budget is 4 MiB
  (`docs/resource-budgets.md`). `ring` avoids the larger native build toolchain
  of `aws-lc-rs`; the actual binary and TLS request will be measured.
- Select `HTTPS_PROXY`, `HTTP_PROXY`, or `ALL_PROXY` (including lowercase
  variants) from `toolu-runtime::env::Env`. That crate owns process environment
  access, and an explicit snapshot makes proxy tests deterministic.
- Set ureq redirect authentication to `Never`: redirects are followed for
  downloads and no redirected request carries `Authorization`. This strictly
  satisfies the cross-origin requirement, including scheme and port changes.
- Place the fixture in a separate test-support crate used as a dev-dependency.
  Its TLS server uses a test-only `toolu-http` feature that re-exports rustls,
  so only `toolu-http` directly links the TLS capability under `check-layers`.

## Alternatives rejected

- Implicit ureq environment discovery would bypass the explicit environment
  snapshot used throughout the Rust core.
- `RedirectAuthHeaders::SameHost` is weaker than an origin boundary when a
  redirect changes scheme or port. Exact-origin manual redirects would add
  code without a stated same-origin authentication requirement.
- An excluded support crate with direct rustls would escape the workspace gate;
  inline integration-test helpers would not serve later Rust crates.

## Risks and checks

- Verify proxy selection and TLS trust with a real loopback HTTPS and CONNECT
  proxy. Test redirect authentication with two distinct origins.
- Verify the stripped release binary is at most 4 MiB, static musl builds can
  make a TLS request, and `cargo tree -p toolu-http` has no Tokio or Hyper.
- Run `cargo xtask gate` with no gate-data exemptions.
