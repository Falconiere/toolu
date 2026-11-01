# Plan: worker job wait and bounded output

**Date:** 2026-11-01   **Status:** Approved   **Spec:** docs/toolu/specs/2026-11-01-worker-job-wait-design.md   **Topic:** `toolu epic job` waits for admission and prints a bounded log tail; Claude workers get long Bash timeouts; worker texts stop asking for sleep-and-retry

## Evidence and approach

The shared runner `run_managed_job` (`crates/core/engine/src/resources/jobs.rs`) is called by the job verb (`crates/epic-orchestrator/src/job.rs`), the ledger's managed check (`crates/core/engine/src/ledger/check.rs`) and three tests. Refusals come from `acquire_lease` as `job capacity exhausted (N)` or `resource hold: …`; `job.rs::failure` already classifies those two as `capacity` and `pressure`. The job log is discoverable after the run through `job_logs::list(root, key)` and readable through `job_logs::read(entry, None, limit)`, which returns the last 64 KiB window by default. `hosts::agent_args` builds the Claude argv behind a safe-character regex that a state-directory path satisfies; `AgentOpts` derives `Default`, so the hc remote and review argv keep `settings: None` untouched. The attempt knows the state directory (`self.graph.state_dir`) where it writes briefs and status. Verb flags are declared in `verbs.rs` and dispatched in `query.rs::job`.

