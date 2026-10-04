#!/usr/bin/env bun
/** Live OpenCode 1.18.34 Rust quality smoke for issue #354. */
import { RUST_QUALITY_SCENARIOS } from "./opencode-host/scenarios-rust-quality-smoke.ts";
import { runSmoke } from "./opencode-host/smoke-main.ts";

if (import.meta.main) {
  process.exitCode = await runSmoke(
    "opencode-rust-quality-smoke",
    RUST_QUALITY_SCENARIOS,
    process.argv[2],
  );
}
