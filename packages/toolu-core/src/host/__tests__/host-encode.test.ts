import { describe, expect, test } from "bun:test";
import type { Decision } from "../../decision/decision.ts";
import { degradeAsk, encodeDecision, supportsAsk } from "../host-encode.ts";
import { HOST_EVENTS, nativeEventName } from "../host-events.ts";
import { HOST_NAMES } from "../host-name.ts";

const ALLOW: Decision = { kind: "allow" };
const ASK: Decision = { kind: "ask", reason: "confirm .env write" };
const DENY: Decision = { kind: "deny", reason: "protected file" };
const ADVISE: Decision = { kind: "advisory", message: "run the tests" };
const POST_BLOCK: Decision = { kind: "post_block", reason: "lint failed" };
const FAILURE: Decision = { kind: "runtime_failure", reason: "gate crashed", code: "nonzero" };
const DECISIONS = [ALLOW, ASK, DENY, ADVISE, POST_BLOCK, FAILURE];

function stdout(...args: Parameters<typeof encodeDecision>): unknown {
  const out = encodeDecision(...args);
  if (out.kind !== "command") throw new Error(`expected command output, got ${out.kind}`);
  expect(out.exitCode).toBe(0);
  expect(out.stderr).toBe("");
  return out.stdout === "" ? "" : JSON.parse(out.stdout);
}

describe("supportsAsk", () => {
  test("defaults to the PreToolUse question bash toolu_supports_ask answers", () => {
    expect(supportsAsk("claude")).toBe(true);
    expect(supportsAsk("codex")).toBe(false);
    expect(supportsAsk("cursor")).toBe(false);
    expect(supportsAsk("hermes")).toBe(false);
    expect(supportsAsk("opencode")).toBe(true);
  });

  test("Cursor asks only before shell execution; permission requests defer to the host prompt", () => {
    expect(supportsAsk("cursor", "shell/pre")).toBe(true);
    expect(supportsAsk("codex", "permission/evaluate")).toBe(true);
    expect(supportsAsk("claude", "prompt")).toBe(false);
    expect(supportsAsk("opencode", "tool/post")).toBe(false);
  });
});

describe("degradeAsk", () => {
  test("a guardrail's ask becomes a block where the host cannot prompt", () => {
    expect(degradeAsk("codex", "tool/pre", ASK, "guardrail")).toEqual({
      kind: "deny",
      reason: "confirm .env write",
    });
  });

  test("a judgement gate's ask becomes advice where the host cannot prompt", () => {
    expect(degradeAsk("hermes", "shell/pre", ASK, "judgement")).toEqual({
      kind: "advisory",
      message: "confirm .env write",
    });
  });

  test("ask survives where the host prompts, and other decisions pass through", () => {
    expect(degradeAsk("claude", "tool/pre", ASK, "guardrail")).toEqual(ASK);
    expect(degradeAsk("cursor", "shell/pre", ASK, "judgement")).toEqual(ASK);
    expect(degradeAsk("codex", "tool/pre", DENY, "judgement")).toEqual(DENY);
  });
});

describe("Claude and Codex encoding", () => {
  test("allow is silent success on every event", () => {
    for (const host of ["claude", "codex"] as const) {
      for (const event of HOST_EVENTS) expect(stdout(host, event, ALLOW)).toBe("");
    }
  });

  test("PreToolUse deny, ask and advice use hookSpecificOutput", () => {
    expect(stdout("claude", "shell/pre", DENY)).toEqual({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: "protected file",
      },
    });
    expect(stdout("claude", "tool/pre", ASK)).toEqual({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "ask",
        permissionDecisionReason: "confirm .env write",
      },
    });
    expect(stdout("codex", "tool/pre", ADVISE)).toEqual({
      hookSpecificOutput: { hookEventName: "PreToolUse", additionalContext: "run the tests" },
    });
  });

  test("Codex never receives ask on PreToolUse: it fails closed to deny", () => {
    expect(stdout("codex", "tool/pre", ASK)).toEqual({
      hookSpecificOutput: {
        hookEventName: "PreToolUse",
        permissionDecision: "deny",
        permissionDecisionReason: "confirm .env write",
      },
    });
  });

  test("PostToolUse carries blocks as decision:block and advice as additionalContext", () => {
    expect(stdout("codex", "tool/post", POST_BLOCK)).toEqual({
      decision: "block",
      reason: "lint failed",
    });
    expect(stdout("claude", "tool/post", DENY)).toEqual({
      decision: "block",
      reason: "protected file",
    });
    expect(stdout("claude", "tool/post", ADVISE)).toEqual({
      hookSpecificOutput: { hookEventName: "PostToolUse", additionalContext: "run the tests" },
    });
  });

  test("prompts block with decision:block and take context", () => {
    expect(stdout("claude", "prompt", DENY)).toEqual({
      decision: "block",
      reason: "protected file",
    });
    expect(stdout("codex", "prompt", ADVISE)).toEqual({
      hookSpecificOutput: { hookEventName: "UserPromptSubmit", additionalContext: "run the tests" },
    });
  });

  test("context-only events cannot block, so deny degrades to context", () => {
    expect(stdout("claude", "session/start", DENY)).toEqual({
      hookSpecificOutput: { hookEventName: "SessionStart", additionalContext: "protected file" },
    });
    expect(stdout("claude", "pre_compact", ADVISE)).toEqual({ systemMessage: "run the tests" });
    expect(stdout("codex", "session/unload", POST_BLOCK)).toEqual({ systemMessage: "lint failed" });
  });

  test("PermissionRequest denies with decision.behavior and defers ask to the native prompt", () => {
    expect(stdout("claude", "permission/evaluate", DENY)).toEqual({
      hookSpecificOutput: {
        hookEventName: "PermissionRequest",
        decision: { behavior: "deny", message: "protected file" },
      },
    });
    expect(stdout("codex", "permission/evaluate", ASK)).toBe("");
    expect(stdout("claude", "permission/evaluate", ADVISE)).toEqual({
      systemMessage: "run the tests",
    });
  });
});