Design: a `JobWait` with bound, poll and observer on a new `run_managed_job_waiting`; the old name keeps its signature as the no-wait form. The ledger managed check reads its bound from `PLAN_LEDGER_JOB_WAIT` in the check environment. The verb journals `job` per attempt and `job-wait` per refusal through the observer (same records as today's external retries), then prints a bounded tail read from the log catalog. The Claude launch writes `<state>/claude-settings.json` and passes `--settings`.

## Workstream summary

runner wait → verb wait, flag and tail → Claude settings → worker texts → generated CLI docs and compat.

## Steps (machine-readable)

```json
[
  {
    "id": "runner-wait",
    "title": "JobWait and run_managed_job_waiting in the engine runner; the ledger managed check waits by default",
    "ac_refs": ["AC-1", "AC-2", "AC-10"],
    "paths": ["crates/core/engine/src/resources/jobs.rs", "crates/core/engine/src/resources/tests/jobs_test.rs", "crates/core/engine/src/ledger/check.rs", "crates/core/engine/src/ledger/tests/check_test.rs"],
    "input": "a temp resource root with policy maxJobs 1, one occupied job lease released from a thread after 300 ms, and the command printf ok; a bound git project with PLAN_LEDGER_JOB_WAIT=5 then 0; boundary: a 100 ms bound that passes while the lease is still held",
    "check": "cargo test -p toolu-engine --lib resources::jobs::tests && cargo test -p toolu-engine --lib ledger::check::tests",
    "model": "inherit"
  },
  {
    "id": "verb-wait-and-tail",
    "title": "toolu epic job waits with journaled attempts, takes --no-wait, and prints the bounded log tail",
    "ac_refs": ["AC-3", "AC-4", "AC-5", "AC-6"],
    "depends_on": ["runner-wait"],
    "paths": ["crates/epic-orchestrator/src/job.rs", "crates/epic-orchestrator/src/tests/job_test.rs", "crates/epic-orchestrator/src/query.rs", "crates/epic-orchestrator/src/verbs.rs"],
    "input": "a temp resource root bound to a git worktree; an occupied lease released after 300 ms with a 5 s bound and 50 ms poll; sh -c 'seq 1 100; echo boom >&2; exit 3'; seq 1 100; seq 1 40; boundary: --no-wait against the held lease",
    "check": "cargo test -p toolu-epic-orchestrator --lib job::tests",
    "model": "inherit"
  },
  {
    "id": "claude-settings",
    "title": "Claude launches write <state>/claude-settings.json and pass --settings; fixture case and tests",
    "ac_refs": ["AC-7"],
    "paths": ["crates/epic-orchestrator/src/hosts.rs", "crates/epic-orchestrator/src/tests/hosts_test.rs", "fixtures/epic/hosts.json", "crates/epic-orchestrator/src/launch/agent.rs", "crates/epic-orchestrator/src/launch/attempt.rs", "crates/epic-orchestrator/src/launch/tests/agent_test.rs", "crates/epic-orchestrator/src/launch/tests/attempt_test.rs"],
    "input": "fixtures/epic/hosts.json args cases including a new Claude case with settings; a dry-run launch plan against a temp state directory; boundary: Codex opts with settings set still get no --settings",
    "check": "cargo test -p toolu-epic-orchestrator --lib hosts::tests && cargo test -p toolu-epic-orchestrator --lib launch::agent::tests && cargo test -p toolu-epic-orchestrator --lib launch::attempt::tests",
    "model": "inherit"
  },
  {
    "id": "worker-texts",
    "title": "Worker brief, delivery-flow execution reference and plugin README describe the blocking job, the tail and the gate cadence",
    "ac_refs": ["AC-8"],
    "depends_on": ["verb-wait-and-tail"],
    "paths": ["plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md", "plugins/delivery-flow/skills/delivery-flow/references/execution.md", "plugins/epic-orchestrator/README.md", "crates/epic-orchestrator/src/launch/tests/brief_test.rs", "crates/epic-orchestrator/src/launch/brief.rs"],
    "input": "the rendered brief for Claude and Codex from graph_248; the three Markdown files",
    "check": "cargo test -p toolu-epic-orchestrator --lib launch::brief::tests && cargo test -p toolu-cli --test epic_markdown && cargo xtask check-markdown-cli && cargo xtask context-budget",
    "model": "inherit"
  },
  {
    "id": "cli-docs",
    "title": "Regenerate docs/cli for --no-wait and keep the CLI contract and compat checks green",
    "ac_refs": ["AC-9"],
    "depends_on": ["verb-wait-and-tail"],
    "paths": ["docs/cli/epic.md", "docs/cli/commands.json", "docs/cli/commands.md", "crates/cli/tests/fixtures/commands_json.snap", "crates/cli/tests/contract.rs"],
    "input": "the built toolu binary's command tree",
    "check": "cargo xtask docs-cli --check && cargo xtask check-cli-compat && cargo test -p toolu-cli --test contract",
    "model": "inherit"
  }
]
```

## Critical files

- `crates/core/engine/src/resources/jobs.rs`, `crates/core/engine/src/resources/tests/jobs_test.rs`, `crates/core/engine/src/ledger/check.rs`
- `crates/epic-orchestrator/src/job.rs`, `crates/epic-orchestrator/src/tests/job_test.rs`, `crates/epic-orchestrator/src/query.rs`, `crates/epic-orchestrator/src/verbs.rs`
- `crates/epic-orchestrator/src/hosts.rs`, `crates/epic-orchestrator/src/tests/hosts_test.rs`, `fixtures/epic/hosts.json`, `crates/epic-orchestrator/src/launch/agent.rs`, `crates/epic-orchestrator/src/launch/attempt.rs`, their tests
- `plugins/epic-orchestrator/skills/epic-orchestrator/references/worker-brief.md`, `plugins/delivery-flow/skills/delivery-flow/references/execution.md`, `plugins/epic-orchestrator/README.md`
- `docs/cli/epic.md`, `docs/cli/commands.json`, `docs/cli/commands.md`, `crates/cli/tests/fixtures/commands_json.snap`

## Verification

End to end: `cargo xtask gate` on the branch (in the toolu-gate container if the macOS host trips host-environment tests). Real inputs: the temp resource roots and git worktrees above, the shipped fixtures and Markdown. Failure and boundary: bound passes while held (AC-2), `--no-wait` refusal (AC-4), log missing after the run (`full log: unavailable`), exactly 40 lines of success output prints whole (AC-6). Docs: README paragraph, brief, execution reference, regenerated `docs/cli`. After the next epic run, `toolu epic metrics --epic <key> --json` should show `jobs.waits` near zero and the rollout profile fewer `sleep` and `wait` calls.
