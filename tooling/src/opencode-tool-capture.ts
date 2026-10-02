#!/usr/bin/env bun
/**
 * Capture pinned-host `tool.execute.before` payloads (#337).
 *
 * `bun tooling/src/opencode-tool-capture.ts` runs two isolated opencode-ai
 * sessions (edit/write and gpt apply_patch are different tool sets) and writes
 * `tools/toolu-opencode/contract/captures/tool-calls.jsonl`.
 * `--check` validates that file and does not start the host.
 */
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { z } from "zod";
import { runHost } from "./opencode-host/host-run.ts";
import { hostCacheDir, resolveHostBinary } from "./opencode-host/install.ts";
import { contractPaths, ROOT } from "./opencode-host/results.ts";
import { PinSchema, readJson } from "./opencode-host/schema.ts";
import { openSession, PROBE_PLUGIN, type SessionOptions } from "./opencode-host/session.ts";

const CAPTURE = join(ROOT, "tools/toolu-opencode/contract/captures/tool-calls.jsonl");
const MCP_SERVER = join(import.meta.dir, "opencode-host/mcp-server.ts");
const HOST_VERSION = "1.18.34";

const BeforeLine = z.object({
  kind: z.literal("before"),
  tool: z.string().min(1),
  sessionID: z.string().min(1),
  callID: z.string().min(1),
  args: z.record(z.string(), z.unknown()),
  hostVersion: z.literal(HOST_VERSION),
});
type BeforeLine = z.infer<typeof BeforeLine>;

function patch(project: string): string {
  return [
    "*** Begin Patch",
    `*** Add File: ${join(project, "added.txt")}`,
    "+hello",
    `*** Update File: ${join(project, "existing.txt")}`,
    `*** Move to: ${join(project, "moved.txt")}`,
    `*** Delete File: ${join(project, "gone.txt")}`,
    "*** End Patch",
  ].join("\n");
}

function requireOne(lines: readonly BeforeLine[], tool: string): BeforeLine {
  const found = lines.filter((line) => line.tool === tool);
  if (found.length !== 1) {
    throw new Error(`${tool}: expected 1 before line, found ${found.length}`);
  }
  const line = found[0];
  if (line === undefined) throw new Error(`${tool}: missing before line`);
  return line;
}

/** Structural contract of the committed capture. Throws on the first gap. */
function assertCapture(lines: readonly BeforeLine[]): void {
  const read = requireOne(lines, "read");
  if (typeof read.args.filePath !== "string" || read.args.filePath.length === 0) {
    throw new Error("read: filePath must be a non-empty string");
  }
  const grep = requireOne(lines, "grep");
  if (typeof grep.args.pattern !== "string" || grep.args.pattern.length === 0) {
    throw new Error("grep: pattern must be a non-empty string");
  }
  if (typeof grep.args.include !== "string" || grep.args.include.length === 0) {
    throw new Error("grep: include must be a non-empty string");
  }
  const glob = requireOne(lines, "glob");
  if (typeof glob.args.pattern !== "string" || glob.args.pattern.length === 0) {
    throw new Error("glob: pattern must be a non-empty string");
  }
  const edit = requireOne(lines, "edit");
  for (const key of ["filePath", "oldString", "newString"] as const) {
    if (typeof edit.args[key] !== "string" || (key === "filePath" && edit.args[key] === "")) {
      throw new Error(`edit: ${key} must be a string`);
    }
  }
  const write = requireOne(lines, "write");
  if (typeof write.args.filePath !== "string" || write.args.filePath.length === 0) {
    throw new Error("write: filePath must be a non-empty string");
  }
  if (typeof write.args.content !== "string") throw new Error("write: content must be a string");
  const bash = lines.filter((line) => line.tool === "bash");
  if (
    !bash.some((line) => line.args.workdir === "nested" && typeof line.args.command === "string")
  ) {
    throw new Error("bash: expected a call with workdir nested");
  }
  const task = requireOne(lines, "task");
  for (const key of ["description", "prompt", "subagent_type"] as const) {
    if (typeof task.args[key] !== "string" || task.args[key] === "") {
      throw new Error(`task: ${key} must be a non-empty string`);
    }
  }
  const child = bash.find((line) => line.sessionID !== task.sessionID);
  if (child === undefined) throw new Error("bash: expected a child session id distinct from task");
  const patched = requireOne(lines, "apply_patch");
  const text = patched.args.patchText;
  if (typeof text !== "string") throw new Error("apply_patch: patchText must be a string");
  for (const header of ["*** Add File:", "*** Update File:", "*** Delete File:", "*** Move to:"]) {
    if (!text.includes(header)) throw new Error(`apply_patch: missing ${header}`);
  }
  requireOne(lines, "probe_touch");
}

function parseCapture(text: string): BeforeLine[] {
  const lines = text.split("\n").filter((line) => line !== "");
  if (lines.length === 0) throw new Error("capture is empty");
  return lines.map((line, index) => {
    const parsed = BeforeLine.safeParse(JSON.parse(line));
    if (!parsed.success) {
      throw new Error(`line ${index + 1}: ${z.prettifyError(parsed.error).replaceAll("\n", " ")}`);
    }
    return parsed.data;
  });
}

