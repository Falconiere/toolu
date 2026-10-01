/** Host command, exit-status and interrupt extraction across real payload shapes. */
import { expect, test } from "bun:test";
import { isJsonObject } from "../../config/config-load.ts";
import { toolCommand, toolExitStatus, toolInterrupted } from "../tool-exit.ts";

type Read = { command: string; exit: string; interrupted: string };

function tsRead(payload: string): Read {
  let doc: unknown;
  try {
    doc = JSON.parse(payload);
  } catch {
    doc = undefined;
  }
  const raw = isJsonObject(doc) ? doc : {};
  return {
    command: toolCommand(raw),
    exit: toolExitStatus(raw),
    interrupted: toolInterrupted(raw) ? "true" : "false",
  };
}

const cmd = { tool_input: { command: "bun test" } };
const PAYLOADS: Record<string, unknown> = {
  "Claude metadata exit 1": { ...cmd, tool_response: { metadata: { exit_code: 1 } } },
  "Claude metadata exit 0": { ...cmd, tool_response: { metadata: { exit_code: 0 } } },
  "tool_response.exit_code": { ...cmd, tool_response: { exit_code: 3 } },
  "Codex response": {
    session_id: "s",
    tool_name: "Bash",
    ...cmd,
    tool_response: { metadata: { exit_code: 1 }, stdout: "", stderr: "failed" },
  },
  "metadata false falls through": {
    ...cmd,
    tool_response: { metadata: { exit_code: false }, exit_code: 2 },
  },
  "tool_output.exit_code": { ...cmd, tool_output: { exit_code: 4 } },
  "Cursor tool_output JSON string": { ...cmd, tool_output: '{"exitCode":1}' },
  "Cursor tool_output object exitCode": { ...cmd, tool_output: { exitCode: 0 } },
  "tool_output non-JSON string": { ...cmd, tool_output: "plain text" },
  "string tool_response is a jq error": {
    ...cmd,
    tool_response: "done",
    tool_output: { exitCode: 5 },
  },
  "string exit_code": { ...cmd, tool_response: { exit_code: "x y" } },
  "string null exit_code": { ...cmd, tool_response: { exit_code: "null" } },
  "string null with tool_output": {
    ...cmd,
    tool_response: { exit_code: "null" },
    tool_output: '{"exit_code":0}',
  },
  "fractional exit_code": { ...cmd, tool_response: { exit_code: 1.5 } },
  "array exit_code": { ...cmd, tool_response: { exit_code: [1] } },
  "no exit code at all": { ...cmd, tool_response: { stdout: "" } },
  "interrupted true": { ...cmd, tool_response: { interrupted: true } },
  "interrupted string true": { ...cmd, tool_response: { interrupted: "true" } },
  "interrupted false": { ...cmd, tool_response: { interrupted: false, exit_code: 0 } },
  "numeric command": { tool_input: { command: 7 } },
  "missing tool_input": { tool_response: { exit_code: 0 } },
  "tool_input is a string": { tool_input: "ls", tool_response: { exit_code: 0 } },
};

const EXPECTED: Record<string, Read> = {
  "Claude metadata exit 1": { command: "bun test", exit: "1", interrupted: "false" },
  "Claude metadata exit 0": { command: "bun test", exit: "0", interrupted: "false" },
  "tool_response.exit_code": { command: "bun test", exit: "3", interrupted: "false" },
  "Codex response": { command: "bun test", exit: "1", interrupted: "false" },
  "metadata false falls through": { command: "bun test", exit: "2", interrupted: "false" },
  "tool_output.exit_code": { command: "bun test", exit: "4", interrupted: "false" },
  "Cursor tool_output JSON string": { command: "bun test", exit: "1", interrupted: "false" },
  "Cursor tool_output object exitCode": { command: "bun test", exit: "0", interrupted: "false" },
  "tool_output non-JSON string": { command: "bun test", exit: "", interrupted: "false" },
  "string tool_response is a jq error": { command: "bun test", exit: "5", interrupted: "false" },
  "string exit_code": { command: "bun test", exit: "x y", interrupted: "false" },
  "string null exit_code": { command: "bun test", exit: "null", interrupted: "false" },
  "string null with tool_output": { command: "bun test", exit: "0", interrupted: "false" },
  "fractional exit_code": { command: "bun test", exit: "1.5", interrupted: "false" },
  "array exit_code": { command: "bun test", exit: "[\n  1\n]", interrupted: "false" },
  "no exit code at all": { command: "bun test", exit: "", interrupted: "false" },
  "interrupted true": { command: "bun test", exit: "", interrupted: "true" },
  "interrupted string true": { command: "bun test", exit: "", interrupted: "true" },
  "interrupted false": { command: "bun test", exit: "0", interrupted: "false" },
  "numeric command": { command: "7", exit: "", interrupted: "false" },
  "missing tool_input": { command: "", exit: "0", interrupted: "false" },
  "tool_input is a string": { command: "", exit: "0", interrupted: "false" },
};

test("every host payload has an explicit expected result", () => {
  expect(Object.keys(EXPECTED)).toEqual(Object.keys(PAYLOADS));
});

for (const [name, payload] of Object.entries(PAYLOADS)) {
  test.concurrent(name, () => {
    const text = JSON.stringify(payload);
    const expected = EXPECTED[name];
    if (expected === undefined) throw new Error(`missing expected result for ${name}`);
    expect(tsRead(text)).toEqual(expected);
  });
}

for (const [name, text] of Object.entries({
  "empty stdin": "",
  "not JSON": "not json",
  array: "[1]",
})) {
  test.concurrent(`raw input: ${name}`, () => {
    expect(tsRead(text)).toEqual({ command: "", exit: "", interrupted: "false" });
  });
}
