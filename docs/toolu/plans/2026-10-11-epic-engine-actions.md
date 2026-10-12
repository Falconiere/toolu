# Epic engine actions: graph, route, launch, finish, close, trackers

**Date:** 2026-10-11   **Status:** Approved   **Spec:** docs/toolu/specs/2026-10-11-epic-engine-actions-design.md   **Topic:** Issue #435 — port epic-orchestrator's actions to Rust, run them in the engine, delete `scripts/` and `hooks/src`

## Evidence and approach

The approved spec is the contract. Inspected: every script and test under
`plugins/epic-orchestrator/scripts/` and `hooks/src/`; the engine in
`crates/epic-orchestrator/src` (`model`, `logic`, `commit`, `server`,
`socket`, `dispatch`, `schedule`, `recover`, `limit`, `watch`,
`github_engine`, `herdr`, `herdr_argv`, `checkpoint`, `effects`, `verbs`,
`control`, `snapshot`, `disk`); `toolu_engine::resources` (leases, store,
binding, jobs; migration and fences still missing); `toolu-github`
(`Config::scheduled`), `toolu-http` (`test_root_ca_der`), `toolu-jev-client`
(`Config.endpoint`); `toolu-http-test-support`'s HTTPS fixture; herdr 0.9.3's
bundled API schema (protocol 22) for method names and params; the CLI fast
path (`toolu epic-orchestrator hook …` is accepted) and `hook/plugins.rs`;
`toolu_runtime::startup::{native_toolu_advice, codex_dependency_notice}`;
the statusline port (#510) for the deletion ripple (tooling, OpenCode
surfaces, payloads). Recalled: the CPU audit (cached-route overbooking,
null-host fallback) and the fence-before-replacement memory.

The spec's `probe` module is named `probe_evidence` here so its tests do not
collide with `github_probe` under a test-name filter; the verb and the CLI test
(`epic_probe`) keep their names. The PR body states this.

Jev decided the forks recorded in the spec (gate 0.80, herdr socket fixture
0.98, seam 0.95, command swap 0.85, chain-first 0.94, batch-only holds 0.74,
CLI retry policy 0.95). No delegation: one crate owns the state machine, and
the steps depend on each other in order. Each step adds its module and its
co-located tests together, keeping every file under 300 code lines.

## Workstream summary

Shared pieces first (refs, retry, trackers, graph, hosts, resource
transitions, route), then the side-effecting actions (workload, checkpoint,
herdr calls, launch, finish, gate, close, coupling), then the CLI verbs, then
the engine (effect thread, refill, refresh, recovery, merge bridge), the
native hook, the scripted scenarios, the deletion ripple, the Markdown and
docs, and the full gate.

## Steps (machine-readable)