function checkFile(): void {
  assertCapture(parseCapture(readFileSync(CAPTURE, "utf8")));
}

const PERMISSION = {
  bash: "allow",
  edit: "allow",
  read: "allow",
  grep: "allow",
  glob: "allow",
  task: "allow",
};

const HostBefore = z.object({
  kind: z.literal("before"),
  tool: z.string().min(1),
  sessionID: z.string().min(1),
  callID: z.string().min(1),
  args: z.record(z.string(), z.unknown()),
});

function beforeLines(logPath: string, hostVersion: string): BeforeLine[] {
  if (hostVersion !== HOST_VERSION) throw new Error(`host ${hostVersion} is not ${HOST_VERSION}`);
  return readFileSync(logPath, "utf8")
    .split("\n")
    .filter((line) => line !== "")
    .flatMap((line) => {
      const parsed: unknown = JSON.parse(line);
      const before = HostBefore.safeParse(parsed);
      if (!before.success) return [];
      const row: BeforeLine = { ...before.data, hostVersion: HOST_VERSION };
      return [row];
    });
}

async function captureSession(
  bin: string,
  cacheRoot: string,
  hostVersion: string,
  prompt: string,
  options: SessionOptions,
): Promise<BeforeLine[]> {
  using session = openSession(cacheRoot, { localPlugins: [PROBE_PLUGIN], ...options });
  const run = await runHost(bin, session, [prompt]);
  const lines = beforeLines(session.logPath, hostVersion);
  if (lines.length === 0) throw new Error(`${prompt}: no before lines\n${run.stderr}`);
  return lines;
}

function toolSession(files: Record<string, string>): SessionOptions {
  return {
    files,
    config: (root) => ({
      permission: PERMISSION,
      mcp: {
        probe: {
          type: "local",
          command: [process.execPath, MCP_SERVER],
          environment: { MCP_MARKER: join(root, "mcp-marker.txt") },
        },
      },
    }),
    scripts: (project) => ({
      "tool-capture": [
        { tool: "read", args: { filePath: join(project, "existing.txt") } },
        { tool: "grep", args: { pattern: "hello", path: join(project, "src"), include: "*.ts" } },
        { tool: "glob", args: { pattern: "*.ts", path: project } },
        {
          tool: "edit",
          args: {
            filePath: join(project, "existing.txt"),
            oldString: "old",
            newString: "new",
            replaceAll: false,
          },
        },
        {
          tool: "write",
          args: { filePath: join(project, "captured-write.txt"), content: "written\n" },
        },
        { tool: "bash", args: { command: "pwd", workdir: "nested", timeout: 5000 } },
        {
          tool: "task",
          args: {
            description: "child probe",
            prompt: "PROBE:tool-capture-child",
            subagent_type: "general",
          },
        },
        { tool: "probe_touch", args: { name: "captured" } },
      ],
      "tool-capture-child": [
        { tool: "bash", args: { command: "echo child", description: "child" } },
      ],
    }),
  };
}

/** gpt-* models offer apply_patch and drop edit/write, so the patch is its own session. */
function patchSession(files: Record<string, string>): SessionOptions {
  return {
    model: "gpt-5-probe",
    files,
    config: () => ({ permission: PERMISSION }),
    scripts: (project) => ({
      "tool-capture-patch": [{ tool: "apply_patch", args: { patchText: patch(project) } }],
    }),
  };
}

async function record(): Promise<void> {
  const pin = readJson(contractPaths().pin, PinSchema);
  const host = await resolveHostBinary(pin);
  if (host.version !== HOST_VERSION) {
    throw new Error(`host ${host.version} is not ${HOST_VERSION}`);
  }
  const cacheRoot = join(hostCacheDir(pin), "run-cache");
  mkdirSync(cacheRoot, { recursive: true });
  const files = {
    "existing.txt": "old\n",
    "gone.txt": "gone\n",
    "nested/keep.txt": "keep\n",
    "src/a.ts": "hello\n",
  };
  const tools = await captureSession(
    host.bin,
    cacheRoot,
    host.version,
    "PROBE:tool-capture",
    toolSession(files),
  );
  const patched = await captureSession(
    host.bin,
    cacheRoot,
    host.version,
    "PROBE:tool-capture-patch",
    patchSession(files),
  );
  const lines = [...tools, ...patched];
  try {
    assertCapture(lines);
  } catch (error) {
    const seen = lines.map((line) => `${line.tool} ${line.sessionID}`).join("\n");
    process.stderr.write(`${seen}\n`);
    throw error;
  }
  mkdirSync(dirname(CAPTURE), { recursive: true });
  writeFileSync(CAPTURE, `${lines.map((line) => JSON.stringify(line)).join("\n")}\n`);
  process.stdout.write(`wrote ${CAPTURE} (${lines.length} calls)\n`);
}

async function main(): Promise<void> {
  if (process.argv.includes("--check")) checkFile();
  else await record();
}

main().catch((error: unknown) => {
  process.stderr.write(`${error instanceof Error ? error.message : String(error)}\n`);
  process.exit(1);
});
