/**
 * PreToolUse for MCP tools (#258): runs `modules/mcp-blocker.sh` exactly as
 * hooks.json used to, until #260 ports it.
 */
import { join } from "node:path";
import { runScript } from "@toolu/core/dispatch";
import { hookMain } from "./pre-tools/hook-main.ts";

await hookMain(import.meta.dir, (stdin, hooks) =>
  runScript(join(hooks, "pre-tools", "modules", "mcp-blocker.sh"), stdin),
);
