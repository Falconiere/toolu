---
name: verify-sandbox-vm
description: Verify toolu sandbox's Lima backend on a real Mac — setup, mounts, guest toolchain, exec passthrough, clones and the gate in the VM.
metadata:
  toolu:
    origin: agent
    created: 2026-10-27T20:30:00Z
---
## When to Use

Use after changing `crates/sandbox` (the Lima backend, the generated Lima file,
the provisioning script or the clone script), or `toolu_runtime::process::exec`.
The unit and black-box tests drive a stub `limactl`; only this procedure proves
the real VM.

## Procedure

1. Build the checkout binary: `cargo build -p toolu-cli`. Use `target/debug/toolu`
   below. Lima 2 must be installed (`limactl --version`).
2. `target/debug/toolu sandbox status`: `absent` before the first setup.
3. Run `target/debug/toolu sandbox setup` in the background; follow
   `~/.local/state/toolu/sandbox/toolu-sandbox-start.log` and `-setup.log`. On an
   M2 Max with the Debian image cached it took 180 s. Check the generated
   `toolu-sandbox.yaml`: `base` is `template:_images/debian-13`, and `mounts` are
   only the worktree and, for a linked worktree, the main checkout's `.git`.
4. Guest toolchain: `target/debug/toolu sandbox run -- sh -c 'rustup show
   active-toolchain; cargo nextest --version; node --version; ast-grep --version;
   bwrap --version; toolu --version; printenv CARGO_TARGET_DIR TMPDIR'`. Expect the
   pinned toolchain from `rust-toolchain.toml` and both variables under
   `/var/cache/toolu-sandbox`.
5. Passthrough: `echo x | target/debug/toolu sandbox run -- sh -c 'cat; exit 42'`
   prints `x` and exits 42; `sandbox shell` gets a TTY.
6. Isolation: `sandbox run -- mount` lists only the planned virtiofs mounts, and
   the host home shows nothing else. `sandbox run -- git status` works from a herdr
   worktree.
7. The gate: `target/debug/toolu sandbox run --clone -- sh -c 'npm ci
   --ignore-scripts && cargo xtask gate'`. A rerun reuses the clone's `target/`.
8. Lifecycle: `sandbox stop`, then a `run` boots it (about 10 s warm); `start
   --restart` after adding a `sandbox.mounts` entry remounts.
9. Host smoke needs the user's CLI logins inside `sandbox shell` (the commands
   `setup` printed); then `sandbox run --clone -- cargo xtask host-smoke --host
   <host>` after `sandbox run --clone -- cargo build -p toolu-cli`.
10. Leave the VM running for the user or `sandbox delete --yes`; deleting removes
    the CLI logins in it.

## Pitfalls

- Lima mounts the path as written. `/var/folders/...` on macOS is a symlink to
  `/private/var/...`; the sandbox canonicalises every mount and working directory,
  so the guest path always exists. Keep it that way.
- `template:debian-13` merges a read-only mount of the whole home directory;
  `template:_images/debian-13` does not. Never base the VM on the former.
- The shared mount is about 35x slower than the VM disk for small-file writes.
  Build output, `TMPDIR` and `node_modules` belong on the VM disk; run the gate
  and `npm ci` with `--clone`.
- Check free space on both the guest and the host volume holding Lima's instance.
  Coverage grows its sparse disk: a full Mac volume caused guest commands to fail
  with I/O errors despite 81 GiB free inside Linux. To relocate a task-owned VM,
  cancel queued sandbox runs, stop it, and verify its disk handles are closed;
  queued runs can otherwise boot it during the copy. Copy allocated data extents
  to a spacious host volume, sync the copy and verify the source stayed unchanged
  before replacing the instance's disk with a symlink. Boot and rerun the failed
  check. Copying data/hole extents avoids reading the disk's entire logical size.
- On the Mac, the first run of a freshly written script can stall for tens of
  seconds while macOS scans it. Tests with stub executables may take that long
  locally; it is the reason this sandbox exists, not a test bug.
- The hub's `doctor::muse` tests and the xtask launcher tests fail on a Mac with a
  Homebrew `toolu` in `/opt/homebrew/bin` or a slow first exec; run them in the VM.
- `run` needs the stamp a finished `setup` writes (`~/.local/state/toolu/sandbox/<name>.json`); a failed setup clears it, and the next `run` says to run `setup`.
- Lima parses `--mount-only` as CSV: a comma in a path splits it into two mounts unless the value is quoted (`lima_argv::csv`). Spaces and colons are fine.
- A project directory that holds `$HOME` is refused; the `sandbox.mounts` list in your user config is the way to allow it.
- Codex and Muse need `bwrap` in the guest; the provisioning installs it. Without
  it Codex refuses to sandbox and host-smoke stops early.
