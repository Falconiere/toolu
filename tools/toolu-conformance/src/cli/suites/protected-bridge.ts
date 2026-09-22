import { readFileSync } from "node:fs";
import { join } from "node:path";
import { runPreToolBridge } from "@toolu/core/bridge";
import type { SuiteOutcome } from "../types.ts";
import { bridgeEnvClaude, repoRoot } from "./helpers.ts";

export type ProtectedBridgeContext = {
  projectRoot: string;
  envPath: string;
  envBefore?: string;
  failPrefix: string;
};

/** Claude pre-tool bridge on protected .env — deny|ask; optional byte check on deny. */
export async function runProtectedEditBridge(ctx: ProtectedBridgeContext): Promise<SuiteOutcome> {
  const root = repoRoot();
  const fixturePath = join(root, "tooling/fixtures/portable-core/protected-files-pre.json");
  let fixtureRaw: unknown;
  try {
    fixtureRaw = JSON.parse(readFileSync(fixturePath, "utf8"));
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { status: "fail", message: `${ctx.failPrefix}: fixture read failed: ${message}` };
  }

  const request = {
    ...(typeof fixtureRaw === "object" && fixtureRaw !== null ? fixtureRaw : {}),
    cwd: ctx.projectRoot,
    projectRoot: ctx.projectRoot,
    worktree: ctx.projectRoot,
    toolInput: { file_path: ctx.envPath },
  };

  let response;
  try {
    response = await runPreToolBridge(request, {
      repoRoot: root,
      env: bridgeEnvClaude(root),
    });
  } catch (err) {
    const message = err instanceof Error ? err.message : String(err);
    return { status: "fail", message: `${ctx.failPrefix}: bridge threw: ${message}` };
  }

  if (!response.ok) {
    return {
      status: "fail",
      message: `${ctx.failPrefix}: bridge failed: ${response.decision.kind}`,
    };
  }

  const kind = response.decision.kind;
  if (kind === "deny") {
    if (ctx.envBefore !== undefined) {
      const after = readFileSync(ctx.envPath, "utf8");
      if (after !== ctx.envBefore) {
        return {
          status: "fail",
          message: `${ctx.failPrefix}: deny path must not mutate protected .env bytes`,
        };
      }
    }
    return { status: "pass" };
  }

  if (kind === "ask") {
    return { status: "pass" };
  }

  return {
    status: "fail",
    message: `${ctx.failPrefix}: expected deny or ask for protected .env edit, got ${kind}`,
  };
}
