#!/usr/bin/env bun
/** Live OpenCode 1.18.34 ast-grep search-nudge and byte-savings smoke for issue #347. */
import { AST_GREP_SCENARIOS } from "./opencode-host/scenarios-ast-grep-smoke.ts";
import { runSmoke } from "./opencode-host/smoke-main.ts";

if (import.meta.main) {
  process.exitCode = await runSmoke("opencode-ast-grep-smoke", AST_GREP_SCENARIOS, process.argv[2]);
}
