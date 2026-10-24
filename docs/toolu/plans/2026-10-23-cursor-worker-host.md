# Cursor as an epic worker host — Plan

**Date:** 2026-10-23   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-23-cursor-worker-host-design.md   **Topic:** installer, doctor, hook gate, smoke, live proof

## Evidence and approach
Muse is the analog: `host/checkout.rs`, `verbs/prepare.rs`, `doctor/muse.rs`, `host_smoke_muse.rs`, `hooks_muse.rs`. Cursor differs: no install command, so a filesystem installer into `~/.cursor/plugins/local`. Live facts: `cursor-agent plugin` has only `marketplace`; local plugins run toolu hooks (`.env` write denied).

## Workstream summary
epic revert → hook-event gate → doctor → installer → smoke wiring → docs → live proof.

## Steps (machine-readable)
```json
[
 {
  "id": "s1",
  "title": "Cursor accepted again by route and launch",
  "ac_refs": [
   "AC-1"
  ],
  "paths": [
   "crates/epic-orchestrator/src",
   "crates/cli/tests/epic_launch.rs"
  ],
  "input": "hosts spec claude,cursor:2; launch kind cursor",
  "check": "cargo test -p toolu-epic-orchestrator hosts"
 },
 {
  "id": "s2",
  "title": "check-hooks requires a Cursor event for every Claude hook event",
  "ac_refs": [
   "AC-3"
  ],
  "paths": [
   "crates/xtask/src"
  ],
  "input": "plugins/*/hooks/hooks.json and a hooks.json with PermissionRequest",
  "check": "cargo test -p xtask cursor"
 },
 {
  "id": "s3",
  "title": "doctor reads Cursor's local and cached plugins",
  "ac_refs": [
   "AC-2"
  ],
  "depends_on": [
   "s1"
  ],
  "paths": [
   "crates/toolu/src/doctor"
  ],
  "input": "real cursor cache/toolu/ast-grep tree and a local plugin copy",
  "check": "cargo test -p toolu-hub cursor"
 },
 {
  "id": "s4",
  "title": "toolu plugins install|list|remove|update --host cursor",
  "ac_refs": [
   "AC-6"
  ],
  "paths": [
   "crates/plugins/src",
   "crates/cli/tests"
  ],
  "input": "a local git origin with a tag carrying plugins/",
  "check": "cargo test -p toolu-plugins cursor"
 },
 {
  "id": "s5",
  "title": "host-smoke --host cursor wiring",
  "ac_refs": [
   "AC-4"
  ],
  "depends_on": [
   "s4"
  ],
  "paths": [
   "crates/xtask/src"
  ],
  "input": "xtask host-smoke args, no cursor-agent on PATH",
  "check": "cargo test -p xtask host_smoke"
 },
 {
  "id": "s6",
  "title": "Docs: installer, epic-orchestrator skill and README, CLI reference",
  "ac_refs": [
   "AC-1",
   "AC-6"
  ],
  "depends_on": [
   "s4"
  ],
  "paths": [
   "docs",
   "plugins/epic-orchestrator"
  ],
  "input": "generated docs/cli",
  "check": "cargo xtask check-markdown-cli"
 },
 {
  "id": "s7",
  "title": "Live: install into a temp Cursor config, doctor passes, update and remove round-trip (log kept in /private/tmp/589-cursor-installer-live.log)",
  "ac_refs": [
   "AC-5"
  ],
  "depends_on": [
   "s3",
   "s4"
  ],
  "paths": [
   "crates/plugins/src"
  ],
  "input": "cursor-agent 2026.10+, TOOLU_BIN debug binary",
  "check": "cargo build -p toolu-cli && sh -c 'C=$(mktemp -d /private/tmp/589cfg.XXXX); B=$PWD/target/debug/toolu; export TOOLU_CONFIG_DIR=$C; { $B plugins install delivery-flow --host cursor && $B --host cursor doctor && $B plugins update --host cursor && $B plugins remove delivery-flow brainstorm pr-babysit toolu-review toolu --yes --host cursor; } > /private/tmp/589-cursor-installer-live.log 2>&1; s=$?; cat /private/tmp/589-cursor-installer-live.log; rm -rf $C; exit $s'"
 },
 {
  "id": "s8",
  "title": "Live: cargo xtask host-smoke --host cursor passes (log kept in /private/tmp/589-host-smoke-cursor.log)",
  "ac_refs": [
   "AC-5"
  ],
  "depends_on": [
   "s5",
   "s7"
  ],
  "paths": [
   "crates/xtask/src"
  ],
  "input": "live cursor-agent session",
  "check": "cargo build -p toolu-cli && sh -c 'cargo xtask host-smoke --host cursor --bin target/debug/toolu > /private/tmp/589-host-smoke-cursor.log 2>&1; s=$?; cat /private/tmp/589-host-smoke-cursor.log; exit $s'"
 }
]
```

## Critical files
`crates/plugins/src/{host.rs,host/cursor.rs,host/checkout.rs,verbs/cursor.rs,dispatch.rs,args.rs}`, `crates/toolu/src/doctor/{cursor.rs,inventory.rs}`, `crates/xtask/src/{check_hooks.rs,hooks_cursor.rs,host_smoke*.rs}`, `tooling/conventions/guardrails/rust/folders.json`, docs.

## Verification
Unit and binary tests per step; `cargo xtask gate`; steps s7/s8 are live and their real command output goes on the PR.

## Deviations
- s7/s8 checks run the real live commands (installer round trip and host-smoke) instead of `cursor-agent --version`; their output is kept under /private/tmp and pasted in the PR.
- Hook gate: events checked through `toolu_protocol::native`; Cursor tool names/cwd gaps found live were fixed in `toolu-engine` (`Session::cwd`).