```json
[
  {
    "id": "refs-retry",
    "title": "Port refs (parse_ref, issue_key, slugify, key_for, branch_for, epics_home) and retry (classify, policy, with_retry with write safety and a per-effect sleep budget, reset_from_headers, error mapping)",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib refs::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 8 && cargo test -p toolu-epic-orchestrator --lib refs::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib retry:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 18 && cargo test -p toolu-epic-orchestrator --lib retry:: && cargo xtask guardrails",
    "ac_refs": [
      "AC-1",
      "AC-2",
      "AC-34"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/refs.rs",
      "crates/epic-orchestrator/src/retry.rs",
      "crates/epic-orchestrator/src/retry",
      "crates/epic-orchestrator/src/tests/refs_test.rs",
      "crates/epic-orchestrator/src/tests/retry_test.rs"
    ],
    "input": "TypeScript common and ratelimit cases: reference forms incl. invalid, herdr-name keys incl. long and digit-led repos, slug, host homes, rate-limit texts, Retry-After date and seconds, Linear epoch-ms and GitHub epoch-s resets, a 429 then success, 5xx on a read (retried) and on a write (not), a reset beyond the cap, three 429s exhausting a 3 s budget after two requests"
  },
  {
    "id": "trackers",
    "title": "Port the GitHub, Jira and Linear trackers and tracker detection over toolu-github (Config::scheduled) and toolu-http",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib tracker:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 30 && cargo test -p toolu-epic-orchestrator --lib tracker:: && cargo xtask guardrails",
    "ac_refs": [
      "AC-1",
      "AC-20",
      "AC-30"
    ],
    "depends_on": [
      "refs-retry"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/tracker.rs",
      "crates/epic-orchestrator/src/tracker"
    ],
    "input": "HTTPS fixture replies from the TypeScript tracker tests: sub-issues and task-list fallback, blocked_by plus blocked-by text, closing PRs, Jira v2/v3 paged search, transitions by name, ADF comments, missing base or credentials and bad API version refused before HTTP, Linear issue and project payloads, auth header forms, sentinel credentials absent from errors"
  },
  {
    "id": "graph",
    "title": "Port levels, downstream, chain, classify, pick_batch, cycle, checkouts, table and graph.json build; move the epic fixtures",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib graph:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 16 && cargo test -p toolu-epic-orchestrator --lib graph:: && cargo xtask guardrails",
    "ac_refs": [
      "AC-2",
      "AC-19",
      "AC-29"
    ],
    "depends_on": [
      "trackers"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/graph.rs",
      "crates/epic-orchestrator/src/graph",
      "fixtures/epic/epic248-graph.json",
      "fixtures/epic/epic248-body.md",
      "fixtures/epic/epic203-body.md",
      "fixtures/epic/pr290-rollup.json"
    ],
    "input": "fixtures/epic/epic248-graph.json, every occupied and terminal stage, an external blocker, an injected cycle, a chain-vs-unblocks conflict, real git repositories with GitHub and non-GitHub origins"
  },
  {
    "id": "hosts",
    "title": "Port hosts (args, resume, safe mode, aliases, skill syntax, OpenCode checks, limit pattern) with fixtures/epic/hosts.json",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib hosts::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 5 && cargo test -p toolu-epic-orchestrator --lib hosts::tests:: && bun run tooling/src/check-fixture-inventory.ts && cargo xtask guardrails",
    "ac_refs": [
      "AC-4"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/hosts.rs",
      "crates/epic-orchestrator/src/tests/hosts_test.rs",
      "fixtures/epic/hosts.json",
      "fixtures/index.json",
      "fixtures/README.md",
      "tooling/src/check-fixture-inventory.ts"
    ],
    "input": "fixtures/epic/hosts.json captured from the TypeScript agentArgs (24 parity combinations, resume, safe), aliases, HOST_LIMIT positives and negatives, DEFAULT_TABLE; shell-unsafe args refused"
  },
  {
    "id": "resources",
    "title": "Add agent migration, worktree fence, active_jobs and lease worktree/pane/session patches to toolu_engine::resources",
    "check": "test \"$(cargo test -p toolu-engine --lib resources::transitions -- --list 2>/dev/null | grep -c ': test$')\" -ge 5 && cargo test -p toolu-engine --lib resources && env -u npm_config_store_dir TMPDIR=/private/tmp bun test --timeout 60000 packages/toolu-core/src/resources && cargo xtask guardrails",
    "ac_refs": [
      "AC-6",
      "AC-8",
      "AC-9"
    ],
    "paths": [
      "crates/core/engine/src/resources.rs",
      "crates/core/engine/src/resources/leases.rs",
      "crates/core/engine/src/resources/transitions.rs",
      "crates/core/engine/src/resources/jobs.rs",
      "crates/core/engine/src/resources/tests",
      "packages/toolu-core/src/resources"
    ],
    "input": "The migration and fence cases of packages/toolu-core/src/resources/__tests__/resources.test.ts on real resource roots (reserve, migrate, cancel, fence with a live managed-job process group, release refused with active jobs), and a Rust-written lease read back by the TypeScript reader"
  },
  {
    "id": "route",
    "title": "Port routing: table, tiers, in-process Jev scores, heuristic, pool, cooldowns, capacity, persisted routes, render",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib route:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 16 && cargo test -p toolu-epic-orchestrator --lib route:: && cargo xtask guardrails",
    "ac_refs": [
      "AC-3",
      "AC-17"
    ],
    "depends_on": [
      "graph",
      "hosts",
      "resources"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/route.rs",
      "crates/epic-orchestrator/src/route",
      "fixtures/epic/epic248-graph.json",
      "fixtures/epic/hosts.json"
    ],
    "input": "The #248 graph in temp state roots; records in every stage incl. an unknown host; shared leases with a reserved migration; hosts.json and resource cooldowns (expired and live); invalid pool specs; a Jev HTTPS fixture answering score questions and a Jev failure"
  },
  {
    "id": "workload-checkpoint",
    "title": "Port the worktree workload inventory and the full checkpoint snapshot",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib workload:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 5 && cargo test -p toolu-epic-orchestrator --lib workload:: && test \"$(cargo test -p toolu-epic-orchestrator --lib checkpoint::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 10 && cargo test -p toolu-epic-orchestrator --lib checkpoint::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib server::watcher_tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 1 && cargo test -p toolu-epic-orchestrator --lib server::watcher_tests:: && cargo xtask guardrails",
    "ac_refs": [
      "AC-8",
      "AC-10"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/workload.rs",
      "crates/epic-orchestrator/src/workload",
      "crates/epic-orchestrator/src/checkpoint.rs",
      "crates/epic-orchestrator/src/tests/checkpoint_test.rs",
      "crates/epic-orchestrator/src/tests/watcher_test.rs",
      "crates/epic-orchestrator/src/server.rs"
    ],
    "input": "Real sleep processes started inside a temp worktree that change cwd and spawn descendants, the caller and a shell pid excluded; real git repositories for every snapshot case with GIT_TRACE2_EVENT (clean, dirty, unpushed, unchanged, new HEAD, corrupt, locked and missing index, concurrent)"
  },
  {
    "id": "herdr-calls",
    "title": "Add the herdr calls launch and finish need, their socket and CLI forms, a committed schema excerpt, and the herdr socket test fixture",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib herdr_calls::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 6 && cargo test -p toolu-epic-orchestrator --lib herdr_calls::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib herdr::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 3 && cargo test -p toolu-epic-orchestrator --lib herdr::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib herdr_argv::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 2 && cargo test -p toolu-epic-orchestrator --lib herdr_argv::tests:: && cargo xtask guardrails",
    "ac_refs": [
      "AC-5",
      "AC-15"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/herdr.rs",
      "crates/epic-orchestrator/src/herdr_argv.rs",
      "crates/epic-orchestrator/src/herdr_calls.rs",
      "crates/epic-orchestrator/src/tests/herdr_calls_test.rs",
      "crates/epic-orchestrator/src/tests/herdr_test.rs",
      "crates/epic-orchestrator/src/tests/herdr_argv_test.rs",
      "crates/epic-orchestrator/src/tests/herdr_fixture_test.rs",
      "crates/epic-orchestrator/src/tests/herdr_state_test.rs",
      "crates/epic-orchestrator/src/tests/herdr_agents_test.rs",
      "fixtures/epic/herdr-calls.json"
    ],
    "input": "fixtures/epic/herdr-calls.json (method names and param keys from the herdr 0.9.3 protocol-22 schema), a fixture serving those methods over a real Unix socket with real git worktrees and real agent processes, herdr error objects, non-JSON output and timeouts"
  },
  {
    "id": "launch",
    "title": "Port launch (record validation, reservation and replacement, checkout, worktree, agent, sessions, lifecycle, brief, prompt, OpenCode preflight, dry run) and swap the worker brief template to toolu epic commands",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib launch:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 40 && cargo test -p toolu-epic-orchestrator --lib launch:: && cargo xtask guardrails",
    "ac_refs": [
      "AC-5",
      "AC-6",
      "AC-7",
      "AC-8",
      "AC-22"
    ],
    "depends_on": [
      "route",
      "workload-checkpoint",
      "herdr-calls"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/launch.rs",
      "crates/epic-orchestrator/src/launch",
      "plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md",
      "fixtures/epic/epic248-graph.json",
      "fixtures/epic/hosts.json",
      "crates/epic-orchestrator/src/tests/herdr_fixture_test.rs",
      "crates/epic-orchestrator/src/tests/herdr_state_test.rs",
      "crates/epic-orchestrator/src/tests/herdr_agents_test.rs"
    ],
    "input": "The #248 graph, temp state and resource roots, the herdr fixture; refusals: null route, malformed route/pool/record, incomplete stages, host change without replace, capacity, a real missing git or gh, OpenCode without a 1.x opencode, a listed worktree without a path, replacement with old workload alive; temp Claude/Codex/Cursor session homes and an opencode session list"
  },
  {
    "id": "finish",
    "title": "Port finish: strict record, legacy fence adoption, removal intent, live/removed/crash paths, branch delete, lease release",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib finish:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 14 && cargo test -p toolu-epic-orchestrator --lib finish:: && cargo xtask guardrails",
    "ac_refs": [
      "AC-8",
      "AC-9"
    ],
    "depends_on": [
      "launch"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/finish.rs",
      "crates/epic-orchestrator/src/finish"
    ],
    "input": "A bare origin, a main clone, a linked worktree and an issue record per TypeScript case (abandon with dirty work, clean pushed, no PR, missing/array/malformed record, every stage, legacy binding, branch retry, crash after prepared removal, absent worktree, incomplete terminal), plus a live process group in the worktree"
  },
  {
    "id": "gate-close-coupling",
    "title": "Port the merge gate, epic close, the coupling check, jira issue get and the host-evidence probe",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib gate:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 18 && cargo test -p toolu-epic-orchestrator --lib gate:: && test \"$(cargo test -p toolu-epic-orchestrator --lib close::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 4 && cargo test -p toolu-epic-orchestrator --lib close::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib coupling::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 10 && cargo test -p toolu-epic-orchestrator --lib coupling::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib jira_cli::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 3 && cargo test -p toolu-epic-orchestrator --lib jira_cli::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib probe_evidence::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 3 && cargo test -p toolu-epic-orchestrator --lib probe_evidence::tests:: && cargo xtask guardrails",
    "ac_refs": [
      "AC-11",
      "AC-12",
      "AC-13",
      "AC-14",
      "AC-18"
    ],
    "depends_on": [
      "trackers",
      "graph"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/gate.rs",
      "crates/epic-orchestrator/src/gate",
      "crates/epic-orchestrator/src/close.rs",
      "crates/epic-orchestrator/src/coupling.rs",
      "crates/epic-orchestrator/src/jira_cli.rs",
      "crates/epic-orchestrator/src/probe_evidence.rs",
      "crates/epic-orchestrator/src/tests",
      "fixtures/epic",
      "docs/toolu/evidence/epic-hosts-v1.json"
    ],
    "input": "fixtures/epic/pr290-rollup.json and the epic248/epic203 bodies; GitHub fixture PR states (open, behind, conflicting, failing, pending, draft, merged, closed), protection and non-protection refusals, a moved head; epic-close graph incomplete and complete; issues naming 0042_add_users, unrelated, and/or, URL pairs; Jev fixture P=0.93 and a Jev failure; Jira fixture issue and bad arguments; committed host evidence and a copy without opencode"
  },
  {
    "id": "verbs",
    "title": "Expose graph, route, launch, finish, close, release, gate, jira and probe as toolu epic verbs; regenerate docs/cli and the commands snapshot",
    "check": "cargo test -p toolu-cli --test epic_graph --test epic_launch --test epic_finish --test epic_probe --test epic_report && cargo test -p toolu-cli --test contract && test \"$(cargo test -p toolu-epic-orchestrator --lib verbs::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 4 && cargo test -p toolu-epic-orchestrator --lib verbs::tests:: && cargo xtask gate --only docs-cli --only cli-compat && cargo xtask guardrails",
    "ac_refs": [
      "AC-2",
      "AC-5",
      "AC-6",
      "AC-9",
      "AC-14",
      "AC-18",
      "AC-37"
    ],
    "depends_on": [
      "launch",
      "finish",
      "gate-close-coupling"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/verbs.rs",
      "crates/epic-orchestrator/src/verbs_actions.rs",
      "crates/epic-orchestrator/src/verbs_actions",
      "crates/epic-orchestrator/src/tests/verbs_test.rs",
      "crates/epic-orchestrator/src/tests/lib_test.rs",
      "crates/cli/Cargo.toml",
      "crates/cli/tests",
      "crates/cli/src/tests/links_test.rs",
      "crates/core/runtime/src/tests/namespace_test.rs",
      "docs/cli",
      "fixtures/epic",
      "docs/toolu/evidence/epic-hosts-v1.json"
    ],
    "input": "The built toolu binary against temp state dirs, the #248 graph, real git checkouts and the committed host evidence; usage errors exit 64; launch dry-run JSON (fresh and resume, every host); finish refusals; report history, note, key preservation and corrupt status"
  },
  {
    "id": "engine-effects",
    "title": "Engine executor and effect thread: parked queue, pause/drain/stop, replace wait, closed channel, socket loop split",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib effect_thread::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 6 && cargo test -p toolu-epic-orchestrator --lib effect_thread::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib executor::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 4 && cargo test -p toolu-epic-orchestrator --lib executor::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib control:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 13 && cargo test -p toolu-epic-orchestrator --lib control:: && test \"$(cargo test -p toolu-epic-orchestrator --lib client::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 3 && cargo test -p toolu-epic-orchestrator --lib client::tests:: && cargo xtask guardrails",
    "ac_refs": [
      "AC-25",
      "AC-32",
      "AC-34",
      "AC-35",
      "AC-36"
    ],
    "depends_on": [
      "verbs"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/executor.rs",
      "crates/epic-orchestrator/src/effect_thread.rs",
      "crates/epic-orchestrator/src/effects_run.rs",
      "crates/epic-orchestrator/src/socket.rs",
      "crates/epic-orchestrator/src/socket_loop.rs",
      "crates/epic-orchestrator/src/server.rs",
      "crates/epic-orchestrator/src/control.rs",
      "crates/epic-orchestrator/src/client.rs",
      "crates/epic-orchestrator/src/model.rs",
      "crates/epic-orchestrator/src/logic.rs",
      "crates/epic-orchestrator/src/tests",
      "crates/epic-orchestrator/src/socket_deadline.rs",
      "crates/epic-orchestrator/src/socket_herdr.rs",
      "crates/epic-orchestrator/src/dispatch.rs"
    ],
    "input": "Executor::Thread with a barrier effect; a stalled issue whose STATUS? parks; replace with an effect outlasting a 200 ms client wait; a closed effect channel; a protocol-1 socket stand-in"
  },
  {
    "id": "engine-refill",
    "title": "Engine per-epic state, refill and batches, coupling holds and release, stage sets, relaunch counter, launch-wait streaks, budget floors and cooldown env",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib refill::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 10 && cargo test -p toolu-epic-orchestrator --lib refill::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib holds::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 6 && cargo test -p toolu-epic-orchestrator --lib holds::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib epics::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 3 && cargo test -p toolu-epic-orchestrator --lib epics::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib limit:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 5 && cargo test -p toolu-epic-orchestrator --lib limit:: && test \"$(cargo test -p toolu-epic-orchestrator --lib schedule:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 11 && cargo test -p toolu-epic-orchestrator --lib schedule:: && cargo xtask guardrails",
    "ac_refs": [
      "AC-16",
      "AC-17",
      "AC-18",
      "AC-19",
      "AC-28",
      "AC-29",
      "AC-33"
    ],
    "depends_on": [
      "engine-effects"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/epics.rs",
      "crates/epic-orchestrator/src/refill.rs",
      "crates/epic-orchestrator/src/holds.rs",
      "crates/epic-orchestrator/src/dispatch_ops.rs",
      "crates/epic-orchestrator/src/dispatch.rs",
      "crates/epic-orchestrator/src/logic.rs",
      "crates/epic-orchestrator/src/schedule.rs",
      "crates/epic-orchestrator/src/limit.rs",
      "crates/epic-orchestrator/src/github_budget.rs",
      "crates/epic-orchestrator/src/snapshot.rs",
      "crates/epic-orchestrator/src/model.rs",
      "crates/epic-orchestrator/src/commit.rs",
      "crates/epic-orchestrator/src/tests",
      "crates/epic-orchestrator/src/effects_launch.rs",
      "crates/epic-orchestrator/src/effects_run.rs",
      "crates/epic-orchestrator/src/route.rs",
      "crates/epic-orchestrator/src/route/score.rs",
      "crates/epic-orchestrator/src/recover.rs",
      "crates/epic-orchestrator/src/server.rs",
      "crates/epic-orchestrator/src/socket.rs",
      "crates/epic-orchestrator/src/verbs_actions/launch.rs",
      "crates/epic-orchestrator/src/verbs_actions/teardown.rs"
    ],
    "input": "Engines over temp resource roots with registered epics: chain-vs-unblocks candidates, uncertain and cleanup-incomplete occupants, closed and abandoned stages, batches with held/waiting members, refresh and pause between launches, release errors with and without a live engine, five ticks without a slot, budget under floor, EPIC_HOST_COOLDOWN_MIN set"
  },
  {
    "id": "engine-recovery",
    "title": "Engine refresh, recovery per action, merge bridge and reopened judgment",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib refresh::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 6 && cargo test -p toolu-epic-orchestrator --lib refresh::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib recover::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 7 && cargo test -p toolu-epic-orchestrator --lib recover::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib merge_bridge::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 3 && cargo test -p toolu-epic-orchestrator --lib merge_bridge::tests:: && test \"$(cargo test -p toolu-epic-orchestrator --lib commit::tests:: -- --list 2>/dev/null | grep -c ': test$')\" -ge 6 && cargo test -p toolu-epic-orchestrator --lib commit::tests:: && cargo xtask guardrails",
    "ac_refs": [
      "AC-24",
      "AC-26",
      "AC-27",
      "AC-29"
    ],
    "depends_on": [
      "engine-refill"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src/refresh.rs",
      "crates/epic-orchestrator/src/recover.rs",
      "crates/epic-orchestrator/src/commit.rs",
      "crates/epic-orchestrator/src/github_engine.rs",
      "crates/epic-orchestrator/src/effects.rs",
      "crates/epic-orchestrator/src/effects_run.rs",
      "crates/epic-orchestrator/src/snapshot.rs",
      "crates/epic-orchestrator/src/tests",
      "crates/epic-orchestrator/src/reach.rs",
      "crates/epic-orchestrator/src/effects_cleanup.rs",
      "crates/epic-orchestrator/src/effects_epic.rs",
      "crates/epic-orchestrator/src/epic_close.rs",
      "crates/epic-orchestrator/src/merge_bridge.rs",
      "crates/epic-orchestrator/src/retry.rs",
      "crates/epic-orchestrator/src/retry/scope.rs",
      "crates/epic-orchestrator/src/retry/tests",
      "crates/epic-orchestrator/src/tracker/gh.rs",
      "crates/epic-orchestrator/src/tracker/rest.rs",
      "crates/epic-orchestrator/src/gate.rs",
      "crates/epic-orchestrator/src/verbs_actions/settle.rs",
      "crates/epic-orchestrator/src/logic.rs",
      "crates/epic-orchestrator/src/server.rs",
      "crates/epic-orchestrator/src/schedule.rs",
      "crates/epic-orchestrator/src/disk.rs",
      "crates/epic-orchestrator/src/model.rs",
      "crates/epic-orchestrator/src/epics.rs",
      "crates/epic-orchestrator/src/refill.rs"
    ],
    "input": "Journals with intents only for launch (starting/uncertain/running/no record), cleanup, refresh and close; a GitHub fixture that flips a PR to merged; EpicChanged; a Jira epic with the clock +15 min; a tracker failing once and three times; a merged record whose tracker issue reopens, then a restart"
  },
  {
    "id": "hook",
    "title": "Replace the three Bun SessionStart entries with native toolu epic-orchestrator hook check-binary, check-deps and engine-ensure",
    "check": "test \"$(cargo test -p toolu-runtime --lib startup::native -- --list 2>/dev/null | grep -c ': test$')\" -ge 4 && test \"$(cargo test -p toolu-cli --test epic_hook -- --list 2>/dev/null | grep -c ': test$')\" -ge 8 && cargo test -p toolu-cli --test epic_hook && cargo test -p toolu-runtime --lib startup && cargo test -p toolu-jev --lib check && cargo test -p toolu-review --lib check && cargo xtask gate --only hooks --only gate-coverage --only final-removal && cargo xtask guardrails",
    "ac_refs": [
      "AC-21"
    ],
    "depends_on": [
      "engine-recovery"
    ],
    "paths": [
      "crates/epic-orchestrator/src/hook.rs",
      "crates/epic-orchestrator/src/control.rs",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/core/runtime/src/startup.rs",
      "crates/core/runtime/src/startup",
      "crates/jev/src/check.rs",
      "crates/jev/src/tests",
      "crates/toolu-review/src/check.rs",
      "crates/toolu-review/src/tests",
      "crates/cli/src/hook/plugins.rs",
      "crates/cli/src/hook/tests",
      "crates/cli/tests/epic_hook.rs",
      "plugins/epic-orchestrator/hooks/hooks.json",
      "fixtures/gate-coverage/inventory.json",
      "docs/gate-coverage-matrix.md",
      "tools/toolu-opencode/generated/resources/repo/docs/gate-coverage-matrix.md"
    ],
    "input": "A Codex home with a codex executable printing a plugin list (missing, disabled, all present), a Claude environment, a session id, PATH with and without native toolu, an empty and a malformed registry.json"
  },
  {
    "id": "scenarios",
    "title": "Scripted engine scenarios: four-issue chain to closed, gone relaunch limit, cooldown reroute, coupling hold and release, Jira blocked child, OpenCode loss, credential leak",
    "check": "test \"$(cargo test -p toolu-epic-orchestrator --lib scenario -- --list 2>/dev/null | grep -c ': test$')\" -ge 7 && cargo test -p toolu-epic-orchestrator --lib scenario && test \"$(cargo test -p toolu-epic-orchestrator --lib leak -- --list 2>/dev/null | grep -c ': test$')\" -ge 1 && cargo test -p toolu-epic-orchestrator --lib leak && cargo xtask guardrails",
    "ac_refs": [
      "AC-15",
      "AC-16",
      "AC-17",
      "AC-18",
      "AC-20",
      "AC-30"
    ],
    "depends_on": [
      "hook"
    ],
    "paths": [
      "crates/epic-orchestrator/Cargo.toml",
      "Cargo.lock",
      "crates/epic-orchestrator/src/lib.rs",
      "crates/epic-orchestrator/src",
      "fixtures/epic"
    ],
    "input": "GitHub and Jira HTTPS fixtures, the herdr socket fixture with real git worktrees and scripted worker processes, a merge script under TOOLU_EPIC_SCRIPTS, a reroute before three pane losses, a resource cooldown and a rate-limited report, sentinel credentials"
  },
  {
    "id": "delete-scripts",
    "title": "Delete scripts/, hooks/src and hooks/dist; repoint the seam, the pr-babysit parity test, the OpenCode generator, scenarios and live worker test, and every stale path",
    "check": "test ! -e plugins/epic-orchestrator/scripts && test ! -e plugins/epic-orchestrator/hooks/src && test ! -e plugins/epic-orchestrator/hooks/dist && env -u npm_config_store_dir TMPDIR=/private/tmp bun test --timeout 60000 tools/toolu-conformance/src/harness/__tests__ tools/toolu-opencode/src/plugin/__tests__/epic-worker.live.test.ts tooling/src/__tests__/bundle-plugins.test.ts tooling/src/__tests__/check-retired-plugin-references.test.ts tooling/src/__tests__/pack-closure.test.ts tooling/src/opencode-host/__tests__ && cargo test -p xtask && cargo test -p toolu-epic-orchestrator --lib snapshot && cargo xtask gate --only dist --only pack --only final-removal",
    "ac_refs": [
      "AC-4",
      "AC-22",
      "AC-23"
    ],
    "depends_on": [
      "scenarios"
    ],
    "paths": [
      "plugins/epic-orchestrator",
      "tools/toolu-conformance/src/harness",
      "tools/toolu-opencode/scripts",
      "tools/toolu-opencode/src/plugin/__tests__/epic-worker.live.test.ts",
      "tooling/src",
      "tsconfig.json",
      "knip.json",
      "plugins/.oxlintrc.json",
      "crates/xtask/src",
      "docs/runtime.md",
      "crates/epic-orchestrator/src/tests/snapshot_test.rs",
      "fixtures/epic"
    ],
    "input": "The repository tree after deletion; the seam entries resolving the built toolu; fixtures/epic/hosts.json read by the parity test"
  },
  {
    "id": "markdown-docs",
    "title": "Swap SKILL.md, the epic command, recovery.md and README to toolu epic; update the OpenCode port text and its surface tests; update docs, AGENTS.md and the verify-epic-watcher skill; regenerate the OpenCode surfaces with bun run generate:opencode-surface",
    "check": "cargo xtask check-markdown-cli && env -u npm_config_store_dir TMPDIR=/private/tmp bun test --timeout 60000 tools/toolu-opencode/scripts/__tests__ && cargo test -p toolu-cli --test epic_markdown && env -u npm_config_store_dir TMPDIR=/private/tmp bun run check:opencode-surface && env -u npm_config_store_dir TMPDIR=/private/tmp bun run check:opencode-docs && bun run tooling/src/check-fixture-inventory.ts && cargo xtask gate --only docs --only context-budget --only portable-core",
    "ac_refs": [
      "AC-22",
      "AC-37"
    ],
    "depends_on": [
      "delete-scripts"
    ],
    "paths": [
      "plugins/epic-orchestrator",
      "docs",
      "AGENTS.md",
      "fixtures",
      ".toolu/skills/verify-epic-watcher/SKILL.md",
      "tools/toolu-opencode/generated",
      "tools/toolu-opencode/contract",
      "crates/cli/tests/epic_markdown.rs",
      "tools/toolu-opencode/scripts"
    ],
    "input": "The plugin Markdown, the generated docs/cli/commands.json, and the OpenCode generator output"
  },
  {
    "id": "gate",
    "title": "Full quality gate on the final tree, the Bun suites, the hook bench, and the release binary size",
    "check": "cargo xtask gate && env -u npm_config_store_dir TMPDIR=/private/tmp bun run test:unit && env -u npm_config_store_dir TMPDIR=/private/tmp bun run test:conformance && env -u npm_config_store_dir TMPDIR=/private/tmp bun run test:rust-conformance && env -u npm_config_store_dir TMPDIR=/private/tmp bun run bench:hooks && cargo build --release -p toolu-cli && ls -l target/release/toolu",
    "ac_refs": [
      "AC-37"
    ],
    "depends_on": [
      "markdown-docs"
    ],
    "paths": [
      "."
    ],
    "input": "The whole branch"
  }
]
```

