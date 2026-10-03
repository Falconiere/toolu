# Epic watcher probe — 2026-10-03

This probe exercised the real `epic-watch.ts` CLI against Herdr 0.9.3 in the
isolated `toolu-376-hostprobe-20261003` server. It used a disposable bare Git
origin, clone, linked worktree, epic state, and shared resource root. The Herdr
CLI and server were real; the probe did not put a canned executable on `PATH`.

Two watcher processes targeted the same epic state. The second returned
`watcher-busy` in 38 ms. Sending SIGTERM to the owner returned exit 130 and its
token-checked lock was gone afterward.

The event probe created two active records for absent agent names which pointed
to the same dirty linked worktree. The worktree contained tracked, staged,
untracked, and deleted content. One watcher iteration emitted both `gone`
events in 407 ms and made one checkpoint. Git trace2 recorded one `read-tree`,
one `add -A`, one `write-tree`, one `commit-tree`, and one `update-ref`. An
immediate watcher restart emitted a heartbeat, created no checkpoint, added no
Git process, and preserved the checkpoint and budget deadlines.

A separate clean-worktree run began without a watcher state file. The first
actual watcher invocation generated its next checkpoint and budget deadlines
and ran four Git processes while establishing that the worktree had nothing at
risk. An immediate restart preserved both generated deadlines and left the Git
trace at four processes, a restart delta of zero.

The dependency probe then pointed the real Herdr CLI at an absent isolated
socket. The first watcher run emitted `herdr-error`, persisted one failure and a
retry deadline. An immediate restart kept the same failure count and deadline
and did not spawn Herdr again, demonstrating restart-safe backoff.

The machine-readable measurements are in
`docs/toolu/evidence/epic-watch-2026-10-03.json`. This probe covers AC-6 watcher
ownership, restart cadence, dependency backoff, urgent event latency, and
per-worktree checkpoint deduplication. Host start, prompt, background-job,
cancel, and teardown behavior belongs to the separate lifecycle probe.
