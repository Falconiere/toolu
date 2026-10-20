# toolu plugins targets Muse (#549) — Plan

**Date:** 2026-10-20   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-20-muse-installer-design.md   **Topic:** Muse adapter, local-dir marketplace checkout and opt-in approval in `crates/plugins`

## Evidence and approach

- `crates/plugins` hides each host behind `host::adapter::Adapter` and `verbs::Driver`;
  the verbs are host-free (`verbs/{install,list,remove,update}.rs`). Tests drive stub
  host executables from `src/tests/support_test.rs`; `crates/cli/tests/plugins_hosts.rs`
  runs the real binary the same way.
- Real Muse 1.4.11 outputs were captured on 2026-10-20 in an isolated `XDG_*` home
  (list, available, marketplace list, inspect in three trust states, approve, remove).
  They become `fixtures/plugins/muse.json`, which stub `muse` executables replay.
- Chosen design: the spec's — `Target::Muse` probed last, `Adapter::Muse` + `host::muse`
  parsers, `Driver::prepare` (checkout + marketplace) before install/update, a trust
  pass after them (`verbs::trust`), `--approve`, foreign sources as skew.
- Process access stays in `host::run::run` (`toolu_runtime::process`); `git` runs
  through it too. The git origin is a `Context` field so tests use `file://`.

## Workstream summary

