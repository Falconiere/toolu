/** Translate core dispatcher output into an OpenCode permission decision. */
import type { Decision } from "@toolu/core/decision";
import type { ModuleResult } from "@toolu/core/dispatch";
import { z } from "zod";

const RecordSchema = z.record(z.string(), z.unknown());

function object(value: unknown): Record<string, unknown> | undefined {
  const parsed = RecordSchema.safeParse(value);
  return parsed.success ? parsed.data : undefined;
}

function string(value: unknown): string | undefined {
  return typeof value === "string" ? value : undefined;
}

export function decisionFromDispatch(result: ModuleResult): Decision {
  if (result.exitCode === 2) {
    return { kind: "deny", reason: result.stderr.trim() || "toolu blocked this action" };
  }
  if (result.exitCode !== 0) {
    return {
      kind: "runtime_failure",
      reason: result.stderr.trim() || `toolu dispatcher exited ${result.exitCode}`,
      code: "nonzero",
    };
  }
  if (result.stdout.trim() === "") return { kind: "allow" };
  let parsed: unknown;
  try {
    parsed = JSON.parse(result.stdout);
  } catch {
    return {
      kind: "runtime_failure",
      reason: "toolu dispatcher returned invalid JSON",
      code: "parse",
    };
  }
  const doc = object(parsed);
  if (!doc) {
    return {
      kind: "runtime_failure",
      reason: "toolu dispatcher returned invalid output",
      code: "parse",
    };
  }
  const hook = object(doc.hookSpecificOutput);
  const permission = string(hook?.permissionDecision);
  const reason = string(hook?.permissionDecisionReason)?.trim();
  if (permission === "deny")
    return { kind: "deny", reason: reason?.length ? reason : "permission denied" };
  if (permission === "ask")
    return { kind: "ask", reason: reason?.length ? reason : "permission ask" };
  if (permission !== undefined) {
    return {
      kind: "runtime_failure",
      reason: `toolu dispatcher returned unknown decision: ${permission}`,
      code: "parse",
    };
  }
  const messages = [string(hook?.additionalContext), string(doc.systemMessage)].filter(
    (part): part is string => part !== undefined && part.trim() !== "",
  );
  if (messages.length > 0) return { kind: "advisory", message: messages.join("\n\n") };
  return {
    kind: "runtime_failure",
    reason: "toolu dispatcher returned an empty decision object",
    code: "parse",
  };
}
