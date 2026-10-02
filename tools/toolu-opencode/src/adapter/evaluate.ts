/** Native core gate decider, shared by tool.execute.before and the evaluate-shaped handler. */
import { join } from "node:path";
import { existsSync } from "node:fs";
import type { Decision } from "@toolu/core/decision";
import { dispatchPreTool, type ToolModule } from "@toolu/core/dispatch";
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
import { decisionFromDispatch } from "./result.ts";

export type PermissionEvaluateHandlerOptions = {
  repoRoot: string;
  configRoot: string;
  permissionContext: PermissionContext;
  env?: Record<string, string>;
};

function nativeGates(pluginRoot: string): ToolModule[] {
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

/** The nine native gates in toolu's PreToolUse order, run in process for one core request. */
export function createGateDecider(opts: PermissionEvaluateHandlerOptions): GateDecider {
  const pluginRoot = join(opts.repoRoot, "plugins", "toolu");
  if (!existsSync(join(pluginRoot, ".claude-plugin", "plugin.json"))) {
    return { ok: false, reason: `toolu: core plugin manifest missing under ${pluginRoot}` };
  }
  const gates = nativeGates(pluginRoot);
  const env = {
    ...process.env,
    ...opts.env,
    TOOLU_CONFIG_DIR: opts.configRoot,
    TOOLU_PROJECT_DIR: opts.permissionContext.projectRoot,
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_SETTINGS_DIR: join(pluginRoot, "settings"),
  };
  const decide = async (request: Record<string, unknown>): Promise<Decision> => {
    try {
      const result = await dispatchPreTool(JSON.stringify(request), {
        builtins: gates,
        libDir: join(pluginRoot, "hooks", "lib"),
        cwd: opts.permissionContext.cwd,
        env,
      });
      return decisionFromDispatch(result);
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