Commits and pushes: each green step is committed locally (the orchestrator's
`refs/epic-wip` snapshots keep it recoverable). Intermediate states are not
pushed: `lefthook.yml` runs the full `cargo xtask gate` before every push that
touches `crates/**`, and bottom-up modules are only used by tests until the
`verbs` and `engine-*` steps wire them. The branch is pushed once, after the
`gate` step and the final `--verify`, with a recorded pre-push review.

## Critical files

- New: `crates/epic-orchestrator/src/{refs,retry,tracker,graph,hosts,route,workload,launch,finish,close,gate,coupling,jira_cli,probe_evidence,hook,verbs_actions,executor,effect_thread,effects_run,refill,holds,epics,refresh,dispatch_ops,socket_loop,herdr_calls}.rs` and their directories and tests; `crates/core/engine/src/resources/transitions.rs`; `crates/cli/tests/epic_{graph,launch,finish,probe,report,hook,markdown}.rs`; `fixtures/epic/{hosts,herdr-calls,epic248-graph,pr290-rollup}.json`, `fixtures/epic/{epic248,epic203}-body.md`.
- Modified: `crates/epic-orchestrator/src/{lib,model,logic,commit,server,socket,dispatch,schedule,recover,limit,snapshot,github_engine,github_budget,checkpoint,effects,herdr,herdr_argv,verbs,control}.rs`; `crates/core/engine/src/resources/{leases,jobs}.rs`; `crates/cli/src/hook/plugins.rs`; `plugins/epic-orchestrator/{hooks/hooks.json,README.md,commands/epic.md,skills/epic-orchestrator/**}`; `tools/toolu-conformance/src/harness/entry-command.ts`; `tools/toolu-opencode/src/plugin/__tests__/epic-worker.live.test.ts`; stale-path files listed in the spec.
- Deleted: `plugins/epic-orchestrator/scripts/`, `plugins/epic-orchestrator/hooks/src/`, `plugins/epic-orchestrator/hooks/dist/`.

