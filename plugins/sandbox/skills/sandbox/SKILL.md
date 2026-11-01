---
name: sandbox
description: Run builds, gates and agent CLIs in toolu's Linux sandbox (a Lima VM on macOS, this machine on Linux) with `toolu sandbox run`, outside macOS seatbelt, TCC and Gatekeeper, and faster. Use to run the gate in the sandbox, test on Linux, run host-smoke, or when macOS policy blocks or slows a build.
---

# toolu sandbox

`toolu sandbox run` replaces toolu with a command that runs in a Linux sandbox at
the same path, so its output, exit status and Ctrl-C are the command's own. On
macOS the sandbox is a Lima VM (Debian 13) that mounts only this project; on
Linux it is this machine.

## When to use it

- The full gate, coverage or `cargo test` on macOS: no Gatekeeper or XProtect
  scan of every fresh test binary, a native Linux build like CI's.
- `cargo xtask host-smoke` for Claude Code, Codex, Cursor or Muse on Linux, where
  Codex and Muse sandbox with bubblewrap and Landlock.
- A command that macOS policy prompts for, stalls on, or refuses.

Do not use it to edit files: the mount already shares them both ways.

## Check first

```bash
toolu sandbox status --json
```

`state` is `running`, `stopped` (the next `run` boots it), `absent` (no VM
yet), `broken`, `host` (Linux) or `unavailable` (no Lima 2; `reason` says what
to install). A `reason` on a running VM means this project is not mounted.

## Run commands

```bash
toolu sandbox run -- cargo test -p toolu-cli
toolu sandbox run --clone -- npm ci --ignore-scripts
toolu sandbox run --clone -- cargo xtask gate
toolu sandbox shell
```

- Paths are the same as on the host. `CARGO_TARGET_DIR` and `TMPDIR` default
  to the VM's own disk, because the shared mount is slow for many small files.
- `--clone` runs in a clone of this checkout on the VM's disk, with staged,
  unstaged and untracked changes and `origin/*`. Use it for the gate, for
  `npm ci` (never run `npm ci` without it: it would put Linux binaries in the
  host's `node_modules`) and for host-smoke. Nothing flows back to the
  checkout; the clone keeps its `target/` between runs. `toolu sandbox prune`
  lists the clones of checkouts that no longer exist, and `--apply` removes
  them with their caches (`toolu gc --apply` does this too).
- For a faster incremental link, opt into mold:
  `toolu sandbox run -- env RUSTFLAGS=-Clink-arg=-fuse-ld=mold cargo build`.

Host smoke runs the binary it builds in the clone:

```bash
toolu sandbox run --clone -- cargo build -p toolu-cli
toolu sandbox run --clone -- cargo xtask host-smoke --host codex
```

## What stays with the user

- `toolu sandbox setup` provisions the VM. It may outlast a tool call: ask the
  user to run it, or run it in the background. Logs are under
  `~/.local/state/toolu/sandbox/`.
- Agent CLI installs and logins. `setup` prints each vendor's install and login
  command; the user runs them inside `toolu sandbox shell`. Logins live in the
  VM, and deleting the VM removes them.
- Restarting a running VM. A new worktree that is not mounted needs
  `toolu sandbox start --restart`, which stops every command running in the VM;
  ask first. Mounting a common parent once avoids it:
  `toolu config set sandbox.mounts '["~/.herdr/worktrees"]'`.
- Under a host agent's own sandbox, `toolu sandbox` needs permission to reach
  Lima (`~/.lima` and the Virtualization framework).

To look at the VM directly:

```bash
limactl list
```

## Exit codes

| Code | Meaning |
| --- | --- |
| command's own | `run` and `shell` exit with the command's status |
| 1 | a step failed (the message names its log) or the config is invalid |
| 64 | usage error, `--json` on `run` or `shell`, `--clone` outside git, or a project directory that holds your home directory |
| 69 | no Lima 2, no VM yet, setup never finished, this project not mounted, or the command could not start |
| 75 | another sandbox command is changing the VM; retry |
