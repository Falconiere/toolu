# Muse host detection and roots — Plan

**Date:** 2026-10-19   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-19-muse-host-roots-design.md   **Topic:** Falconiere/toolu#542

## Evidence and approach

The issue and epic's Muse 1.4.10 probe establish the environment and layout.
Inspected `Host::ALL`, `native_event`, `detect`, `Roots`, their colocated tests,
`fixtures/host/root.json`, and the CLI contract snapshot. Current CLI host
values derive from `Host::ALL`. Extend those abstractions with Muse, retain
the existing overrides and fallback rules, and test the shared-event ambiguity.
The installed `muse plugins --help` confirms install/remove grammar; Meta's
Muse Code extending guide confirms skill slash shortcuts.

## Workstream summary

Protocol host/event mapping → detection precedence → roots and real worktree
paths → generated CLI documentation and full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "S1",
    "title": "Add Host::Muse and PascalCase events, leaving extra events unmapped and staged payload/encoder routes fail-closed",
    "check": "cargo test -p toolu-protocol --lib",
    "ac_refs": ["AC-1", "AC-4"],
    "paths": ["crates/core/protocol/src/host.rs", "crates/core/protocol/src/native.rs", "crates/core/protocol/src/payload.rs", "crates/core/protocol/src/encode.rs", "crates/core/protocol/src/tests/host_test.rs", "crates/core/protocol/src/tests/native_test.rs", "crates/core/protocol/src/tests/encode_test.rs", "crates/core/protocol/src/tests/hook_test.rs"],
    "input": "Host::parse muse and mus; PreToolUse, Stop, Notification, SubagentStop and PostToolUseFailure native names; a real run_hook_io Muse decision with nonempty stderr on enforcing failure",
    "model": "inherit"
  },
  {
    "id": "S2",
    "title": "Detect Muse before Codex's PLUGIN_ROOT alias while preserving override warnings and ambiguous shared-event fallback",
    "check": "cargo test -p toolu-runtime --lib host::detect",
    "ac_refs": ["AC-2", "AC-4"],
    "depends_on": ["S1"],
    "paths": ["crates/core/runtime/src/host/detect.rs", "crates/core/runtime/src/host/tests/detect_test.rs", "crates/core/protocol/src/host.rs", "crates/core/protocol/src/native.rs"],
    "input": "Env pairs with MUSE_SESSION_ID or MUSE_PLUGIN_ID plus PLUGIN_ROOT; PLUGIN_ROOT alone; bogus override; PreToolUse without Muse variables",
    "model": "inherit"
  },
  {
    "id": "S3",
    "title": "Resolve Muse config, project, plugin and data roots and host-native command strings; extend host root fixture and its replay runner",
    "check": "cargo test -p toolu-runtime --lib host::roots && cargo test -p toolu-runtime --test host_roots_fixture",
    "ac_refs": ["AC-3"],
    "depends_on": ["S2"],
    "paths": ["crates/core/runtime/src/host/roots.rs", "crates/core/runtime/src/host/tests/roots_test.rs", "crates/core/runtime/tests/host_roots_fixture.rs", "fixtures/host/root.json"],
    "input": "MUSE_CONFIG_DIR, XDG_CONFIG_HOME and HOME environment cases; MUSE_PLUGIN_ROOT and MUSE_PLUGIN_DATA_DIR with Claude aliases; a real git worktree cwd; muse plugins install/remove; /toolu:setup",
    "model": "inherit"
  },
  {
    "id": "S4",
    "title": "Document the additive --host muse interface in generated CLI reference and accept commands JSON snapshot",
    "check": "TOOLU_RESOURCE_HOME=\"$TMPDIR/isolated-resources\" cargo test -p toolu-cli --test contract && cargo xtask gate --only docs-cli --only cli-compat && cargo xtask check-markdown-cli",
    "ac_refs": ["AC-1", "AC-5"],
    "depends_on": ["S3"],
    "paths": ["crates/cli/tests/contract.rs", "crates/cli/tests/fixtures/commands_json.snap", "docs/cli", "crates/core/protocol/src/host.rs", "crates/plugins/src/host.rs", "crates/core/engine/src/registry/gate.rs", "crates/core/engine/src/ledger/preflight.rs", "crates/core/engine/tests/helpers", "crates/toolu/src/doctor/inventory.rs", "crates/toolu/src/lifecycle/notices.rs"],
    "input": "Built CLI --host muse commands --json and invalid --host mus, then generated --help and commands snapshot",
    "model": "inherit"
  },
  {
    "id": "S5",
    "title": "Run the full Rust quality gate and CLI compatibility contract",
    "check": "capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c 'TOOLU_RESOURCE_HOME=\"$TMPDIR/isolated-resources\" cargo xtask gate'",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4", "AC-5"],
    "depends_on": ["S4"],
    "paths": ["crates", "fixtures/host", "docs/cli"],
    "input": "The complete branch diff and the real Rust workspace gate",
    "model": "inherit"
  }
]
```

## Critical files

- Protocol: `crates/core/protocol/src/{host,native}.rs` and their colocated
  tests.
- Runtime: `crates/core/runtime/src/host/{detect,roots}.rs`, colocated tests,
  and `fixtures/host/root.json` (including the replay runner's case count and
  uninstall-command dispatch).
- CLI documentation: `docs/cli/**`, command snapshot, and a CLI assertion
  for the accepted and rejected host values.

## Verification

Write tests before implementation and record red → green for each behavior
step. Run each test and gate through `toolu epic job --`. The root fixture
uses the repository's real git setup and environment snapshots, including
missing/empty variables. Check the actual binary's accepted/rejected host
  values. Regenerate docs and snapshot, then run `check-cli-compat` and the full
  gate. Delivery requires a scoped commit, final `toolu ledger run
  <plan> --verify`, local review, a ready push verdict, push, and a PR targeting
  `main` with the issue and epic links. Rebase on `origin/main` before
  implementation and before push if it moved; rerun affected checks after a
  rebase. The epic engine owns babysitting after `report babysit`.

## Review

Plan review (2026-10-19): fixed the fixture runner path, declared its case
count and uninstall dispatch, and recorded delivery preflight. All ACs have a
dependent check and representative boundary input. Status: Approved.

## Deviations

- S1: `Host::Muse` made the existing exhaustive payload and encoder matches
  fail to compile. #543/#544 own the full ports, so S1 includes the smallest
  compile-safe interim arms. The payload reads Claude-shaped JSON (as the
  epic probe observed), while decision encoding explicitly errors. A real
  `run_hook_io` test proves enforcing events exit 2 with blocked stderr and
  context events return `systemMessage` until #544 lands.
- S4: the CLI contract build exposed other exhaustive host matches. Muse stays
  an unsupported installer target and an unknown installed-plugin inventory
  until #549/#550; the registry keeps unknown presence, and the ledger and
  lifecycle use neutral instructions. The dispatch test harness uses Muse's
  native config root. This resolves the compiler boundary without claiming
  later epic integration is complete.
- S4 verification uses a task-owned `TOOLU_RESOURCE_HOME` for the contract
  test. Without it, the test connects to this machine's resident epic engine,
  whose newer status fields are outside this checkout's schema. The isolated
  run passed all 20 tests against this checkout's own status document.
- S5 keeps that isolation for the full gate and drops root capabilities as
  required by the repository's root-host gate procedure.
