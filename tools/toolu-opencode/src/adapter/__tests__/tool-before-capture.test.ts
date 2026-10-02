import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { z } from "zod";
import { applyPatchRecords } from "@toolu/core/state";
import { mapToolCall } from "../tool-before.ts";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const CAPTURE = join(REPO_ROOT, "tools/toolu-opencode/contract/captures/tool-calls.jsonl");
const CTX = { cwd: "/work/app", projectRoot: "/work/app", worktree: "/work/app" };

const CaptureLine = z.object({
  tool: z.string(),
  sessionID: z.string(),
  callID: z.string(),
  args: z.record(z.string(), z.unknown()),
});

type CaptureLine = z.infer<typeof CaptureLine>;

function capture(): CaptureLine[] {
  return readFileSync(CAPTURE, "utf8")
    .split("\n")
    .filter((row) => row !== "")
    .map((row) => {
      const parsed: unknown = JSON.parse(row);
      return CaptureLine.parse(parsed);
    });
}

function captured(tool: string, where?: (entry: CaptureLine) => boolean): CaptureLine {
  const found = capture().find((entry) => entry.tool === tool && (where?.(entry) ?? true));
  if (found === undefined) throw new Error(`capture has no ${tool}`);
  return found;
}

function requestOf(entry: CaptureLine): Record<string, unknown> {
  const result = mapToolCall(
    { tool: entry.tool, sessionID: entry.sessionID, callID: entry.callID },
    entry.args,
    CTX,
    ["probe"],
  );
  if (result.kind !== "request") throw new Error(`${entry.tool} was ${result.kind}`);
  return result.request;
}

test("replays read, grep, glob, edit and write from the host capture", () => {
  const read = captured("read");
  expect(requestOf(read)).toMatchObject({
    session_id: read.sessionID,
    tool_use_id: read.callID,
    cwd: "/work/app",
    tool_name: "Read",
    tool_input: { file_path: read.args.filePath },
  });
  const grep = captured("grep");
  expect(requestOf(grep).tool_input).toEqual({
    pattern: grep.args.pattern,
    path: grep.args.path,
    include: grep.args.include,
    glob: grep.args.include,
  });
  const glob = captured("glob");
  expect(requestOf(glob)).toMatchObject({
    tool_name: "Glob",
    tool_input: { pattern: glob.args.pattern, path: glob.args.path },
  });
  const edit = captured("edit");
  expect(requestOf(edit).tool_input).toEqual({
    file_path: edit.args.filePath,
    old_string: edit.args.oldString,
    new_string: edit.args.newString,
    replace_all: false,
  });
  const write = captured("write");
  expect(requestOf(write).tool_input).toEqual({
    file_path: write.args.filePath,
    content: write.args.content,
  });
});

test("replays bash workdir, task, mcp and the child session", () => {
  const bash = captured("bash", (entry) => entry.args.workdir === "nested");
  expect(requestOf(bash)).toMatchObject({
    tool_name: "Bash",
    cwd: "/work/app/nested",
    session_id: bash.sessionID,
    tool_use_id: bash.callID,
    tool_input: { command: "pwd", timeout: 5000 },
  });
  const task = captured("task");
  expect(requestOf(task).tool_input).toEqual({
    description: "child probe",
    prompt: "PROBE:tool-capture-child",
    subagent_type: "general",
  });
  const touch = captured("probe_touch");
  expect(requestOf(touch)).toMatchObject({
    tool_name: "mcp__probe__touch",
    session_id: touch.sessionID,
    tool_use_id: touch.callID,
    tool_input: { name: "captured", opencode_tool: "probe_touch" },
  });
  const child = captured("bash", (entry) => entry.sessionID !== task.sessionID);
  expect(child.sessionID).not.toBe(task.sessionID);
  expect(requestOf(child)).toMatchObject({
    session_id: child.sessionID,
    tool_use_id: child.callID,
    cwd: "/work/app",
    tool_input: { command: "echo child" },
  });
});

test("replays the captured multi-file patch", () => {
  const patched = captured("apply_patch");
  const text = patched.args.patchText;
  if (typeof text !== "string") throw new Error("patchText missing");
  const result = mapToolCall(
    { tool: patched.tool, sessionID: patched.sessionID, callID: patched.callID },
    patched.args,
    CTX,
    ["probe"],
  );
  if (result.kind !== "request") throw new Error("patch was not mapped");
  expect(result.request.tool_name).toBe("apply_patch");
  expect(result.request.tool_input).toEqual({ command: text, patchText: text });
  const records = applyPatchRecords(text);
  expect(records?.map((record) => record.operation).toSorted()).toEqual([
    "add",
    "delete",
    "move",
    "update",
  ]);
  expect(records?.map((record) => record.path).every((path) => path.length > 0)).toBe(true);
});
