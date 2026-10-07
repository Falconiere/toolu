# Epic engine core

**Date:** 2026-10-07   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-07-epic-engine-core-design.md   **Topic:** Resident engine, journal and control socket for #434

## Evidence and approach

The approved spec is the contract. `toolu_engine::resources::jobs::run_managed_job` and `store::resource_home` stay the resource owner. The epic crate today only has the planned namespace plus `babysit::next`. Detach is added to `toolu_runtime::process` because plugin crates cannot spawn. Jev on the spec mapping stayed near 0.5 after a tightening pass; the manual pair of each issue acceptance item to an AC is the fallback and is what the steps below check.

## Workstream summary

Runtime detach, then the pure state machine and journal, then socket and scripts, then CLI docs and the idle measurement.

## Steps (machine-readable)

```json
[
  {
    "id": "detach",
    "title": "Detach a process group without waiting for it",
    "check": "PATH=\"/root/.nvm/versions/node/v24.21.0/bin:/root/.cargo/bin:/root/.bun/bin:$PATH\" cargo test -p toolu-runtime --lib detach_leaves",
    "ac_refs": ["AC-8"],
    "paths": ["crates/core/runtime/src/process.rs", "crates/core/runtime/src/tests/process_test.rs"],
    "input": "A real sleep child started with toolu_runtime::process::detach"
  },
  {
    "id": "machine",
    "title": "State machine, journal, pause, faults and scripted two-issue run",
    "check": "PATH=\"/root/.nvm/versions/node/v24.21.0/bin:/root/.cargo/bin:/root/.bun/bin:$PATH\" bash -c 'cargo test -p toolu-epic-orchestrator --lib scripted_epic && cargo test -p toolu-epic-orchestrator --lib merge_fault && cargo test -p toolu-epic-orchestrator --lib pause_survives'",
    "ac_refs": ["AC-1", "AC-2", "AC-3", "AC-4"],
    "depends_on": ["detach"],
    "paths": ["crates/epic-orchestrator/src/logic.rs", "crates/epic-orchestrator/src/tests/logic_test.rs", "crates/epic-orchestrator/src/journal.rs", "crates/epic-orchestrator/src/effects.rs", "crates/epic-orchestrator/src/server.rs"],
    "input": "Temp git repo, graph of issues a and b, and executable merge/cleanup/launch scripts"
  },
  {
    "id": "adopt-watch",
    "title": "Adopt TypeScript snapshots, checkpoints, lock and herdr backoff",
    "check": "PATH=\"/root/.nvm/versions/node/v24.21.0/bin:/root/.cargo/bin:/root/.bun/bin:$PATH\" bash -c 'cargo test -p toolu-epic-orchestrator --lib adopt_typescript && cargo test -p toolu-epic-orchestrator --lib watcher_engine'",
    "ac_refs": ["AC-5", "AC-7"],
    "depends_on": ["machine"],
    "paths": ["crates/epic-orchestrator/src/snapshot.rs", "crates/epic-orchestrator/src/checkpoint.rs", "crates/epic-orchestrator/src/herdr.rs", "crates/epic-orchestrator/src/tests/snapshot_test.rs", "crates/epic-orchestrator/src/tests/watcher_test.rs"],
    "input": "TypeScript-shaped issue JSON plus a dirty git worktree and herdr 0.9.3"
  },
  {
    "id": "socket",
    "title": "Spool across SIGKILL, protocol replace, and wait limit",
    "check": "PATH=\"/root/.nvm/versions/node/v24.21.0/bin:/root/.cargo/bin:/root/.bun/bin:$PATH\" bash -c 'cargo test -p toolu-epic-orchestrator --lib spool_survives && cargo test -p toolu-epic-orchestrator --lib protocol_replace && cargo test -p toolu-epic-orchestrator --lib wait_limit'",
    "ac_refs": ["AC-8", "AC-9", "AC-10"],
    "depends_on": ["machine"],
    "paths": ["crates/epic-orchestrator/src/client.rs", "crates/epic-orchestrator/src/server.rs", "crates/epic-orchestrator/src/tests/socket_test.rs"],
    "input": "A spawned engine process, SIGKILL, and a protocol-2 client"
  },
  {
    "id": "job",
    "title": "epic job uses the existing managed-job runner",
    "check": "PATH=\"/root/.nvm/versions/node/v24.21.0/bin:/root/.cargo/bin:/root/.bun/bin:$PATH\" cargo test -p toolu-epic-orchestrator --lib epic_job",
    "ac_refs": ["AC-11", "AC-12"],
    "depends_on": ["machine"],
    "paths": ["crates/epic-orchestrator/src/job.rs", "crates/epic-orchestrator/src/tests/job_test.rs"],
    "input": "A git worktree bound with bind_worktree and the command true"
  },
  {
    "id": "docs",
    "title": "CLI reference, schema, README, watcher skill and SessionStart hook",
    "check": "PATH=\"/root/.nvm/versions/node/v24.21.0/bin:/root/.cargo/bin:/root/.bun/bin:$PATH\" bash -c 'cargo xtask docs-cli --check && cargo test -p toolu-cli --test contract'",
    "ac_refs": ["AC-10"],
    "depends_on": ["socket", "job"],
    "paths": ["docs/cli/epic.md", "docs/cli/commands.json", "crates/cli/src/commands.schema.json", "plugins/epic-orchestrator/README.md", ".toolu/skills/verify-epic-watcher/SKILL.md", "plugins/epic-orchestrator/hooks/hooks.json"],
    "input": "toolu epic --help and toolu commands --json"
  },
  {
    "id": "idle",
    "title": "Record idle RSS for three epics and the 30 second tick",
    "check": "PATH=\"/root/.nvm/versions/node/v24.21.0/bin:/root/.cargo/bin:/root/.bun/bin:$PATH\" cargo test -p toolu-epic-orchestrator --lib idle_tick",
    "ac_refs": ["AC-6"],
    "depends_on": ["socket"],
    "paths": ["docs/toolu/evidence/epic-engine-idle.json", "crates/epic-orchestrator/src/lib.rs", "crates/epic-orchestrator/src/tests/idle_test.rs"],
    "input": "Release toolu epic engine with three registered epic directories"
  }
]
```

