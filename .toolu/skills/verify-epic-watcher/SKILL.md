---
name: verify-epic-watcher
description: Verify epic watcher locks, restarts, backoff, events, and checkpoints against real Herdr and Git.
metadata:
  toolu:
    origin: agent
    created: 2026-10-03T18:10:08Z
---
## When to Use

Use after changing epic watcher ownership, checkpoint cadence, dependency
backoff, or event ordering. It verifies the real CLI against an isolated Herdr
server and a real linked Git worktree.

## Procedure

1. Create a disposable bare origin, clone, and linked worktree. Give it tracked,
   staged, untracked, and deleted changes when checkpoint recovery is in scope.
2. Use an isolated real Herdr session and its actual socket. Never put a canned
   `herdr` executable on `PATH`. Create or borrow a named agent only within the
   isolated session, and record the Herdr version.
3. Point `TOOLU_RESOURCE_HOME` and the epic state directory at disposable roots.
   Run the real `epic-watch.ts` CLI without `--peek`.
4. Start one waiting watcher, wait for its lock, then start a competitor. Record
   the busy result and latency. Stop the owner and verify its token lock is gone.
5. Create two active records with absent agent names that share the dirty linked
   worktree. Enable `GIT_TRACE2_EVENT`, run one watcher iteration, and record
   event latency, checkpoint count, and Git command counts.
6. Restart immediately. Verify checkpoint and budget deadlines are unchanged,
   no checkpoint is returned, and the Git trace process count does not grow.
7. Point the real Herdr CLI at an absent isolated socket, run once to persist a
   retry deadline, then restart before it. Verify failure count and retry time
   stay unchanged and no second dependency error is emitted.
8. Save machine-readable measurements under `docs/toolu/evidence/` and state
   which acceptance criteria and host lifecycle cases remain outside the probe.

## Resident engine

Use this after the TypeScript watcher procedure when the change is the resident
`toolu epic engine`.

1. Point `TOOLU_RESOURCE_HOME` at a disposable root. Register epic directories
   only under that root.
2. Run `toolu epic engine` in the foreground for that root. A second start must
   exit with `engine-busy` and must leave the owner's lock in place.
3. Do not start the user systemd unit. `toolu epic service install` only writes
   unit files. Do not `herdr session attach`, and do not stop the default
   Herdr session. An absent-session probe is `herdr --session <name> agent list`.
4. Checkpoint a dirty worktree with `GIT_TRACE2_EVENT`. Count `start` events for
   `read-tree`, `add -A`, `write-tree`, `commit-tree`, and `update-ref`. An
   immediate reload of the same root must add no Git processes.
5. SessionStart `engine-ensure` runs `toolu epic engine --ensure` and stays
   silent when no native `toolu` is on `PATH`.

## Pitfalls

- Do not use or close another agent's Herdr workspace.
- `--peek` does not prove lock ownership, restart persistence, or snapshots.
- A prewritten future deadline only proves loading. Also let a first watcher run
  generate checkpoint and budget deadlines, then restart it.
- Count Git trace2 `start` records before and after restart; reflog length alone
  misses redundant index and candidate-commit work.
- Keep resource roots disposable so tests never read or write developer state.

## Verification

- The competing watcher reports `watcher-busy`, and owner shutdown removes only
  its own lock.
- Two records sharing one worktree produce at most one checkpoint per iteration.
- The first dirty snapshot has one `read-tree`, `add -A`, `write-tree`,
  `commit-tree`, and `update-ref`; immediate restart adds zero Git processes.
- Generated checkpoint and budget deadlines survive restart unchanged.
- Herdr failure count and retry deadline survive restart unchanged during the
  backoff window.
- Focused watcher/checkpoint tests, TypeScript, formatting, and lint all pass.