## Verification

End to end, the scripted four-issue epic runs to a closed epic on the engine
with no `wait` caller, and the gone, cooldown, coupling, Jira and leak
scenarios hold, all on real git, a real Unix-socket herdr fixture and real TLS
fixtures. Boundaries: malformed records and routes, capacity, uncertain
launches, live workload, protection-only admin retries, refresh failures,
crashes between intent and done. Documentation: `docs/cli/` regenerated and
checked, the plugin Markdown passes `check-markdown-cli`, generated OpenCode
surfaces checked. Before delivery: `cargo xtask gate`, `bun run test:unit`,
and the release binary size before and after recorded in the PR.

Delivery readiness: each green step is committed with a scoped Conventional
Commit (`feat(epic): …`, `docs(epic): …`, `test(epic): …`); before the PR,
`env -u npm_config_store_dir TMPDIR=/private/tmp bun plugins/toolu/hooks/dist/plan-ledger.js
run docs/toolu/plans/2026-10-11-epic-engine-actions.md --verify` must report
every step fresh-green on the final tree, `toolu-review:review` records a
version-2 state with zero findings, and `verdict.js status` reports `overall:
ready`. Prerequisites checked before any delivery write: authenticated
`gh api user`, the non-default branch `feat/435-epic-engine-actions-graph-route`,
and the installed brainstorm, toolu, toolu-review and pr-babysit skills. The PR
targets `main`, starts with `Closes Falconiere/toolu#435` and `Part of
Falconiere/toolu#402`, states the gate scope transfer from #448 and the AC-19
reading of the refill scenario, records the release binary size before
(7,472,112 bytes, macOS arm64, built at the branch base e68f7528 before any
code change) and after, and hands off to `pr-babysit:babysit`. Babysit on this repository needs the
`merge-approved` label from an approved verdict, a reply and resolve on every
review thread (outdated ones included), and a re-run of any cancelled
`merge-gate` job. `bun run test:opencode` (the required OpenCode acceptance)
runs in the `opencode` CI job; it is also run locally when the pinned host is
cached, and its result is recorded in the PR.

