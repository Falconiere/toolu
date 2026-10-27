---
name: verify-root-host-gates
description: Verify toolu Rust gates on root Linux hosts with shared leases, native paths, permissions, and durable temp storage.
metadata:
  toolu:
    origin: agent
    created: 2026-10-09T15:50:04Z
---
## When to Use

Use when running toolu's required Rust gates as root on a shared Linux host, especially from an epic worker worktree.

## Procedure

1. Read the worker brief for the shared job lease. Run expensive focused or full checks with `toolu epic job -- <command> <args...>`. A `toolu ledger run ... --verify` acquires the lease for its checks. If admission is refused, wait and retry the same command.
2. Check `df -i /tmp` and the intended temporary directory's filesystem. Create a unique mode-0700 directory under `$HOME/.cache` on a filesystem with free inodes. Set `TMPDIR` to that absolute path on every focused, ledger, and full-gate command. Keep the native executable and pinned Rust toolchain on `PATH` with `PATH="$PWD/target/debug:$HOME/.cargo/bin:$PATH"`.
3. Reproduce a failure with its smallest real test first. For the ledger CLI golden, run `cargo test -p toolu-cli --test ledger_cases -- --nocapture` under the lease and the same `TMPDIR`. Its isolated child environment must forward `TMPDIR`; a child that calls `env_clear()` otherwise falls back to `/tmp`.
   Before a direct native fixture suite, build the exact `target/debug/toolu` executable it launches. `cargo test` compiles test binaries but may leave that CLI executable stale.
4. On a live Herdr pane, remove `HERDR_SOCKET_PATH`, `HERDR_ENV`, `HERDR_PANE_ID`, `HERDR_TAB_ID`, `HERDR_WORKSPACE_ID`, `HERDR_BIN_PATH` and `HERDR_CONFIG_PATH` from the gate process after the lease is acquired. The epic fixtures must use their own socket. If `/usr/local/bin/toolu` exists, launcher tests need a private mount view: `unshare -m -- sh -c 'mount --make-rprivate / && mount --bind "$TMPDIR/empty-local-bin" /usr/local/bin && ...'`, with a task-owned empty directory created first. This masks the installed binary only inside the job.
5. Run the approved plan with `toolu ledger run <plan> --verify` under the same environment. Where a full gate runs directly as root, use `capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c 'cargo xtask gate'` under the lease, inside the isolation above when needed. Let the live process finish and inspect the terminal result and ledger evidence.
6. After any code or documentation edit, rerun the affected focused check and the required final-tree gate. Record a clean review state only for the committed final diff before pushing.

## Pitfalls

- A directory inside `/tmp` still uses `/tmp`'s inodes. `ENOSPC` can mean inode exhaustion even when `df -h` shows free bytes.
- Do not delete another worker's or unproven-owned `/tmp` data. Redirect this worker's temporary files to a separate filesystem.
- A test helper may clear the environment before starting a child process. Forward only the needed `TMPDIR` in that helper; do not weaken a gate or suppress a lint to work around host pressure.
- Preserve the shared lease and root capability drops on retries. A failed environmental run remains failed until the complete final-tree gate passes.
- If a real parser budget test passes alone but times out amid parallel coverage tests on a loaded host, set `NEXTEST_TEST_THREADS=1` for the local full gate (the gate runs tests under nextest, which ignores `RUST_TEST_THREADS`). This runs every test without changing the repository's budget or assertion.
- Run coverage tests through `cargo llvm-cov` or point `LLVM_PROFILE_FILE` outside the repository. Executing its instrumented test binary directly without that variable leaves `default_*.profraw` files at the root, which correctly fail the folder guardrail.
- After acquiring any shared job lease, isolate the gate subprocess's `TOOLU_RESOURCE_HOME` in task-owned scratch. Otherwise CLI contract tests that inherit `HOME` can query the machine's resident epic engine: `epic status --json` then returns live state instead of the offline fixture and can fail schema validation. Confirm with the focused contract test before rerunning the gate; leave the resident engine running.
- When a macOS installation shadows launcher fixtures, use a task-owned Linux container and independent checkout instead of moving the installed binary. Run as a non-root user with `--init` to reap detached fixture children. Compare `git write-tree` in both checkouts before running checks; synchronize the exact commit for final verification.
- Mount the container's target volume outside the checkout, link the ignored `target` directory to it, and place a private `TMPDIR` on that volume. Confirm `git -C "$TMPDIR" rev-parse --show-toplevel` fails and the temp directory and built binary share a filesystem. This keeps outside-repository fixtures valid while allowing hard links instead of cold cross-filesystem binary copies.
- On a constrained shared VM disk, use `CARGO_INCREMENTAL=0` for verification and monitor free space. If storage fills, stop the owned runner and discard only its build cache and incomplete coverage data before rerunning; never prune another worker's volumes.
- Ledger state retains only the end of a failed command's output. Capture its temporary `.run.<pid>.<step>` log while the check runs when full failure diagnostics are needed; the temporary log is removed when the command finishes.

## Verification

Confirm the dedicated filesystem has free inodes, the focused failure reproduces and then passes under the corrected environment, the final committed tree passes all required ledger steps plus the full Rust gate, and `toolu ledger verdict status --json` reports `ready` before push.
