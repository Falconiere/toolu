/**
 * Prompt and compaction stdout (#341). Startup stays on `parseStartupOutput`,
 * which accepts only SessionStart. A block decision is a reminder, not a
 * refusal; when both shapes are present, `additionalContext` wins.
 */
import { sessionContext } from "@toolu/core/startup";
import { z } from "zod";

const HOOK_EVENTS = ["SessionStart", "UserPromptSubmit", "PreCompact"] as const;

export type HookContextEvent = (typeof HOOK_EVENTS)[number];

export type HookContextBody =
  | { kind: "context"; additionalContext?: string; systemMessage?: string }
  | { kind: "block"; reason: string };

export type ParsedHookContext =
  | { ok: true; context: HookContextBody }
  | { ok: false; reason: string };

function schema(event: HookContextEvent) {
  return z.strictObject({
    hookSpecificOutput: z
      .strictObject({
        hookEventName: z.literal(event),
        additionalContext: z.string(),
      })
      .optional(),
    systemMessage: z.string().optional(),
    decision: z.literal("block").optional(),
    reason: z.string().optional(),
  });
}

function contextBody(
  event: HookContextEvent,
  text: string,
  systemMessage: string | undefined,
): HookContextBody {
  const body: HookContextBody = { kind: "context" };
  const bounded = sessionContext(event, text)?.hookSpecificOutput.additionalContext;
  if (bounded !== undefined) body.additionalContext = bounded;
  if (systemMessage !== undefined) body.systemMessage = systemMessage;
  return body;
}

/** `stdout` for `event`, or why it is not context this host can deliver. */
export function parseHookContext(stdout: string, event: HookContextEvent): ParsedHookContext {
  if (stdout.trim() === "") return { ok: true, context: { kind: "context" } };
  let raw: unknown;
  try {
    raw = JSON.parse(stdout);
  } catch {
    return { ok: false, reason: "invalid hook output: not JSON" };
  }
  const parsed = schema(event).safeParse(raw);
  if (!parsed.success) {
    return { ok: false, reason: `invalid hook output: ${z.prettifyError(parsed.error)}` };
  }
  const text = parsed.data.hookSpecificOutput?.additionalContext ?? "";
  if (text !== "")
    return { ok: true, context: contextBody(event, text, parsed.data.systemMessage) };
  if (parsed.data.decision === "block") {
    const reason = parsed.data.reason ?? "";
    if (reason === "") return { ok: false, reason: "invalid hook output: block without reason" };
    return { ok: true, context: { kind: "block", reason } };
  }
  return { ok: true, context: contextBody(event, "", parsed.data.systemMessage) };
}
