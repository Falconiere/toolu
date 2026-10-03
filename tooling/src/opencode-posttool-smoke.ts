#!/usr/bin/env bun
/** Live OpenCode 1.18.34 post-tool dispatch smoke for issue #340. */
import { POSTTOOL_SCENARIOS } from "./opencode-host/scenarios-posttool-smoke.ts";
import { runSmoke } from "./opencode-host/smoke-main.ts";

if (import.meta.main) {
  process.exitCode = await runSmoke("opencode-posttool-smoke", POSTTOOL_SCENARIOS, process.argv[2]);
}
