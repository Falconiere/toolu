# Epic workload ownership and supervision — Design

**Date:** 2026-10-03   **Status:** Approved   **Author:** Codex   **Topic:** Issue #376

## Problem

Parallel epic workers can bypass capacity and outlive their accounting.
Watcher restarts repeat expensive checkpoint work and stale working jobs go
unreported. The historical CPU-starvation incident motivates enforcing
bounded work; its exact crash trigger is unproven.

## Non-Goals

1. Diagnose or repair hypervisor contention from guest code.
2. Replace the orchestrator with a daemon or weaken mandatory quality checks.
3. Treat missing live-host credentials as successful conformance evidence.

## Architecture

Use portable core process execution with deadlines, drained bounded output,
group cancellation and observable exit status. Resource state lives in a shared
per-user machine directory, outside host-specific epic homes. Serialize resource
admission with exclusive owner-token locks. Persist launch ownership before
starting/prompting; uncertain attempts remain reserved until reconciliation.

Keep separate agent and expensive-job limits. Bind managed worktrees to resource
state; the ledger runner acquires job capacity automatically, and an explicit
job CLI covers tests outside the ledger. Preserve leases while background
descendants are alive. Sample load, available memory and Linux CPU steal at a
bounded cadence; sustained pressure blocks new work and recovery uses hysteresis.

Persist watcher checkpoint/budget/retry deadlines. Inspect urgent events before
optional snapshots. Serialize each worktree's independent checkpoint index;
reuse it when HEAD is unchanged and compare tree plus ancestry before creating
a commit. Final recovery snapshots must succeed before destructive teardown.

Extend host adapters with explicit capability and typed lifecycle outcomes.
Verify live identity, pane, cwd and captured session before reuse. Native failures
retain exit codes; uncertain start/prompt cannot automatically trigger fresh work.

## Interfaces / Schema

- Core `process` module: `runCommand(argv, {cwd, env, stdin, timeoutMs,
  maxOutputBytes, signal})` returns stdout, stderr, exit code, duration,
  timeout/cancellation and truncation information; no conformance dependency.
- `TOOLU_RESOURCE_HOME` overrides the shared resource root. Its versioned
  policy/state contains agent limits, job limits, pressure history, leases and
  worktree bindings. Defaults are conservative and documented.
- Lease records include token, issue, state directory, host, process identity,
  pane, worktree, session and lifecycle stage as each becomes known. Missing
  identity never implies successful cleanup. Shared host cooldowns apply to
  all epic state directories using this root.
- Launch stages include starting, running, uncertain and cleanup-incomplete;
  all occupy capacity. Terminal stages require verified shutdown/removal.
- `job.ts -- <command> ...` runs an expensive command under shared admission.
  Heartbeat/job identity is separate from phase/meaningful-progress timestamps.
- Watch state persists next checkpoint, next budget query, dependency retry,
  repeated-stall deadlines and acknowledgements; `--peek` consumes no state.
- Lifecycle/probe output distinguishes started, resumed, uncertain, blocked,
  provider-limited, exited, cleanup-incomplete and unverified capabilities.

## Failure modes and edge cases

Reject invalid/nonfinite timing and capacity settings. Cached routes remain
preferences and are revalidated against pool membership/capacity; null routes
cannot launch. Concurrent callers and multiple epics share admission. Existing
workers are counted once. Corrupt shared ownership fails closed.

Command timeouts and prompt/start failures retain uncertainty and native errors.
No automatic fresh retry follows an ambiguous mutation. Cleanup failure retains
capacity and recovery metadata. Never remove dirty/unpushed work after a failed
snapshot, for merge or abandonment. Lock release checks owner tokens; unique
temporary files prevent competing writers from colliding.

Stale progress is reportable for working and idle agents with live jobs; parked
ready/needs-human phases remain quiet. Repeated unacknowledged stalls escalate
at bounded intervals. Missing hosts/providers yield explicit unverified results.

## Acceptance criteria

- **AC-1:** Fixture routes respect cached-batch limits, removed/reduced pools,
  active workers and null-host refusal; concurrent launch reservations in two
  epic state directories share machine/host limits.
- **AC-2:** Real commands with children/grandchildren and large stderr finish
  or time out within a deadline, drain output, and leave no owned running
  descendants after cancellation; native nonzero status is retained.
- **AC-3:** Concurrent real managed jobs obey a separate shared limit, retain
  leases while descendants run, recover dead ownership safely, and mandatory
  ledger checks use that admission path for bound worktrees.
- **AC-4:** Recorded incident CPU samples trigger sustained pressure holds;
  recovery requires a lower-pressure window. Bounded local work demonstrates
  admission without killing existing useful jobs.
- **AC-5:** Real linked worktrees preserve tracked/staged/untracked/deleted
  content and worker index through HEAD changes. Unchanged snapshots avoid
  candidate commits and redundant index initialization; cleanup refuses failed
  final snapshots.
- **AC-6:** Actual watcher runs with isolated state and real herdr exercise
  exclusive ownership, restart cadence, dependency backoff, timely urgent
  events and at most one checkpoint per worktree/iteration.
- **AC-7:** Stale working/idle jobs produce bounded repeat alerts independent
  of heartbeat; parked workers remain quiet; invalid CLI timing fails early.
- **AC-8:** Launch persists attempt ownership before prompt and reconciles
  uncertain outcomes; reuse validates live identity. Shutdown/removal failures
  remain retryable and continue occupying capacity.
- **AC-9:** Bounded isolated Claude, Codex, Cursor and OpenCode probes record
  installed versions and actual start/tool/session/background/cancel/teardown
  evidence, explicitly marking unavailable capabilities unverified.
- **AC-10:** Measured checkpoint/subprocess counts, event latency, CPU and
  survivors are reported with reproducible checks; docs and full quality gate
  agree with the implemented behavior.

## Acceptance evidence

AC-1/8: committed epic248 graph, sandboxed shared state, concurrent real Bun
reservation processes and live lifecycle probes; run epic script tests.
AC-2/3: real shell/Bun process trees, large stderr, concurrent managed jobs,
ledger subprocesses and dead-owner recovery; run core process/resources/ledger
tests. Include timeout, cancellation and background-child cases.
AC-4: replay incident samples recorded in #376 plus portable local sampling
and bounded CPU commands; resource policy tests and probe report.
AC-5: real Git bare origin/clone/linked worktrees with trace2 and index byte
checks, failed snapshots and HEAD changes; checkpoint and finish tests.
AC-6/7: actual watcher CLI in isolated epic/worktree state on a controlled
herdr session, plus status records written by report.ts; watcher tests/probes.
AC-9: installed-host lifecycle probe command, isolated profiles and bounded
deadlines, observed artifacts/session IDs and cleanup evidence. Unavailable
providers are reported as an explicit remaining verification limitation.
AC-10: compare real command counts and resource usage, run `bun run test`,
generated OpenCode/bundle checks, final plan-ledger verification and review.

## Documentation impact

Update epic README, skill, worker brief/recovery guidance, portable runtime
documentation, generated OpenCode surface and committed hook bundles affected
by core changes. Record measured probe evidence and verification limitations.

## Open Questions

No implementation blockers. Codex owns determining installed-host/provider
availability during isolated probes; unavailable capabilities cannot be
represented as passing live evidence.
