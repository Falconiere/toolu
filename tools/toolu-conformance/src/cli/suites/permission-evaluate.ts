import { createPermissionEvaluateHandler } from "@toolu/opencode/adapter/evaluate";
import type { PermissionEvaluationEvent } from "@toolu/opencode/adapter/permission-map";
import type { SuiteOutcome } from "../types.ts";
import { bridgeEnvOpencode, createProtectedProject, repoRoot } from "./helpers.ts";

function editEvent(envPath: string): PermissionEvaluationEvent {
  return {
    sessionID: "sess_conf_eval_1",
    action: "edit",
    resources: [envPath],
    effect: "allow",
    metadata: { toolCallId: "call_conf_eval_1" },
  };
}

/** permission.evaluate + block mode on protected .env → deny (#204 / #212). */
export async function runPermissionEvaluateSuite(): Promise<SuiteOutcome> {
  const root = repoRoot();
  const { projectRoot, envPath } = createProtectedProject("toolu-conformance-pe-", ".opencode");

  const handler = createPermissionEvaluateHandler({
    repoRoot: root,
    bridgeContext: {
      cwd: projectRoot,
      projectRoot,
      worktree: projectRoot,
      host: "opencode",
    },
    env: bridgeEnvOpencode(root),
  });

  const event = editEvent(envPath);
  try {
    await handler(event);
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { status: "fail", message: `permission evaluate threw: ${message}` };
  }

  if (event.effect === "deny") {
    return { status: "pass" };
  }

  return {
    status: "fail",
    message: `expected deny on protected .env edit, got effect=${event.effect}`,
  };
}