## Critical files

- `crates/core/runtime/src/process.rs`
- `crates/epic-orchestrator/src/` (engine modules and `lib.rs`)
- `crates/epic-orchestrator/Cargo.toml`
- `crates/cli/tests/contract.rs`, `crates/cli/src/tests/links_test.rs`, `crates/cli/tests/helpers/schema.rs`
- `crates/cli/src/commands.schema.json`
- `docs/cli/epic.md`, `docs/cli/commands.json`
- `plugins/epic-orchestrator/README.md`
- `plugins/epic-orchestrator/hooks/src/engine-ensure.ts` and `hooks/hooks.json`
- `.toolu/skills/verify-epic-watcher/SKILL.md`
- `docs/toolu/evidence/epic-engine-idle.json`

## Verification

`cargo test -p toolu-epic-orchestrator` and `cargo test -p toolu-cli --test contract` against a temp resource root, a real git worktree, and herdr 0.9.3. Then `cargo xtask gate` through the epic job lease. Docs regenerate from the binary. Failure paths covered: busy lock, unknown reconcile, pause, SIGKILL, protocol mismatch, missing binding.

## Deviations

- The herdr probe stays in `server.rs`. `herdr.rs` was not added. `watcher_engine` is wired from `server.rs`.
- `logic.rs`, `socket.rs`, and `verbs.rs` are split into `commit.rs`, `dispatch.rs`, `control.rs`, and `query.rs` so each file stays inside the length gate. The plan checks are unchanged.
- A `SIGKILL` hold sleeps after the request is read and before it is dispatched, so the spool file still exists for the next start.
