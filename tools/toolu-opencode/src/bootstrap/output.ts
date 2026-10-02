/**
 * A startup entry's stdout (#342): nothing, or the SessionStart hook JSON that
 * Claude Code and Codex accept. Context is bounded the way those hosts bound
 * it; any other output is a reason, not context.
 */
import { sessionContext } from "@toolu/core/startup";
import { z } from "zod";

const StartupOutput = z.strictObject({
  hookSpecificOutput: z
    .strictObject({ hookEventName: z.literal("SessionStart"), additionalContext: z.string() })
    .optional(),
  systemMessage: z.string().optional(),
});

export type StartupContext = { additionalContext?: string; systemMessage?: string };

export type ParsedOutput = { ok: true; context: StartupContext } | { ok: false; reason: string };

export function parseStartupOutput(stdout: string): ParsedOutput {
  if (stdout.trim() === "") return { ok: true, context: {} };
  let raw: unknown;
  try {
    raw = JSON.parse(stdout);
  } catch {
    return { ok: false, reason: "invalid startup output: not JSON" };
  }
  const parsed = StartupOutput.safeParse(raw);
  if (!parsed.success) {
    return { ok: false, reason: `invalid startup output: ${z.prettifyError(parsed.error)}` };
  }
  const context: StartupContext = {};
  const text = parsed.data.hookSpecificOutput?.additionalContext ?? "";
  const bounded = sessionContext("SessionStart", text)?.hookSpecificOutput.additionalContext;
  if (bounded !== undefined) context.additionalContext = bounded;
  if (parsed.data.systemMessage !== undefined) context.systemMessage = parsed.data.systemMessage;
  return { ok: true, context };
}
