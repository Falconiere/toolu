import { existsSync } from "node:fs";
import { join, resolve } from "node:path";
import { dirname } from "node:path";
import { fileURLToPath } from "node:url";
import type { ExtensionAPI, ExtensionContext } from "@earendil-works/pi-coding-agent";
import {
  mappedToolName,
  parseHookOutput,
  hookText,
  refreshGateStatus,
  runHook,
  runRegistrySync,
  toolCallPayload,
  toolResultPayload,
} from "./toolu-lib.js";

const extensionDir = dirname(fileURLToPath(import.meta.url));
const packageRoot = resolve(extensionDir, "../..");

const preToolsScript = join(packageRoot, "plugins/toolu/hooks/pre-tools/mod.sh");
const postToolsScript = join(packageRoot, "plugins/toolu/hooks/post-tools/mod.sh");
const REGISTRY_SCRIPTS = [
  join(packageRoot, "plugins/ast-grep/hooks/register.sh"),
  join(packageRoot, "plugins/comemory/hooks/register.sh"),
  join(packageRoot, "plugins/ts-quality/hooks/register.sh"),
  join(packageRoot, "plugins/rust-quality/hooks/register.sh"),
];

export type HookOutput = {
  systemMessage?: string;
  hookSpecificOutput?: {
    additionalContext?: string;
    permissionDecision?: string;
    permissionDecisionReason?: string;
  };
};

/**
 * Set CLAUDE_PLUGIN_ROOT to the comemory plugin directory so scripts resolved
 * via `${CLAUDE_PLUGIN_ROOT}/skills/agent-memory/scripts/comemory.sh` in skill
 * instructions work inside pi (pi does not natively set this Claude Code
 * convention). Called once from the extension factory so the side effect is
 * intentional and ordered, not a silent module-level mutation.
 *
 * We target comemory because its agent-memory skill is ALWAYS ACTIVE and
 * carries no repo-checkout fallback path (unlike context7, exa-search, jira).
 */
function initComemoryPluginRoot() {
  const root = join(packageRoot, "plugins/comemory");
  if (existsSync(root)) {
    process.env.CLAUDE_PLUGIN_ROOT = root;
  }
}

/** pi extension entry point: wires toolu pre/post-tool hooks and gate status into the agent. */
export default function tooluPiExtension(pi: ExtensionAPI) {
  initComemoryPluginRoot();

  pi.on("session_start", async (_event, ctx) => {
    await runRegistrySync(ctx.cwd, REGISTRY_SCRIPTS);
    refreshGateStatus(ctx);
  });

  pi.on("session_shutdown", (_event, ctx) => {
    if (ctx.hasUI) {
      ctx.ui.setStatus("toolu-gate", undefined);
    }
  });

  pi.on("tool_call", async (event, ctx) => {
    const name = mappedToolName(event.toolName);
    if (!name || !existsSync(preToolsScript)) return;

    const result = await runHook(preToolsScript, ctx.cwd, toolCallPayload(event), ctx.signal);
    const output = parseHookOutput(result.stdout);

    if (result.code === 2) {
      return { block: true, reason: result.stderr || result.stdout || "Blocked by toolu pre-tool hook" };
    }

    if (output?.hookSpecificOutput?.permissionDecision === "deny") {
      return {
        block: true,
        reason: output.hookSpecificOutput.permissionDecisionReason || "Blocked by toolu pre-tool hook",
      };
    }

    const note = hookText(output);
    if (note && ctx.hasUI) {
      ctx.ui.notify(note, "info");
    }
  });

  pi.on("tool_result", async (event, ctx) => {
    if (!existsSync(postToolsScript)) return;
    if (event.toolName !== "bash" && event.toolName !== "edit" && event.toolName !== "write") {
      return;
    }

    const result = await runHook(postToolsScript, ctx.cwd, toolResultPayload(event), ctx.signal);
    const output = parseHookOutput(result.stdout);
    refreshGateStatus(ctx);

    if (result.code !== 0 && result.stderr) {
      return {
        content: [...event.content, { type: "text" as const, text: `\n[toolu]\n${result.stderr}` }],
      };
    }

    const text = hookText(output);
    if (!text) return { content: event.content };
    return {
      content: [...event.content, { type: "text" as const, text: `\n[toolu]\n${text}` }],
    };
  });
}
