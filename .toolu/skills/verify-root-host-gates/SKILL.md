---
name: verify-root-host-gates
description: Verify toolu Rust and Bun gates on root Linux hosts with shared leases, native paths, permissions, and durable temp storage.
metadata:
  toolu:
    origin: agent
    created: 2026-10-09T15:50:04Z
---
## When to Use

Use when running toolu's required Rust and Bun gates as root on a shared Linux host, especially from an epic worker worktree.

## Procedure

1. Read the worker brief for the shared job lease. Run expensive focused or full checks through its `job.ts` wrapper; under Bun the command needs `-- --`. A `toolu ledger run ... --verify` acquires the lease for its checks. If admission is refused, wait and retry the same command.
2. Check `df -i /tmp` and the intended temporary directory's filesystem. Create a unique mode-0700 directory under `$HOME/.cache` on a filesystem with free inodes. Set `TMPDIR` to that absolute path on every focused, ledger, and full-gate command. Keep the native executable and pinned Rust toolchain on `PATH` with `PATH="$PWD/target/debug:$HOME/.cargo/bin:$PATH"`.
3. Reproduce a failure with its smallest real test first. For the ledger CLI golden, run `cargo test -p toolu-cli --test ledger_cases -- --nocapture` under the lease and the same `TMPDIR`. Its isolated child environment must forward `TMPDIR`; a child that calls `env_clear()` otherwise falls back to `/tmp`.
4. Run the approved plan with `toolu ledger run <plan> --verify` under the same environment. Where a full Bun gate runs directly as root, use `capsh --drop=cap_dac_override,cap_dac_read_search,cap_sys_ptrace -- -c 'bun run test'` under the lease. Let the live process finish and inspect the terminal result and ledger evidence.
5. After any code or documentation edit, rerun the affected focused check and the required final-tree gate. Record a clean review state only for the committed final diff before pushing.

## Pitfalls

- A directory inside `/tmp` still uses `/tmp`'s inodes. `ENOSPC` can mean inode exhaustion even when `df -h` shows free bytes.
- Do not delete another worker's or unproven-owned `/tmp` data. Redirect this worker's temporary files to a separate filesystem.
- A test helper may clear the environment before starting a child process. Forward only the needed `TMPDIR` in that helper; do not weaken a gate or suppress a lint to work around host pressure.
- Preserve the shared lease and root capability drops on retries. A failed environmental run remains failed until the complete final-tree gate passes.

## Verification

Confirm the dedicated filesystem has free inodes, the focused failure reproduces and then passes under the corrected environment, the final committed tree passes all required ledger steps plus full Rust and Bun gates, and `toolu ledger verdict status --json` reports `ready` before push.
