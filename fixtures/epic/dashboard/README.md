# Dashboard status capture

`status.json` contains unmodified responses from two running in-process epic engines: a local registered epic and a reachable peer. The capture uses real HTTP routes, graph files, a worker report, and this branch's plan ledger. It is produced by `dashboard_status_capture_comes_from_running_engines` with `TOOLU_CAPTURE_DASHBOARD=1 cargo test -p toolu-epic-orchestrator dashboard_status_capture`. The temporary peer port and sampled times are capture facts; tests do not assume they remain reachable.
