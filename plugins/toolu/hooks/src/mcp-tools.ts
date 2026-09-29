/**
 * PreToolUse for `mcp__*` tools (#260): the native mcp-blocker alone, as the
 * standalone `mcp__` hook ran `mcp-blocker.sh`. MCP calls do not walk the
 * other built-ins or the `pre-tools.d` registry.
 */
import { dirname } from "node:path";
import { mcpHook } from "@toolu/core/gates/mcp-hook";
import { hookMain } from "./pre-tools/hook-main.ts";

await hookMain(import.meta.dir, (stdin, hooks) => mcpHook(stdin, { pluginRoot: dirname(hooks) }));
