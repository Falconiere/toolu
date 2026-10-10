/** Native core gate decider, shared by tool.execute.before and the evaluate-shaped handler. */
import { join } from "node:path";
import { existsSync } from "node:fs";
import type { Decision } from "@toolu/core/decision";
import { dispatchPreTool, type ToolModule } from "@toolu/core/dispatch";
import { agentTierHook } from "@toolu/core/gates/agent-tier";
import { mcpHook } from "@toolu/core/gates/mcp-hook";
import {
  bashCommandsModule,
  codeEditRulesModule,
  commitGateModule,
  docsSyncModule,
  mcpBlockerModule,
  planLedgerModule,
  protectedFilesModule,
  pushReviewModule,
  qualityGateModule,
} from "@toolu/core/gates";
import {
  applyDecisionToPermission,
  mapPermissionEventToTool,
  type PermissionContext,
  type PermissionEvaluationEvent,
} from "./permission-map.ts";
import { definedEnv, withoutForeignHostVars } from "../host/runtime-env.ts";
import { decisionFromDispatch } from "./result.ts";
import { astGrepRule } from "./ast-grep-native.ts";

export type PermissionEvaluateHandlerOptions = {
  repoRoot: string;
  configRoot: string;
  permissionContext: PermissionContext;
  env?: Record<string, string>;
  selectedPluginSpecs?: ReadonlySet<string>;
  /** Global `toolu.config.json` directory (#343); absent, the config root holds it, as before. */
  userConfigRoot?: string;
};

export function nativeGates(pluginRoot: string): ToolModule[] {
  const options = { pluginRoot };
  return [
    bashCommandsModule(options),
    codeEditRulesModule(options),
    commitGateModule(options),
    docsSyncModule(options),
    mcpBlockerModule(options),
    planLedgerModule(options),
    protectedFilesModule(options),
    pushReviewModule(options),
    qualityGateModule(options),
  ];
}

function runtimeFailureDeny(event: PermissionEvaluationEvent, reason: string): void {
  applyDecisionToPermission({ kind: "runtime_failure", reason, code: "parse" }, event);
}

export type GateDecider =
  | { ok: true; decide: (request: Record<string, unknown>) => Promise<Decision> }
  | { ok: false; reason: string };

function priority(decision: Decision): number {
  switch (decision.kind) {
    case "runtime_failure":
      return 5;
    case "deny":
    case "post_block":
      return 4;
    case "ask":
      return 3;
    case "advisory":
      return 2;
    case "allow":
      return 1;
  }
  throw new Error("unknown toolu decision");
}

function strongerDecision(first: Decision, second: Decision): Decision {
  return priority(second) > priority(first) ? second : first;
}

/**
 * The env every gate sees: the process env under `opts.env`, minus every other
 * host's root variables (#343: dropped, not overridden), plus toolu's roots.
 */
export function gateEnv(
  opts: PermissionEvaluateHandlerOptions,
  pluginRoot: string,
): Record<string, string> {
  return {
    ...withoutForeignHostVars({ ...definedEnv(process.env), ...opts.env }),
    ...(opts.userConfigRoot === undefined ? {} : { TOOLU_USER_CONFIG_DIR: opts.userConfigRoot }),
    TOOLU_CONFIG_DIR: opts.configRoot,
    TOOLU_PROJECT_DIR: opts.permissionContext.projectRoot,
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_SETTINGS_DIR: join(pluginRoot, "settings"),
  };
}

/** The nine native gates in toolu's PreToolUse order, run in process for one core request. */
export function createGateDecider(opts: PermissionEvaluateHandlerOptions): GateDecider {
  const pluginRoot = join(opts.repoRoot, "plugins", "toolu");
  if (!existsSync(join(pluginRoot, ".claude-plugin", "plugin.json"))) {
    return { ok: false, reason: `toolu: core plugin manifest missing under ${pluginRoot}` };
  }
  const gates = nativeGates(pluginRoot);
  const env = gateEnv(opts, pluginRoot);
  const decide = async (request: Record<string, unknown>): Promise<Decision> => {
    try {
      const payload = JSON.stringify(request);
      if (typeof request.tool_name === "string" && request.tool_name.startsWith("mcp__")) {
        return decisionFromDispatch(await mcpHook(payload, { pluginRoot, env }));
      }
      const result = await dispatchPreTool(JSON.stringify(request), {
        builtins: gates,
        libDir: join(pluginRoot, "hooks", "lib"),
        cwd: opts.permissionContext.cwd,
        env,
        ...(opts.selectedPluginSpecs === undefined
          ? {}
          : { selectedRegistrySpecs: opts.selectedPluginSpecs }),
      });
      const core = decisionFromDispatch(result);
      if (core.kind === "runtime_failure" || core.kind === "deny" || core.kind === "post_block")
        return core;
      const native = decisionFromDispatch(
        await astGrepRule("pre-tools", request, {
          configRoot: opts.configRoot,
          cwd: opts.permissionContext.cwd,
          env,
          selectedPluginSpecs: opts.selectedPluginSpecs,
        }),
      );
      const combined = strongerDecision(core, native);
      if (request.tool_name !== "Task") return combined;
      const tier = decisionFromDispatch(
        agentTierHook(payload, { env, cwd: opts.permissionContext.cwd }),
      );
      return strongerDecision(combined, tier);
    } catch (error) {
      const reason = error instanceof Error ? error.message : String(error);
      return { kind: "runtime_failure", reason: `toolu dispatch: ${reason}`, code: "parse" };
    }
  };
  return { ok: true, decide };
}

/** One permission call, with the same built-in order as toolu's PreToolUse bundle. */
export function createPermissionEvaluateHandler(
  opts: PermissionEvaluateHandlerOptions,
): (event: PermissionEvaluationEvent) => Promise<void> {
  const decider = createGateDecider(opts);
  if (!decider.ok) return createDenyAllPermissionHandler(decider.reason);
  return async (event: PermissionEvaluationEvent): Promise<void> => {
    const mapping = mapPermissionEventToTool(event, opts.permissionContext);
    if (mapping.kind === "skip") return;
    if (mapping.kind === "deny") {
      applyDecisionToPermission({ kind: "deny", reason: mapping.reason }, event);
      return;
    }
    applyDecisionToPermission(await decider.decide(mapping.request), event);
  };
}

/** Fail-closed evaluate handler when bootstrap/preflight is NotReady. */
export function createDenyAllPermissionHandler(
  reason: string,
): (event: PermissionEvaluationEvent) => Promise<void> {
  return (event: PermissionEvaluationEvent): Promise<void> => {
    runtimeFailureDeny(event, reason);
    return Promise.resolve();
  };
}