captured fixture + parsers → host selection and replay → checkout/marketplace prepare,
trust pass and `--approve` → binary-level tests → docs and generated reference → live
Muse run → full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "Capture Muse 1.4.11 outputs into fixtures/plugins/muse.json; add Adapter::Muse argv, host::muse parsers (installed, available, inspect, marketplaces) and Installed.source",
    "check": "cargo test -p toolu-plugins --lib host::",
    "ac_refs": ["AC-2", "AC-3"],
    "paths": ["fixtures/plugins/muse.json", "crates/plugins/src/host/adapter.rs", "crates/plugins/src/host/muse.rs", "crates/plugins/src/host/claude.rs", "crates/plugins/src/host/codex.rs", "crates/plugins/src/host/tests"],
    "input": "fixtures/plugins/muse.json cases list, available, marketplace-list, inspect-review-needed, inspect-trusted, inspect-modified; mutated {\"plugins\":{}} and an entry without record",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "Target::Muse probed last, target_of(Host::Muse), --scope rejected for Muse, replay host muse, dispatch maps Muse to its driver",
    "check": "cargo test -p toolu-plugins --lib -- host replay dispatch args",
    "ac_refs": ["AC-1", "AC-6", "AC-8"],
    "depends_on": ["S1"],
    "paths": ["crates/plugins/src/host.rs", "crates/plugins/src/replay.rs", "crates/plugins/src/dispatch.rs", "crates/plugins/src/args.rs", "crates/plugins/src/lib.rs", "crates/plugins/src/tests"],
    "input": "sandbox PATH with stub muse alone and with claude+codex+muse; --scope user --host muse; a .toolu/plugins.json with host muse",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "Driver::prepare (clone or move the checkout, register or refresh marketplace toolu), the trust pass with --approve, foreign-source skew in install and update",
    "check": "cargo test -p toolu-plugins --lib",
    "ac_refs": ["AC-4", "AC-5", "AC-7"],
    "depends_on": ["S2"],
    "paths": ["crates/plugins/src/verbs.rs", "crates/plugins/src/verbs", "crates/plugins/src/args.rs", "crates/plugins/src/dispatch.rs", "crates/plugins/src/host", "crates/plugins/src/tests", "fixtures/plugins/muse.json"],
    "input": "real git with a file:// origin repo tagged v<version> carrying .agents/plugins/marketplace.json; stub muse replaying the captured marketplace-list (own and foreign source), list, available and inspect cases",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "Binary-level Muse cases: sole muse on PATH, three hosts with --host muse and without a terminal, list from captured output, malformed list exit 1, --scope usage error, install with a pre-seeded checkout",
    "check": "cargo test -p toolu-cli --test plugins_muse --test plugins_hosts --test plugins_args",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-5", "AC-6"],
    "depends_on": ["S3"],
    "paths": ["crates/cli/tests/plugins_muse.rs", "crates/cli/tests/plugins_hosts.rs", "crates/cli/tests/plugins_args.rs", "crates/plugins/src", "fixtures/plugins/muse.json"],
    "input": "the built toolu binary with stub muse/claude/codex serving fixtures/plugins/muse.json and a checkout cloned from a local repo whose origin URL is the GitHub one and whose tag v<version> is present",
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "Document Muse in docs/cli/installer.md, crates/plugins/README.md, AGENTS.md and fixtures/README.md; regenerate docs/cli and accept the commands snapshot",
    "check": "cargo xtask gate --only docs-cli --only cli-compat && cargo test -p toolu-cli --test contract && cargo xtask check-markdown-cli",
    "ac_refs": ["AC-10"],
    "depends_on": ["S4"],
    "paths": ["docs/cli", "crates/plugins/README.md", "AGENTS.md", "fixtures/README.md", "crates/cli/tests/contract.rs", "crates/cli/tests/fixtures", "crates/plugins/src/args.rs"],
    "input": "the built binary's --help for toolu plugins install/update with --approve",
    "model": "inherit"
  },
  {
    "id": "S6",
    "title": "Live run against real Muse 1.4.11 in an isolated home: install toolu --approve, inspect trusted_enabled, re-run without re-approval, remove; record the transcript",
    "check": "test -s docs/toolu/evidence/2026-10-20-muse-installer.md && grep -q trusted_enabled docs/toolu/evidence/2026-10-20-muse-installer.md",
    "ac_refs": ["AC-9"],
    "depends_on": ["S5"],
    "paths": ["docs/toolu/evidence/2026-10-20-muse-installer.md"],
    "input": "real muse 1.4.11 with XDG_* in a scratch home; the checkout pre-seeded from this branch with a local tag v8.0.4 and the GitHub origin URL",
    "model": "inherit"
  },
  {
    "id": "S7",
    "title": "Full Rust quality gate",
    "check": "mkdir -p \"$TMPDIR/empty-local-bin\" && env -u HERDR_SOCKET_PATH -u HERDR_ENV -u HERDR_PANE_ID -u HERDR_TAB_ID -u HERDR_WORKSPACE_ID -u HERDR_BIN_PATH -u HERDR_CONFIG_PATH unshare -m -- sh -c 'mount --make-rprivate / && mount --bind \"$TMPDIR/empty-local-bin\" /usr/local/bin && capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c \"RUST_TEST_THREADS=1 TOOLU_RESOURCE_HOME=$TMPDIR/isolated-resources cargo xtask gate\"'",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5", "AC-6", "AC-7", "AC-8", "AC-10"],
    "depends_on": ["S6"],
    "paths": ["crates", "fixtures", "docs", "AGENTS.md"],
    "input": "the branch",
    "model": "inherit"
  }
]
```

## Critical files

- Create: `crates/plugins/src/host/muse.rs`, `crates/plugins/src/host/tests/muse_test.rs`,
  `crates/plugins/src/verbs/prepare.rs`, `crates/plugins/src/verbs/trust.rs` (+ tests),
  `crates/cli/tests/plugins_muse.rs`, `fixtures/plugins/muse.json`,
  `docs/toolu/evidence/2026-10-20-muse-installer.md`.
- Modify: `crates/plugins/src/{host.rs,host/adapter.rs,host/claude.rs,host/codex.rs,args.rs,dispatch.rs,lib.rs,replay.rs,verbs.rs,verbs/install.rs,verbs/update.rs}`,
  `crates/plugins/src/tests/support_test.rs`, `docs/cli/installer.md`, `docs/cli/*` (generated),
  `crates/cli/tests/fixtures/commands_json.snap`, `crates/plugins/README.md`, `AGENTS.md`, `fixtures/README.md`.

## Verification

End to end: the live transcript (S6) shows `toolu plugins install toolu --host muse --approve`
leaving every capability `trusted_enabled`, a second run exiting 0 with no approve call, and
`remove` emptying `muse plugins list`. Boundaries: malformed listings (exit 1), foreign
marketplace and non-toolu checkout (exit 1 before any install), `--scope` with Muse (64),
inspect failure (failed step). Docs: installer guide, generated reference, crate README,
AGENTS.md row, fixtures README; `check-markdown-cli` and `docs-cli` gate them.

Delivery: scoped `feat(plugins):` commit, `toolu ledger run <plan> --verify` with
`TMPDIR` on a durable filesystem (`verify-root-host-gates`), `toolu-review:review`,
`toolu ledger verdict status` ready, push, PR with `Closes Falconiere/toolu#549`
and `Part of Falconiere/toolu#539`, then `report babysit` (the epic engine babysits).

## Plan review (2026-10-20)

- S5: 🟡 should-fix: the check chained `||` and `;`, so a failing docs-cli could pass. Fixed: one `&&` chain.
- S7: 🟡 should-fix: a bare `cargo xtask gate` on this root host masks `/usr/local/bin/toolu` poorly and runs with full capabilities. Fixed: the `verify-root-host-gates` isolation.
- Verification: 🟡 should-fix: no delivery line. Fixed.
- Coverage: every spec AC (AC-1 … AC-10) is in at least one `ac_refs`.

**Status:** Approved
