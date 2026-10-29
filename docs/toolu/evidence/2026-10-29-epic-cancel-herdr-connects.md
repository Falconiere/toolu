# Issue #747: bounded herdr connect evidence

## Workload and method

On 2026-10-29, the test `backlog_connects_release_resources_at_deadline` used a real Unix listener with backlog 1, kept its queue full, and attempted four sequential `http_live::connect` calls. Linux queued two peers; macOS queued one. The test then accepted the queued peers and checked that no connector arrived after its deadline. The baseline used `origin/main`'s connector with only the new test module wired in. The fixed run used this branch's nonblocking connector. Both were compiled and run with `cargo test --release -p toolu-epic-orchestrator backlog_connects_release_resources_at_deadline -- --nocapture` as one isolated test process. No mock socket or ignored test was used.

| Platform | Source | Four attempts | Open descriptors before → after | Threads before → after | Late connection | Result |
| --- | --- | ---: | ---: | ---: | --- | --- |
| Linux aarch64, Lima Debian 13 | baseline | 4.023809985 s | 7 → 11 | 2 → 6 | Accepted after deadline | Failed regression |
| Linux aarch64, Lima Debian 13 | fixed | 4.00415642 s | 7 → 7 | 2 → 2 | None | Passed |
| Darwin arm64 | baseline | 278.417 µs | 6 → 6 | Not measured | None; full backlog refused immediately | Passed platform boundary |
| Darwin arm64 | fixed | 44.625 µs | 6 → 6 | Not measured | None; full backlog refused immediately | Passed |

The Linux baseline grew by one blocked thread and one descriptor for every timed-out attempt. The fixed connector returned with no retained thread or descriptor. Each Linux attempt finished by its one-second deadline; the aggregate time remained about four seconds because the test made four serial attempts. On macOS the kernel refused a full backlog immediately, so these tiny timings are not a meaningful speedup comparison. The same code path passed the real herdr success, missing socket, disconnect, shutdown, stalled reply, and concurrency-cap tests on both platforms.

The Linux VM's running sandbox did not mount this worktree. I transferred the tracked source and new test to a temporary directory in that VM, used `toolu epic job -- limactl shell toolu-sandbox -- ...` for the test runs, and forced a rebuild after copying changed source because the transfer preserved an older timestamp. The final fixed release run and the full focused Linux test suite compiled the changed connector. The macOS baseline ran from a temporary copy and the fixed run from this worktree, with a forced rebuild after the baseline shared the target directory.