## Deviations

- `refs-retry`: the retry guard counts `retry::` (the module and its `map` and
  `reset` submodules, 18 or more) instead of `retry::tests::` only.
- `trackers`: Linear sends a personal key bare, which `toolu-http` could not
  express (it refuses an `Authorization` header outside `Auth`), so
  `toolu_http::Auth::Raw` is added, redacted in `Debug` and dropped on
  redirects like the other variants (`crates/core/http/src/auth.rs` and its
  test). Tracker HTTP errors also redact the Jira and Linear credentials
  (AC-30).
- `hosts`: `fixtures/index.json` also renames one `opencode-permission-evaluate`
  case to the name #508 gave it in `fixtures/opencode/permission-evaluate.json`,
  and the five `statusline` cases #510 renamed in `fixtures/statusline/cases.json`;
  without that, `check-fixture-inventory.ts` fails on `main` before this
  change and the step's check could not pass.
- `hosts`: the guard counts 5 tests, not 14: each test replays one case family
  of `fixtures/epic/hosts.json` (81 cases: argv, skills, aliases, limit texts,
  OpenCode versions), so a missing case fails a test rather than the count.
- `resources`: the transitions guard counts 5 tests (full or cooling
  destination, reserve-migrate-cancel, refusals, live job against the fence,
  fence against new jobs), the TypeScript cases split by phase; the
  `LeasePatch` gains `worktree`, `pane` and `session`, so the existing patch
  test now asserts their key order too.
