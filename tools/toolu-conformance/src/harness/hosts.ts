/**
 * Per-host hook output contracts (#251). Each host speaks its own JSON; the
 * harness validates it against that host's documented schema and normalises it
 * to one Outcome so a port's test can assert the same effect on every host.
 *
 * Claude Code and Codex share `hookSpecificOutput`, but Codex cannot prompt:
 * `permissionDecision: "ask"` is a contract violation there. Cursor answers
 * permission events with `{permission, user_message, agent_message}` and blocks
 * on an invalid response. OpenCode runs in process and mutates the effect of
 * its permission event.
 */
import { z } from "zod";
import type { RunResult } from "./spawn.ts";

export type Effect = "allow" | "deny" | "ask";
export type Outcome = { effect: Effect; reason?: string; context?: string };
export type CommandHost = "claude" | "codex" | "cursor";

export class HostOutputError extends Error {
  override name = "HostOutputError";
}

function hookOutputSchema(decisions: readonly [Effect, ...Effect[]]) {
  return z.looseObject({
    hookSpecificOutput: z
      .looseObject({
        hookEventName: z.string().optional(),
        permissionDecision: z.enum(decisions).optional(),
        permissionDecisionReason: z.string().optional(),
        additionalContext: z.string().optional(),
      })
      .optional(),
    decision: z.enum(["approve", "block"]).optional(),
    reason: z.string().optional(),
    systemMessage: z.string().optional(),
    continue: z.boolean().optional(),
  });
}

export const ClaudeOutputSchema = hookOutputSchema(["allow", "deny", "ask"]);
export const CodexOutputSchema = hookOutputSchema(["allow", "deny"]);
export const CursorPermissionSchema = z.looseObject({
  permission: z.enum(["allow", "deny", "ask"]),
  user_message: z.string().optional(),
  agent_message: z.string().optional(),
  updated_input: z.unknown().optional(),
  continue: z.boolean().optional(),
});
export const OpencodeEffectSchema = z.looseObject({
  effect: z.enum(["allow", "deny", "ask"]),
  message: z.string().optional(),
});

type HookOutput = z.infer<typeof ClaudeOutputSchema>;

function parseJson(host: string, stdout: string): unknown {
  try {
    const parsed: unknown = JSON.parse(stdout);
    return parsed;
  } catch {
    throw new HostOutputError(`${host}: hook stdout is not JSON: ${stdout.slice(0, 200)}`);
  }
}

function validate<T>(host: string, schema: z.ZodType<T>, value: unknown, stdout: string): T {
  const parsed = schema.safeParse(value);
  if (!parsed.success) {
    throw new HostOutputError(
      `${host}: output violates the host contract (${parsed.error.message}): ${stdout}`,
    );
  }
  return parsed.data;
}

function withText(outcome: Outcome, key: "reason" | "context", text: string | undefined): Outcome {
  const trimmed = text?.trim();
  return trimmed ? { ...outcome, [key]: trimmed } : outcome;
}

function hookOutcome(host: string, event: string, out: HookOutput): Outcome {
  const hso = out.hookSpecificOutput;
  if (hso?.hookEventName !== undefined && hso.hookEventName !== event) {
    throw new HostOutputError(
      `${host}: hookEventName ${hso.hookEventName} does not match event ${event}`,
    );
  }
  if (hso?.permissionDecision === "deny" || hso?.permissionDecision === "ask") {
    return withText({ effect: hso.permissionDecision }, "reason", hso.permissionDecisionReason);
  }
  if (out.decision === "block") {
    return withText({ effect: "deny" }, "reason", out.reason);
  }
  const context = [hso?.additionalContext, out.systemMessage]
    .map((part) => part?.trim() ?? "")
    .filter((part) => part.length > 0)
    .join("\n\n");
  return withText({ effect: "allow" }, "context", context);
}

function cursorOutcome(stdout: string): Outcome {
  if (stdout.trim() === "") {
    throw new HostOutputError("cursor: empty stdout (Cursor blocks on an invalid response)");
  }
  const out = validate("cursor", CursorPermissionSchema, parseJson("cursor", stdout), stdout);
  return withText({ effect: out.permission }, "reason", out.agent_message ?? out.user_message);
}

/** Normalise a spawned hook's result under `host`'s contract for native `event`. */
export function readHostOutcome(host: CommandHost, event: string, result: RunResult): Outcome {
  if (result.exitCode === 2) {
    return withText({ effect: "deny" }, "reason", result.stderr);
  }
  if (result.exitCode !== 0) {
    throw new HostOutputError(`${host}: hook exited ${result.exitCode}: ${result.stderr.trim()}`);
  }
  if (host === "cursor") {
    return cursorOutcome(result.stdout);
  }
  if (result.stdout.trim() === "") {
    return { effect: "allow" };
  }
  const schema = host === "codex" ? CodexOutputSchema : ClaudeOutputSchema;
  const out = validate(host, schema, parseJson(host, result.stdout), result.stdout);
  return hookOutcome(host, event, out);
}

/** Normalise an OpenCode permission event after the in-process hook evaluated it. */
export function readOpencodeOutcome(event: { effect: unknown; message?: unknown }): Outcome {
  const out = validate("opencode", OpencodeEffectSchema, event, JSON.stringify(event));
  return withText({ effect: out.effect }, "reason", out.message);
}
