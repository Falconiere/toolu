# Debug fixtures — provenance

All files here are **real captured output** from genuine tool runs on 2026-06-18 (no synthetic/mocked data). Temp paths (`/private/var/folders/.../tmp.*`) are the real ephemeral dirs the captures ran in — frozen as part of the transcript.

| File | Source command | What it exercises |
|---|---|---|
| `bun-testfail.txt` | `bun test math.test.ts` on a 2-test file where `add` used `-` instead of `+` (both tests fail) | `debug-testfail.sh` — JS/TS test-failure parse: `(fail) <name>`, `expect` error lines, `file:line` |
| `cargo-testfail.txt` | `cargo test` on a crate whose `add` used `-` instead of `+` | `debug-testfail.sh` — Rust test-failure parse: `test <name> ... FAILED`, `failures:` block, `src/lib.rs:L:C` |
| `stacktrace.txt` | `node throw.mjs` — `JSON.parse` of bad JSON through `outer→middle→inner` | `debug-stack.sh` — JS stack: app frames (`inner`/`middle`/`outer`) vs `node:internal/*` noise |
| `rust-panic.txt` | `RUST_BACKTRACE=1 cargo run` — index-out-of-bounds through `level_one→level_two→level_three` | `debug-stack.sh` — Rust backtrace: app frames (`panicker::*` at `./src/main.rs`) vs `core::panicking`/`__rustc`/`FnOnce` noise |
| `big.log` | `cargo build --verbose` + `bun add --verbose` + `git log -p -n 60` of this repo, trimmed to 3000 lines (~120 KB) | `debug-log.sh` — cap enforcement: input far exceeds line/byte caps |
