---
name: verify-epic-watcher
description: Verify the resident epic engine's locks, restarts, backoff, events and checkpoints against real Herdr and Git.
metadata:
  toolu:
    origin: agent
    created: 2026-10-03T18:10:08Z
---
## When to Use

Use after changing the resident `toolu epic` engine's ownership, checkpoint
cadence, dependency backoff, event ordering or in-process actions. It verifies
the real binary against an isolated Herdr server and a real linked Git
worktree. (The TypeScript watcher this skill first covered is gone; the engine
replaced it in #434 and #435.)

## Procedure

1. Create a disposable bare origin, clone, and linked worktree. Give it tracked,
   staged, untracked, and deleted changes when checkpoint recovery is in scope.
2. Use an isolated real Herdr session and its actual socket. Never put a canned
   `herdr` executable on `PATH`. Create or borrow a named agent only within the
   isolated session, and record the Herdr version. An absent-session probe is
   `herdr --session <name> agent list`.
3. Point `TOOLU_RESOURCE_HOME` and the epic state directory at disposable roots.
   Register epic directories only under that root (`toolu epic start <dir>`).
4. Run `toolu epic engine` in the foreground for that root. A second start must
   exit with `engine-busy` and must leave the owner's lock in place. Stop the
   owner by ending its process, and verify its lock is gone only once the
   process has ended. `toolu epic engine --replace` is not a stop: it starts a
   successor, and the lock passes to that successor.
5. Create two active records with absent agent names that share the dirty linked
   worktree. Set `TOOLU_EPIC_GIT_TRACE` (the engine's `GIT_TRACE2_EVENT` file),
   let one checkpoint wave run, and record the checkpoint count and Git command
   counts: `start` events for `read-tree`, `add -A`, `write-tree`,
   `commit-tree`, and `update-ref`.
6. Restart immediately. Verify the checkpoint and budget deadlines in
   `watch.json` are unchanged, no checkpoint repeats, and the Git trace process
   count does not grow.
7. Point Herdr at an absent isolated socket, let the engine persist a retry
   deadline, then restart before it. Verify the failure count and retry time
   stay unchanged and no second dependency error is journaled.
8. Do not start the user systemd unit: `toolu epic service install` only writes
   unit files. Do not `herdr session attach`, and do not stop the default
   Herdr session.
9. The SessionStart `engine-ensure` hook (`toolu epic-orchestrator hook
   engine-ensure`) starts the engine in process when an epic is registered and
   stays silent otherwise; a malformed `registry.json` is one `systemMessage`.
10. Judgment wait: with no engine, `toolu --json epic wait --max-seconds 0`
    prints `{"state":"waiting"}` at once. With the engine running, start
    `toolu --json epic wait` (a background Bash call on Claude Code; a loop of
    `--max-seconds <n>` elsewhere), then raise a judgment with `toolu epic
    report needs-human --status-file <epic dir>/status/<key>.json --note <text>`.
    The wait must return that item (`kind`, `key`, `note`); `toolu epic ack
    <key>` clears it before the next probe. Run an engine for this with
    `toolu epic engine --no-http` so it never takes the real dashboard port.
11. Save machine-readable measurements under `docs/toolu/evidence/` and state
    which acceptance criteria and host lifecycle cases remain outside the probe.

## Pitfalls

- Do not use or close another agent's Herdr workspace.
- `toolu epic status` does not prove lock ownership, restart persistence, or
  snapshots.
- A prewritten future deadline only proves loading. Also let a first engine run
  generate checkpoint and budget deadlines, then restart it.
- Count Git trace2 `start` records before and after restart; reflog length alone
  misses redundant index and candidate-commit work.
- Keep resource roots disposable so tests never read or write developer state.

## Verification

- The competing engine reports `engine-busy`, and owner shutdown removes only
  its own lock.
- Two records sharing one worktree produce at most one checkpoint per wave.
- The first dirty snapshot has one `read-tree`, `add -A`, `write-tree`,
  `commit-tree`, and `update-ref`; immediate restart adds zero Git processes.
- Generated checkpoint and budget deadlines survive restart unchanged.
- Herdr failure count and retry deadline survive restart unchanged during the
  backoff window.
- The focused engine tests (`cargo test -p toolu-epic-orchestrator`) and
  `cargo xtask gate` pass.
