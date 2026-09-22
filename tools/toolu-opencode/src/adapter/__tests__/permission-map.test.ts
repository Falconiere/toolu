import { expect, test } from "bun:test";
import type { Decision } from "@toolu/core/decision";
import {
  applyDecisionToPermission,
  mapPermissionEventToBridge,
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

test("AC-4: mapPermissionEventToBridge maps edit to Edit tool/pre", () => {
  const mapping = mapPermissionEventToBridge(event(), ctx);
  expect(mapping.kind).toBe("request");
  if (mapping.kind !== "request") {
    return;
  }
  expect(mapping.request.protocolVersion).toBe(1);
  expect(mapping.request.event).toBe("tool/pre");
  expect(mapping.request.toolName).toBe("Edit");
  expect(mapping.request.toolInput).toEqual({ file_path: "/proj/.env" });
  expect(mapping.request.host).toBe("opencode");
});

test("AC-4: mapPermissionEventToBridge skips unknown actions", () => {
  expect(mapPermissionEventToBridge(event({ action: "read" }), ctx).kind).toBe("skip");
});

test("AC-4: gated write without path fails closed in mapper", () => {
  const mapping = mapPermissionEventToBridge(event({ action: "write", resources: [] }), ctx);
  expect(mapping.kind).toBe("deny");
});

test("AC-4: bash maps command from metadata", () => {
  const mapping = mapPermissionEventToBridge(
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
  expect(mapping.request.toolName).toBe("Bash");
  expect(mapping.request.toolInput).toEqual({ command: "rm -rf /" });
});

const decisionCases: Array<{
  decision: Decision;
  effect: PermissionEvaluationEvent["effect"];
  message?: string;
}> = [
  { decision: { kind: "deny", reason: "blocked" }, effect: "deny", message: "blocked" },
  { decision: { kind: "ask", reason: "confirm" }, effect: "ask", message: "confirm" },
  { decision: { kind: "allow" }, effect: "allow" },
  { decision: { kind: "advisory", message: "hint" }, effect: "allow", message: "hint" },
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
