/** permission.hook("evaluate") handler backed by runPreToolBridge (#204). */
import { runPreToolBridge } from "@toolu/core/bridge";
import type { BashRunner } from "@toolu/core/runner";
import {
  applyDecisionToPermission,
  mapPermissionEventToBridge,
  type PermissionBridgeContext,
  type PermissionEvaluationEvent,
} from "./permission-map.ts";

export type PermissionEvaluateHandlerOptions = {
  repoRoot: string;
  bridgeContext: PermissionBridgeContext;
  runner?: BashRunner;
  env?: Record<string, string>;
};

function runtimeFailureDeny(event: PermissionEvaluationEvent, reason: string): void {
  applyDecisionToPermission({ kind: "runtime_failure", reason, code: "parse" }, event);
}

/** Handler for OpenCode permission.evaluate — real bash bridge, fail closed on errors. */
export function createPermissionEvaluateHandler(
  opts: PermissionEvaluateHandlerOptions,
): (event: PermissionEvaluationEvent) => Promise<void> {
  return async (event: PermissionEvaluationEvent): Promise<void> => {
    const mapping = mapPermissionEventToBridge(event, opts.bridgeContext);
    if (mapping.kind === "skip") {
      return;
    }
    if (mapping.kind === "deny") {
      applyDecisionToPermission({ kind: "deny", reason: mapping.reason }, event);
      return;
    }

    const response = await runPreToolBridge(mapping.request, {
      repoRoot: opts.repoRoot,
      ...(opts.runner !== undefined ? { runner: opts.runner } : {}),
      ...(opts.env !== undefined ? { env: opts.env } : {}),
    });

    applyDecisionToPermission(response.decision, event);
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
