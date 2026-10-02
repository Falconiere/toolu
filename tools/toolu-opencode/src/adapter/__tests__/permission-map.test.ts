import { expect, test } from "bun:test";
import type { Decision } from "@toolu/core/decision";
import {
  applyDecisionToPermission,
  mapPermissionEventToTool,
  type PermissionEvaluationEvent,
} from "../permission-map.ts";

const ctx = {
  cwd: "/proj",
  projectRoot: "/proj",
  worktree: "/proj",
  host: "opencode",
};

function event(overrides: Partial<PermissionEvaluationEvent> = {}): PermissionEvaluationEvent {
  return {
    sessionID: "sess_1",
    action: "edit",
    resources: ["/proj/.env"],
    effect: "allow",
    ...overrides,
  };
}

test("AC-4: mapPermissionEventToTool maps edit to PreToolUse payload", () => {
  const mapping = mapPermissionEventToTool(event(), ctx);
  expect(mapping.kind).toBe("request");
  if (mapping.kind !== "request") {
    return;
  }
  expect(mapping.request.tool_name).toBe("Edit");
  expect(mapping.request.tool_input).toEqual({ file_path: "/proj/.env" });
  expect(mapping.request.session_id).toBe("sess_1");
});

test("AC-4: mapPermissionEventToTool skips unknown actions", () => {
  expect(mapPermissionEventToTool(event({ action: "read" }), ctx).kind).toBe("skip");
});

test("AC-4: gated write without path fails closed in mapper", () => {
  const mapping = mapPermissionEventToTool(event({ action: "write", resources: [] }), ctx);
  expect(mapping.kind).toBe("deny");
});

test("AC-4: bash maps command from metadata", () => {
  const mapping = mapPermissionEventToTool(
    event({
      action: "bash",
      resources: [],
      metadata: { command: "rm -rf /" },
    }),
    ctx,
  );
  expect(mapping.kind).toBe("request");
  if (mapping.kind !== "request") {
    return;
  }
  expect(mapping.request.tool_name).toBe("Bash");
  expect(mapping.request.tool_input).toEqual({ command: "rm -rf /" });
});

const decisionCases: Array<{
  decision: Decision;
  effect: PermissionEvaluationEvent["effect"];
  message?: string;
}> = [
  { decision: { kind: "deny", reason: "blocked" }, effect: "deny", message: "blocked" },
  { decision: { kind: "ask", reason: "confirm" }, effect: "deny", message: "confirm" },
  { decision: { kind: "allow" }, effect: "allow" },
  { decision: { kind: "advisory", message: "hint" }, effect: "allow" },
  { decision: { kind: "post_block", reason: "post" }, effect: "deny", message: "post" },
  {
    decision: { kind: "runtime_failure", reason: "fail", code: "parse" },
    effect: "deny",
    message: "fail",
  },
];

for (const { decision, effect, message } of decisionCases) {
  test(`AC-4: applyDecisionToPermission ${decision.kind}`, () => {
    const ev = event();
    applyDecisionToPermission(decision, ev);
    expect(ev.effect).toBe(effect);
    if (message !== undefined) {
      expect(ev.message).toBe(message);
    }
  });
}

test("toolu allow or advisory preserves a stricter native deny or ask", () => {
  const decisions: Decision[] = [{ kind: "allow" }, { kind: "advisory", message: "hint" }];
  for (const effect of ["deny", "ask"] as const) {
    for (const decision of decisions) {
      const ev = event({ effect, message: "native reason" });
      applyDecisionToPermission(decision, ev);
      expect(ev.effect).toBe(effect);
      expect(ev.message).toBe("native reason");
    }
  }
});

test("a generated ask never weakens a native deny or replaces an existing native ask", () => {
  for (const effect of ["deny", "ask"] as const) {
    const ev = event({ effect, message: "native reason" });
    applyDecisionToPermission({ kind: "ask", reason: "toolu ask" }, ev);
    expect(ev.effect).toBe(effect);
    expect(ev.message).toBe("native reason");
  }
});
