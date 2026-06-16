import { describe, expect, test } from "bun:test";
import {
  mappedToolName,
  toolInputForEvent,
  parseBashExitCode,
  toolResultPayload,
  parseHookOutput,
  hookText,
  gateFile,
  projectRoot,
} from "../toolu-lib.ts";

describe("mappedToolName", () => {
  test("maps known tools correctly", () => {
    expect(mappedToolName("bash")).toBe("Bash");
    expect(mappedToolName("read")).toBe("Read");
    expect(mappedToolName("edit")).toBe("Edit");
    expect(mappedToolName("write")).toBe("Write");
    expect(mappedToolName("grep")).toBe("Grep");
    expect(mappedToolName("find")).toBe("Glob");
    expect(mappedToolName("ls")).toBe("Glob");
  });

  test("returns undefined for unknown tools", () => {
    expect(mappedToolName("unknown")).toBeUndefined();
  });
});

describe("gateFile", () => {
  test("constructs correct path under project root", () => {
    const result = gateFile("/some/path");
    expect(result).toMatch(/\.claude\/tmp\/quality-gate-status\.json$/);
  });
});

describe("parseBashExitCode", () => {
  test("returns undefined for non-bash tools", () => {
    const event = { toolName: "write", content: [] } as any;
    expect(parseBashExitCode(event)).toBeUndefined();
  });

  test("returns 0 for successful bash commands", () => {
    const event = { toolName: "bash", isError: false, content: [] } as any;
    expect(parseBashExitCode(event)).toBe(0);
  });

  test("extracts exit code from error text", () => {
    const event = {
      toolName: "bash",
      isError: true,
      content: [{ type: "text", text: "Command exited with code 42" }],
    } as any;
    expect(parseBashExitCode(event)).toBe(42);
  });

  test("returns undefined when no exit code found", () => {
    const event = {
      toolName: "bash",
      isError: true,
      content: [{ type: "text", text: "Some other error" }],
    } as any;
    expect(parseBashExitCode(event)).toBeUndefined();
  });
});

describe("toolResultPayload", () => {
  test("includes exit code in tool_response and tool_output", () => {
    const event = {
      toolName: "bash",
      isError: true,
      content: [{ type: "text", text: "Command exited with code 1" }],
      input: { command: "exit 1" },
    } as any;
    const payload = toolResultPayload(event);
    const parsed = JSON.parse(payload);
    expect(parsed.tool_response.metadata.exit_code).toBe(1);
    expect(parsed.tool_output.exitCode).toBe(1);
  });
});

describe("parseHookOutput", () => {
  test("parses valid JSON output", () => {
    const output = parseHookOutput('{"hookSpecificOutput": {"additionalContext": "hello"}}');
    expect(output?.hookSpecificOutput?.additionalContext).toBe("hello");
  });

  test("returns undefined for empty string", () => {
    expect(parseHookOutput("")).toBeUndefined();
  });

  test("returns undefined for invalid JSON", () => {
    expect(parseHookOutput("{ invalid json }")).toBeUndefined();
  });
});

describe("hookText", () => {
  test("joins systemMessage and additionalContext", () => {
    const output = {
      systemMessage: "System",
      hookSpecificOutput: { additionalContext: "Context" },
    };
    expect(hookText(output)).toBe("System\n\nContext");
  });

  test("returns undefined for undefined output", () => {
    expect(hookText(undefined)).toBeUndefined();
  });

  test("returns undefined when both are missing", () => {
    expect(hookText({})).toBeUndefined();
  });
});
