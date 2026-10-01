import { expect, test } from "bun:test";
import { gateDecision } from "@toolu/core/config";
import { encodeDecision } from "@toolu/core/host";
import {
  HostOutputError,
  readHermesOutcome,
  readHostOutcome,
  readOpencodeOutcome,
} from "../hosts.ts";
import { run, type RunResult } from "../spawn.ts";

/** The native gate decision encoded for Claude's PreToolUse contract. */
function gateEmit(mode: "block" | "ask" | "advise" | "off"): RunResult {
  const decision = gateDecision(mode, "why it matters");
  if (decision === null) return result("");
  const encoded = encodeDecision("claude", "tool/pre", decision);
  if (encoded.kind !== "command") throw new Error("expected command output");
  return result(encoded.stdout, encoded.exitCode, encoded.stderr);
}

function result(stdout: string, exitCode = 0, stderr = ""): RunResult {
  return { exitCode, stdout, stderr, durationMs: 1, timedOut: false };
}

test.concurrent("claude: native gate output maps block/ask/advise/off to deny/ask/allow", () => {
  expect(readHostOutcome("claude", "PreToolUse", gateEmit("block"))).toEqual({
    effect: "deny",
    reason: "why it matters",
  });
  expect(readHostOutcome("claude", "PreToolUse", gateEmit("ask"))).toEqual({
    effect: "ask",
    reason: "why it matters",
  });
  expect(readHostOutcome("claude", "PreToolUse", gateEmit("advise"))).toEqual({
    effect: "allow",
    context: "why it matters",
  });
  expect(readHostOutcome("claude", "PreToolUse", gateEmit("off"))).toEqual({
    effect: "allow",
  });
});

test.concurrent("codex: accepts deny and rejects ask, which Codex cannot prompt for", () => {
  expect(readHostOutcome("codex", "PreToolUse", gateEmit("block")).effect).toBe("deny");
  const ask = gateEmit("ask");
  expect(() => readHostOutcome("codex", "PreToolUse", ask)).toThrow(HostOutputError);
});

test.concurrent("exit 2 is a deny carrying stderr on every command host", async () => {
  const blocked = await run(["sh", "-c", "echo 'blocked by hook' >&2; exit 2"]);
  for (const host of ["claude", "codex", "cursor"] as const) {
    expect(readHostOutcome(host, "PreToolUse", blocked)).toEqual({
      effect: "deny",
      reason: "blocked by hook",
    });
  }
});

test.concurrent("claude and codex read the post-tool and legacy block shapes as deny", () => {
  const block = result(JSON.stringify({ decision: "block", reason: "lint failed" }));
  expect(readHostOutcome("claude", "PostToolUse", block)).toEqual({
    effect: "deny",
    reason: "lint failed",
  });
  expect(readHostOutcome("codex", "PreToolUse", block)).toEqual({
    effect: "deny",
    reason: "lint failed",
  });
});

test.concurrent("systemMessage and additionalContext both surface as context", () => {
  const out = result(
    JSON.stringify({
      systemMessage: "note",
      hookSpecificOutput: { hookEventName: "SessionStart", additionalContext: "ctx" },
    }),
  );
  expect(readHostOutcome("claude", "SessionStart", out)).toEqual({
    effect: "allow",
    context: "ctx\n\nnote",
  });
});

test.concurrent("a hookEventName that disagrees with the event is a contract violation", () => {
  const out = gateEmit("block");
  expect(() => readHostOutcome("claude", "PostToolUse", out)).toThrow("hookEventName");
});

test.concurrent("non-JSON stdout and a nonzero non-2 exit are contract violations", () => {
  expect(() => readHostOutcome("claude", "PreToolUse", result("not json"))).toThrow(
    HostOutputError,
  );
  expect(() => readHostOutcome("claude", "PreToolUse", result("", 1, "crash"))).toThrow("exited 1");
  expect(() => readHostOutcome("claude", "PreToolUse", result("[1]"))).toThrow(HostOutputError);
});

