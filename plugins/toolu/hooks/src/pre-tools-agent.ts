/**
 * PreToolUse for subagent spawns (#258): runs `pre-tools/agent-tier.sh`
 * exactly as hooks.json used to, until #262 ports it.
 */
import { join } from "node:path";
import { runScript } from "@toolu/core/dispatch";
import { hookMain } from "./pre-tools/hook-main.ts";

await hookMain(import.meta.dir, (stdin, hooks) =>
  runScript(join(hooks, "pre-tools", "agent-tier.sh"), stdin),
);