- `herdr-calls`: the herdr socket fixture lives in
  `src/tests/herdr_fixture_test.rs`, `herdr_state_test.rs` and
  `herdr_agents_test.rs`, wired from `lib.rs` like `support_test.rs`, not in
  `src/tests/helpers/`: guardrails accept `#[path]` in `src` only for a flat
  `tests/<name>_test.rs`, and `helpers/` only under a crate's integration
  `tests/`. The socket client also gains the call's CLI deadline as its read
  timeout (a passed deadline reads `timed out`, so the call is uncertain) and
  hands `result_of` the raw reply line, so a non-JSON socket reply reports
  `non-JSON output` as the CLI form does; the CLI form keeps a herdr error
  object printed with a non-zero exit, as `common.ts` does.
- `launch`: the default branch comes from the `toolu-github` client
  (`GET /repos/<repo>`, as the spec routes GitHub through it) instead of
  `gh repo view`; the dry-run log says `# dry-run: skip the default-branch
  lookup; assume main`. `gh repo clone` stays for a missing checkout.
  `--reprompt` and `--replace` become one `Recovery` value (replace includes
  reprompt) to keep `LaunchOpts` within three `bool` fields. The brief template
  drops the `REPORT` and `JOB` placeholders for literal `toolu epic report` and
  `toolu epic job`. A record whose `session_started_after` overflows `f64`
  (`1e400`) is refused as malformed JSON, since JSON has no infinity to
  carry to the epoch check. The socket tests' 1 s polling windows widen to
  5 s: the launch tests' processes made them flaky under parallel load.
