# #440 evidence: native toolu with Bun hidden

**Date:** 2026-10-14   **Machine:** macOS 27.0.1 arm64   **Binary:** toolu 7.11.0   **Commit:** 26c34bc2 + the npm smoke-project fix

## Live host smoke (AC-9)

Command: `cargo xtask host-smoke --bin target/debug/toolu --host both`. Every child PATH had each directory holding `bun` removed and `TOOLU_BUN` unset. Real Claude Code (haiku) and Codex sessions each ran a protected `.env` write (denied, file unchanged), an allowed `pwd` control, a TypeScript edit over the 3-line limit (quality gate file recorded failing), `git commit` (denied by the quality gate, HEAD unchanged), and `git push origin HEAD:refs/heads/feat/smoke` from a feature branch (denied by push review, the bare remote has no `feat/smoke`).

First run: 4 findings. The smoke project shipped `bun.lock`, so ts-quality, which runs only when the project's package manager is on PATH, stayed silent with Bun hidden. The project now uses `package-lock.json`. Second run:

```
claude: native deny, post-edit quality state, commit gate and push review passed
codex: native deny, post-edit quality state, commit gate and push review passed
host-smoke: ok
exit=0
```

## Hook resources (AC-7)

macOS arm64, release binary, `cargo xtask bench-hooks --out benchmarks/results/hook-resources-rust-darwin-arm64-2026-10-14.json` (report only; Linux CI asserts the budgets). The retired Bun baselines are in `benchmarks/results/hook-resources-bun-*-2026-10-05.json`: pre-tools 37.2 MiB / 57 ms and post-tools 45.1 MiB / 120 ms on macOS CI, against 4.0 MiB / 5.9 ms and 3.2 MiB / 3.4 ms native here.