test.concurrent("cursor: permission objects map to effects and empty stdout is rejected", () => {
  const deny = result(
    JSON.stringify({ permission: "deny", user_message: "u", agent_message: "a" }),
  );
  expect(readHostOutcome("cursor", "beforeShellExecution", deny)).toEqual({
    effect: "deny",
    reason: "a",
  });
  const ask = result(JSON.stringify({ continue: true, permission: "ask", user_message: "u" }));
  expect(readHostOutcome("cursor", "beforeMCPExecution", ask)).toEqual({
    effect: "ask",
    reason: "u",
  });
  const allow = result(JSON.stringify({ permission: "allow" }));
  expect(readHostOutcome("cursor", "preToolUse", allow)).toEqual({ effect: "allow" });
  expect(() => readHostOutcome("cursor", "preToolUse", result(""))).toThrow(HostOutputError);
  expect(() => readHostOutcome("cursor", "preToolUse", result('{"permission":"maybe"}'))).toThrow(
    HostOutputError,
  );
});

test.concurrent("opencode: the evaluated permission effect is the outcome", () => {
  expect(readOpencodeOutcome({ effect: "ask", message: "m" })).toEqual({
    effect: "ask",
    reason: "m",
  });
  expect(readOpencodeOutcome({ effect: "allow" })).toEqual({ effect: "allow" });
  expect(() => readOpencodeOutcome({ effect: "maybe" })).toThrow(HostOutputError);
});

test.concurrent("claude and codex: PermissionRequest decision.behavior deny is a deny", () => {
  const out = result(
    JSON.stringify({
      hookSpecificOutput: {
        hookEventName: "PermissionRequest",
        decision: { behavior: "deny", message: "no" },
      },
    }),
  );
  for (const host of ["claude", "codex"] as const) {
    expect(readHostOutcome(host, "PermissionRequest", out)).toEqual({
      effect: "deny",
      reason: "no",
    });
  }
});

test.concurrent("cursor: beforeSubmitPrompt answers continue and context events answer context", () => {
  const stop = result(JSON.stringify({ continue: false, user_message: "stop" }));
  expect(readHostOutcome("cursor", "beforeSubmitPrompt", stop)).toEqual({
    effect: "deny",
    reason: "stop",
  });
  const go = result(JSON.stringify({ continue: true }));
  expect(readHostOutcome("cursor", "beforeSubmitPrompt", go)).toEqual({ effect: "allow" });
  const ctx = result(JSON.stringify({ additional_context: "note" }));
  expect(readHostOutcome("cursor", "postToolUse", ctx)).toEqual({
    effect: "allow",
    context: "note",
  });
  expect(readHostOutcome("cursor", "preCompact", result("{}"))).toEqual({ effect: "allow" });
  expect(() => readHostOutcome("cursor", "beforeSubmitPrompt", result("{}"))).toThrow(
    HostOutputError,
  );
});

test.concurrent("hermes: action or decision block denies, context advises, empty allows", async () => {
  expect(readHermesOutcome(result(JSON.stringify({ action: "block", message: "m" })))).toEqual({
    effect: "deny",
    reason: "m",
  });
  expect(readHermesOutcome(result(JSON.stringify({ decision: "block", reason: "r" })))).toEqual({
    effect: "deny",
    reason: "r",
  });
  expect(readHermesOutcome(result(JSON.stringify({ context: "c" })))).toEqual({
    effect: "allow",
    context: "c",
  });
  expect(readHermesOutcome(result(""))).toEqual({ effect: "allow" });
  const blocked = await run(["sh", "-c", "echo 'no' >&2; exit 2"]);
  expect(readHermesOutcome(blocked)).toEqual({ effect: "deny", reason: "no" });
  expect(() => readHermesOutcome(result('{"action":"maybe"}'))).toThrow(HostOutputError);
  expect(() => readHermesOutcome(result("", 1, "boom"))).toThrow("exited 1");
});