- `finish`: the merge check reads the PR through the `toolu-github` client
  (`GET /repos/<repo>/pulls/<n>`: `merged`, else the uppercased `state`)
  instead of `gh pr view`. Warnings the script wrote to stderr (no
  `workspace_id`, a failed `herdr worktree remove`) come back with the
  document for the CLI to print. An absent worktree without removal
  evidence fails with `cannot resolve worktree …` where Node said `ENOENT`.
  A finish dry run validates the record and the PR and prints the planned
  actions.
- `gate-close-coupling`: the gate reads the PR in one GraphQL query (state,
  draft, mergeability, `mergeStateStatus`, head, checks, threads, auto-merge)
  plus the compare and workflow endpoints, and merges through `PUT
  /repos/<repo>/pulls/<n>/merge` pinned to the head. As `gh pr merge` does,
  a `BLOCKED` merge state is a protection refusal before any request, so the
  admin attempt (the same pinned `PUT`) only follows it or a refusal whose
  text names protection; a timed-out merge is `uncertain` and not retried.
  Auto-merge arms through `enablePullRequestAutoMerge` (expected head) and
  disarms through `disablePullRequestAutoMerge`; GitHub has no
  `--delete-branch` there, so head-branch deletion follows the repository
  setting. A gate dry run assesses only. `probe --check-evidence` without a
  file checks the evidence the binary embeds, since the binary has no repo
  path. `toolu-github`'s `Gh` gains `write_once`, a write that keeps GitHub's
  own error.