describe("Cursor encoding", () => {
  test("permission events always answer with a permission", () => {
    expect(stdout("cursor", "tool/pre", ALLOW)).toEqual({ permission: "allow" });
    expect(stdout("cursor", "tool/pre", ADVISE)).toEqual({
      permission: "allow",
      agent_message: "run the tests",
    });
    expect(stdout("cursor", "shell/pre", DENY)).toEqual({
      permission: "deny",
      user_message: "protected file",
      agent_message: "protected file",
    });
    expect(stdout("cursor", "shell/pre", ASK)).toEqual({
      permission: "ask",
      user_message: "confirm .env write",
      agent_message: "confirm .env write",
    });
  });

  test("preToolUse does not enforce ask, so ask fails closed to deny", () => {
    expect(stdout("cursor", "tool/pre", ASK)).toEqual({
      permission: "deny",
      user_message: "confirm .env write",
      agent_message: "confirm .env write",
    });
  });

  test("prompts continue or stop; post and session hooks take additional_context", () => {
    expect(stdout("cursor", "prompt", DENY)).toEqual({
      continue: false,
      user_message: "protected file",
    });
    expect(stdout("cursor", "prompt", ADVISE)).toEqual({ continue: true });
    expect(stdout("cursor", "tool/post", POST_BLOCK)).toEqual({
      additional_context: "lint failed",
    });
    expect(stdout("cursor", "session/start", ADVISE)).toEqual({
      additional_context: "run the tests",
    });
    expect(stdout("cursor", "pre_compact", ADVISE)).toEqual({});
  });
});

describe("Hermes encoding", () => {
  test("pre_tool_call blocks with action:block; allow and advice are silent", () => {
    expect(stdout("hermes", "tool/pre", DENY)).toEqual({
      action: "block",
      message: "protected file",
    });
    expect(stdout("hermes", "shell/pre", ASK)).toEqual({
      action: "block",
      message: "confirm .env write",
    });
    expect(stdout("hermes", "tool/pre", ADVISE)).toBe("");
    expect(stdout("hermes", "tool/pre", ALLOW)).toBe("");
  });

  test("pre_llm_call takes context; tool and session hooks have no channel", () => {
    expect(stdout("hermes", "prompt", ADVISE)).toEqual({ context: "run the tests" });
    expect(stdout("hermes", "prompt", DENY)).toEqual({ context: "protected file" });
    expect(stdout("hermes", "tool/post", POST_BLOCK)).toBe("");
    expect(stdout("hermes", "session/start", ADVISE)).toBe("");
  });
});

describe("OpenCode encoding", () => {
  test("decisions become a permission effect", () => {
    expect(encodeDecision("opencode", "permission/evaluate", DENY)).toEqual({
      kind: "effect",
      effect: "deny",
      message: "protected file",
    });
    expect(encodeDecision("opencode", "tool/pre", ASK)).toEqual({
      kind: "effect",
      effect: "ask",
      message: "confirm .env write",
    });
    expect(encodeDecision("opencode", "tool/post", POST_BLOCK)).toEqual({
      kind: "effect",
      effect: "allow",
      message: "lint failed",
    });
    expect(encodeDecision("opencode", "tool/pre", ALLOW)).toEqual({
      kind: "effect",
      effect: "allow",
    });
  });
});

describe("fail-closed invariants", () => {
  test("a runtime failure denies every pre-action event on every host", () => {
    for (const host of HOST_NAMES) {
      for (const event of ["tool/pre", "shell/pre"] as const) {
        const text = JSON.stringify(encodeDecision(host, event, FAILURE));
        expect(text).toMatch(/deny|block/);
        expect(text).toContain("gate crashed");
      }
    }
  });

  test("ask never reaches a host or event that cannot prompt", () => {
    for (const host of HOST_NAMES) {
      for (const event of HOST_EVENTS) {
        if (nativeEventName(host, event) === null || supportsAsk(host, event)) continue;
        for (const decision of DECISIONS) {
          expect(JSON.stringify(encodeDecision(host, event, decision))).not.toContain('"ask"');
        }
      }
    }
  });

  test("an event the host does not have is a wiring error", () => {
    expect(() => encodeDecision("cursor", "permission/evaluate", DENY)).toThrow(
      "cursor has no native event for permission/evaluate",
    );
    expect(() => encodeDecision("hermes", "pre_compact", ALLOW)).toThrow();
  });
});
