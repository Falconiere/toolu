/**
 * AC-4 (#259): the host's exit status and interrupt flag read exactly as the
 * shipped `gate-status.sh` and `push-waiver.sh` read them with jq. The bash
 * side runs the modules' own lines (asserted verbatim below) under the real jq.
 */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { isJsonObject } from "../../config/config-load.ts";
import { toolCommand, toolExitStatus, toolInterrupted } from "../tool-exit.ts";

const MODULES = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/post-tools/modules");
const GATE_STATUS = readFileSync(`${MODULES}/gate-status.sh`, "utf8");
const PUSH_WAIVER = readFileSync(`${MODULES}/push-waiver.sh`, "utf8");

const COMMAND = `command=$(echo "$input" | jq -r '.tool_input.command // ""' 2>/dev/null || echo "")`;
const EXIT = `exit_code=$(echo "$input" | jq -r '.tool_response.metadata.exit_code // .tool_response.exit_code // .tool_output.exit_code // empty' 2>/dev/null || echo "")
if [[ -z "$exit_code" || "$exit_code" == "null" ]]; then
  tool_output_raw=$(echo "$input" | jq -r '.tool_output // empty' 2>/dev/null || echo "")
  if [[ -n "$tool_output_raw" ]]; then
    exit_code=$(echo "$tool_output_raw" | jq -r '.exitCode // .exit_code // empty' 2>/dev/null || echo "")
  fi
fi`;
const INTERRUPTED = `interrupted=$(echo "$input" | jq -r '.tool_response.interrupted // false' 2>/dev/null || echo "false")`;

test("the bash lines are the shipped modules' own", () => {
  for (const line of [COMMAND, EXIT]) {
    expect(GATE_STATUS).toContain(line);
    expect(PUSH_WAIVER).toContain(line);
  }
  expect(PUSH_WAIVER).toContain(INTERRUPTED);
});

type Read = { command: string; exit: string; interrupted: string };

function bashRead(payload: string): Read {
  const script = `input=$(cat)\n${COMMAND}\n${EXIT}\n${INTERRUPTED}\nprintf '%s\\0%s\\0%s' "$command" "$exit_code" "$interrupted"`;
  const res = spawnSync("bash", ["-c", script], { input: payload, encoding: "utf8" });
  const [command = "", exit = "", interrupted = ""] = res.stdout.split("\0");
  // push-waiver.sh only asks `[[ "$interrupted" == "true" ]]`; empty stdin prints nothing.
  return { command, exit, interrupted: interrupted === "true" ? "true" : "false" };
}

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

for (const [name, payload] of Object.entries(PAYLOADS)) {
  test.concurrent(name, () => {
    const text = JSON.stringify(payload);
    expect(tsRead(text)).toEqual(bashRead(text));
  });
}

for (const [name, text] of Object.entries({
  "empty stdin": "",
  "not JSON": "not json",
  array: "[1]",
})) {
  test.concurrent(`raw input: ${name}`, () => {
    expect(tsRead(text)).toEqual(bashRead(text));
  });
}
