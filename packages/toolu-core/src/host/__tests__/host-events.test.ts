import { describe, expect, test } from "bun:test";
import {
  HOST_EVENTS,
  canonicalEvent,
  hostsForNativeEvent,
  nativeEventName,
} from "../host-events.ts";
import { HOST_NAMES } from "../host.ts";

describe("nativeEventName", () => {
  test("Claude and Codex share PascalCase names", () => {
    for (const host of ["claude", "codex"] as const) {
      expect(nativeEventName(host, "tool/pre")).toBe("PreToolUse");
      expect(nativeEventName(host, "shell/pre")).toBe("PreToolUse");
      expect(nativeEventName(host, "tool/post")).toBe("PostToolUse");
      expect(nativeEventName(host, "session/start")).toBe("SessionStart");
      expect(nativeEventName(host, "session/unload")).toBe("SessionEnd");
      expect(nativeEventName(host, "prompt")).toBe("UserPromptSubmit");
      expect(nativeEventName(host, "pre_compact")).toBe("PreCompact");
      expect(nativeEventName(host, "permission/evaluate")).toBe("PermissionRequest");
    }
  });

  test("Cursor uses camelCase and splits shell out of preToolUse", () => {
    expect(nativeEventName("cursor", "tool/pre")).toBe("preToolUse");
    expect(nativeEventName("cursor", "shell/pre")).toBe("beforeShellExecution");
    expect(nativeEventName("cursor", "tool/post")).toBe("postToolUse");
    expect(nativeEventName("cursor", "prompt")).toBe("beforeSubmitPrompt");
    expect(nativeEventName("cursor", "permission/evaluate")).toBeNull();
  });

  test("Hermes uses snake_case and has no compaction or permission event", () => {
    expect(nativeEventName("hermes", "tool/pre")).toBe("pre_tool_call");
    expect(nativeEventName("hermes", "tool/post")).toBe("post_tool_call");
    expect(nativeEventName("hermes", "prompt")).toBe("pre_llm_call");
    expect(nativeEventName("hermes", "session/start")).toBe("on_session_start");
    expect(nativeEventName("hermes", "pre_compact")).toBeNull();
    expect(nativeEventName("hermes", "permission/evaluate")).toBeNull();
  });

  test("OpenCode uses dotted plugin hook names", () => {
    expect(nativeEventName("opencode", "tool/pre")).toBe("tool.execute.before");
    expect(nativeEventName("opencode", "permission/evaluate")).toBe("permission.evaluate");
    expect(nativeEventName("opencode", "session/start")).toBe("session.created");
  });
});

describe("canonicalEvent", () => {
  test("every mapped row round-trips to an event with the same native name", () => {
    for (const host of HOST_NAMES) {
      for (const event of HOST_EVENTS) {
        const native = nativeEventName(host, event);
        if (native === null) continue;
        const back = canonicalEvent(host, native);
        expect(back).not.toBeNull();
        expect(back === null ? null : nativeEventName(host, back)).toBe(native);
      }
    }
  });

  test("a shared native name reverses to the first row", () => {
    expect(canonicalEvent("claude", "PreToolUse")).toBe("tool/pre");
    expect(canonicalEvent("hermes", "pre_tool_call")).toBe("tool/pre");
  });

  test("Cursor aliases fold into tool/pre and tool/post", () => {
    expect(canonicalEvent("cursor", "beforeMCPExecution")).toBe("tool/pre");
    expect(canonicalEvent("cursor", "afterFileEdit")).toBe("tool/post");
    expect(canonicalEvent("cursor", "afterShellExecution")).toBe("tool/post");
    expect(canonicalEvent("cursor", "afterMCPExecution")).toBe("tool/post");
  });

  test("names from another host or unknown names are null", () => {
    expect(canonicalEvent("claude", "preToolUse")).toBeNull();
    expect(canonicalEvent("cursor", "PreToolUse")).toBeNull();
    expect(canonicalEvent("codex", "Notification")).toBeNull();
  });
});

describe("hostsForNativeEvent", () => {
  test("PascalCase names belong to both Claude and Codex", () => {
    expect(hostsForNativeEvent("PreToolUse")).toEqual(["claude", "codex"]);
  });

  test("Cursor, Hermes and OpenCode names belong to one host", () => {
    expect(hostsForNativeEvent("beforeShellExecution")).toEqual(["cursor"]);
    expect(hostsForNativeEvent("afterFileEdit")).toEqual(["cursor"]);
    expect(hostsForNativeEvent("pre_tool_call")).toEqual(["hermes"]);
    expect(hostsForNativeEvent("tool.execute.before")).toEqual(["opencode"]);
  });

  test("an unknown name belongs to no host", () => {
    expect(hostsForNativeEvent("pre_compact")).toEqual([]);
    expect(hostsForNativeEvent("")).toEqual([]);
  });
});