- `engine-effects`: the socket loop's deadline and herdr feeds
  (`socket_deadline.rs`, `socket_herdr.rs`) and `dispatch.rs` follow the
  move of the state loop into `socket_loop.rs`.
- `engine-refill`: the in-process launch body lives in `effects_launch.rs`
  (routing, admission and the launch itself on the effect thread), with
  `route/score.rs` sharing the Jev client; `server.rs`, `socket.rs`,
  `recover.rs` and the launch and teardown verbs pick up the per-epic state
  and the environment floors and cooldown.
- `engine-recovery`: cleanup runs in process (`effects_cleanup.rs`: finish,
  then settle unless the tracker already reports the item closed), and the
  epic refresh and close have their own effect bodies (`effects_epic.rs`)
  and state-thread halves (`refresh.rs`, `epic_close.rs`); the merge bridge
  is `merge_bridge.rs`. Effects reach GitHub and the trackers through
  `reach.rs`, so tests point them at a loopback fixture. One effect's sleep
  budget is shared by every client it builds through a thread-scoped budget
  (`retry/scope.rs`); `Gh` and the Jira and Linear HTTP client take it. With
  `Retry-After: 2` the first wait is 3 s (the reset plus the 1 s margin
  every rate-limit wait carries), so the AC-34 case fails after about 3 s on
  the second request rather than 2 s. Logic tests that drove cleanup through
  a script now see the in-process finish refuse their bare records
  (`cleanup-incomplete`). A failed or uncertain close journals `close-done`
  with that note so a restart neither re-raises it nor comments again; an
  uncertain launch likewise journals `launch-done` with the note
  `uncertain`. `toolu epic gate --merge --key K` sends `event merged` to a
  live engine.
- `hook`: `ensure` reads `registry.json` once and fails on malformed JSON
  (the detail names the file), so `engine-ensure` reports it; the three
  gate-coverage rows keep their ids and move to `native` under #435. The
  epic CLI test helpers return `Result` (clippy denies `expect` outside a
  test function), and each binary includes only the helper files it uses.
- `scenarios`: the scenarios play the workers by sending their reports to
  the engine (the agents are the herdr fixture's processes), and close a
  child in the fixture when its PR merges, as GitHub's closing keyword does,
  so the settlement finds it closed. The scenario checkout's origin is a
  local bare repository under `github.com/o/r.git`, which the graph resolves
  as `o/r` and launches fetch from. An in-process launch now records its
  host on the issue (so a rate-limited report cools that host), builds its
  GitHub client through `Reach`, and replaces the recorded agent when the
  record is still `running` (a relaunch after a lost pane). A seventh
  scenario drives the merge bridge end to end without merge scripts.
- `delete-scripts`: the OpenCode live worker test now plans with `toolu epic
  launch --dry-run --json` (prompt, agent flags, brief and `info/exclude`
  lines) and resumes from the plan a recorded session gives; with no
  `toolu epic checkpoint` verb it checks that the killed worker's edit is
  still in the worktree instead of snapshotting it, and it reads the ready
  report from the status file instead of the deleted watcher. The pr-babysit
  parity test drops the usage-limit wording check (pr-babysit has no limit
  expression of its own; the Rust hosts tests keep those cases). The xtask
  Bun-launcher test now reads pr-babysit's committed `check-toolu` entry.
- `markdown-docs`: the skill drops the plugin-root block and the background
  watcher on every host: the orchestrator starts the engine and loops on
  `toolu epic wait --max-seconds 480` with a judgment table, so the OpenCode
  port keeps only its skill-list and brief-header edits. The verify-epic-watcher
  project skill now drives the engine. `toolu-engine`'s `leases` module doc
  links `crate::resources::transitions` (rustdoc rejected `super::`).
- `review`: the pre-push review added behavior beyond the steps. `toolu epic
  start <state_dir> --safe` records the choice in `<state_dir>/launch.json`
  (an unreadable file keeps prompts on) and the engine's launches for that
  epic honor it, so the skill's `--safe` reaches engine-run workers. The
  merge gate follows the check-context and review-thread cursors past 100.
  A held, waiting or failed launch journals `launch-done` with that note so a
  restart neither relaunches nor finishes it, and a relaunch that waits stays
  pending until its retry time. Tracker HTTP failures are classified by
  status only, so the text classifier `retry::classify` is gone. herdr
  worktree and listing calls get 120 s, replies are read whole up to 16 MiB,
  a CLI `agent read` answers its text, and herdr failures keep their exit
  status. The new tests live in the existing test files plus
  `src/tests/commit_launch_test.rs` and
  `crates/core/engine/src/resources/tests/transitions_rig_test.rs`.
- `rebase`: #512 removed pr-babysit's TypeScript, so the TypeScript half of
  the hosts parity check (`plugins/pr-babysit/hooks/src/__tests__/hosts-parity.test.ts`)
  went with it; `fixtures/epic/hosts.json` is replayed by
  `crates/epic-orchestrator/src/tests/hosts_test.rs` alone. #513's shared
  `toolu_runtime::startup::native_toolu_hook_output` replaces this branch's
  `native_advice_output`; its attention prompts and notifications keep their
  synchronous path beside the effect thread.
- `rebase` (checks): the merge put this branch's `control`, `limit` and
  `schedule` tests beside #513's in second wired modules
  (`control::replace_tests`, `limit::reroute_tests`, `schedule::gone_tests`),
  so `engine-effects` and `engine-refill` count `control::`, `limit::` and
  `schedule::` instead of `…::tests::`. #512 made pr-babysit's hooks native,
  so the xtask Bun-launcher test reads rust-quality's `register` entry.
