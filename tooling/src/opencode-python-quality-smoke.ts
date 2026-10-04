#!/usr/bin/env bun
/** Live OpenCode 1.18.34 Python quality smoke for issue #353. */
import { PYTHON_QUALITY_SCENARIOS } from "./opencode-host/scenarios-python-quality-smoke.ts";
import { runSmoke } from "./opencode-host/smoke-main.ts";

if (import.meta.main) {
  process.exitCode = await runSmoke(
    "opencode-python-quality-smoke",
    PYTHON_QUALITY_SCENARIOS,
    process.argv[2],
  );
}
