/** Bash bridge protocol v1 (#210). */
import { z } from "zod";
import { type Decision, DecisionSchema } from "../decision/decision.ts";
import { BridgeEventSchema } from "../events/events.ts";
import { createBunBashRunner, type BashRunner, type RawProcessResult } from "../runner/runner.ts";
import { decisionFromHookResult, decisionFromRunnerFailure, toHookStdin } from "./hook-map.ts";

export const BridgeRequestSchema = z.object({
  protocolVersion: z.literal(1),
  event: BridgeEventSchema,
  sessionId: z.string().min(1),
  toolCallId: z.string().min(1),
  cwd: z.string().min(1),
  projectRoot: z.string().min(1),
  worktree: z.string().min(1),
  toolName: z.string().min(1),
  toolInput: z.record(z.string(), z.unknown()).default({}),
  host: z.string().min(1),
  deadlineMs: z.number().int().positive(),
  maxStdoutBytes: z.number().int().positive(),
});

export type BridgeRequest = z.infer<typeof BridgeRequestSchema>;

const BridgeMetaSchema = z.object({
  exitCode: z.number().int().nullable(),
  stdout: z.string(),
  stderr: z.string(),
  truncated: z.boolean().optional(),
});

export const BridgeResponseSchema = z.discriminatedUnion("ok", [
  z
    .object({
      ok: z.literal(true),
      decision: DecisionSchema,
      meta: BridgeMetaSchema,
    })
    .strict(),
  z
    .object({
      ok: z.literal(false),
      decision: DecisionSchema,
      meta: BridgeMetaSchema,
    })
    .strict(),
]);

export type BridgeResponse = z.infer<typeof BridgeResponseSchema>;

/** Parse a bridge request envelope. */
export function parseBridgeRequest(input: unknown): BridgeRequest {
  return BridgeRequestSchema.parse(input);
}

export { toHookStdin, decisionFromHookResult } from "./hook-map.ts";

function responseFromProcess(event: BridgeRequest["event"], raw: RawProcessResult): BridgeResponse {
  const meta = {
    exitCode: raw.exitCode,
    stdout: raw.stdout,
    stderr: raw.stderr,
    truncated: raw.truncated,
  };

  if (!raw.ok) {
    const decision = decisionFromRunnerFailure(raw.code, raw.message);
    return { ok: false, decision, meta };
  }

  const decision = decisionFromHookResult(raw.exitCode, raw.stdout, raw.stderr, event);
  if (decision.kind === "runtime_failure") {
    return { ok: false, decision, meta };
  }
  return { ok: true, decision, meta };
}

export type PreToolBridgeOptions = {
  repoRoot: string;
  runner?: BashRunner;
  env?: Record<string, string>;
  signal?: AbortSignal;
};

/** Resolve pre-tools/mod.sh under a toolu checkout. */
export function resolvePreToolsModSh(repoRoot: string): string {
  return `${repoRoot.replace(/\/$/, "")}/plugins/toolu/hooks/pre-tools/mod.sh`;
}

/** Run the real pre-tools dispatcher and map to BridgeResponse. */
export async function runPreToolBridge(
  requestInput: unknown,
  opts: PreToolBridgeOptions,
): Promise<BridgeResponse> {
  const request = parseBridgeRequest(requestInput);
  if (request.event !== "tool/pre") {
    const decision: Decision = {
      kind: "runtime_failure",
      reason: `runPreToolBridge supports tool/pre only, got ${request.event}`,
      code: "parse",
    };
    return {
      ok: false,
      decision,
      meta: { exitCode: null, stdout: "", stderr: decision.reason, truncated: false },
    };
  }

  const modSh = resolvePreToolsModSh(opts.repoRoot);
  const runner = opts.runner ?? createBunBashRunner();
  const stdin = toHookStdin({ toolName: request.toolName, toolInput: request.toolInput });

  const env: Record<string, string> = {
    ...opts.env,
    TOOLU_PROJECT_DIR: request.projectRoot,
    CLAUDE_PROJECT_DIR: request.projectRoot,
  };

  const raw = await runner.run({
    argv: ["bash", modSh],
    cwd: request.cwd,
    env,
    stdin,
    deadlineMs: request.deadlineMs,
    maxStdoutBytes: request.maxStdoutBytes,
    ...(opts.signal !== undefined ? { signal: opts.signal } : {}),
  });

  return responseFromProcess(request.event, raw);
}
