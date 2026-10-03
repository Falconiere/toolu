#!/usr/bin/env bun
/** Live OpenCode 1.18.34 TypeScript quality smoke for issue #352. */
import { TS_QUALITY_SCENARIOS } from "./opencode-host/scenarios-ts-quality-smoke.ts";
import { runSmoke } from "./opencode-host/smoke-main.ts";

if (import.meta.main) {
  process.exitCode = await runSmoke(
    "opencode-ts-quality-smoke",
    TS_QUALITY_SCENARIOS,
    process.argv[2],
  );
}
