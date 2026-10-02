import { expect, test } from "bun:test";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createDenyAllToolBefore, createToolBeforeHandler, mapToolCall } from "../tool-before.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";
const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const CTX = { cwd: "/work/app", projectRoot: "/work/app", worktree: "/work/app" };
const CALL = { sessionID: "ses_1", callID: "call_1" };

async function project(mode: "block" | "ask"): Promise<{ root: string; envPath: string }> {
  const root = await mkdtemp(join(tmpBase, "toolu-oc-before-"));
  const envPath = join(root, ".env");
  await writeFile(envPath, "SECRET=1\n", "utf8");
  await mkdir(join(root, ".opencode"), { recursive: true });
  await writeFile(
    join(root, ".opencode/toolu.config.json"),
    JSON.stringify({ version: 1, gates: { protectedFiles: { mode } } }),
    "utf8",
  );
  return { root, envPath };
}

/** The message a promise rejects with; fails the test when it resolves instead. */
async function rejection(pending: Promise<unknown>): Promise<string> {
  try {
    await pending;
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  throw new Error("expected the tool call to be refused");
}

function handlerFor(root: string): ReturnType<typeof createToolBeforeHandler> {
  return createToolBeforeHandler({
    repoRoot: REPO_ROOT,
    configRoot: join(root, ".opencode/toolu/state"),
    permissionContext: { cwd: root, projectRoot: root, worktree: root },
  });
}

test("maps the host's bash, edit and write args to core requests", () => {
  const bash = mapToolCall({ tool: "bash", ...CALL }, { command: "ls", description: "list" }, CTX);
  expect(bash).toEqual({
    kind: "request",
    request: {
      session_id: "ses_1",
      tool_use_id: "call_1",
      cwd: "/work/app",
      tool_name: "Bash",
      tool_input: { command: "ls" },
    },
  });
  const edit = mapToolCall(
    { tool: "edit", ...CALL },
    { filePath: "/work/app/a.ts", oldString: "a", newString: "b" },
    CTX,
  );
  expect(edit.kind === "request" && edit.request.tool_name).toBe("Edit");
  expect(edit.kind === "request" && edit.request.tool_input).toEqual({
    file_path: "/work/app/a.ts",
  });
  const write = mapToolCall(
    { tool: "write", ...CALL },
    { filePath: "/work/app/b.ts", content: "x" },
    CTX,
  );
  expect(write.kind === "request" && write.request.tool_name).toBe("Write");
});

test("skips tools outside today's coverage and denies gated tools with invalid args", () => {
  expect(mapToolCall({ tool: "read", ...CALL }, { filePath: "/a" }, CTX)).toEqual({
    kind: "skip",
  });
  expect(mapToolCall({ tool: "fixture_echo", ...CALL }, {}, CTX)).toEqual({ kind: "skip" });
  const noCommand = mapToolCall({ tool: "bash", ...CALL }, { description: "x" }, CTX);
  expect(noCommand.kind).toBe("deny");
  const badPath = mapToolCall({ tool: "edit", ...CALL }, { filePath: 5 }, CTX);
  expect(badPath.kind).toBe("deny");
  expect(mapToolCall({ tool: "write", ...CALL }, null, CTX).kind).toBe("deny");
});

test("a protected .env edit or write throws the gate reason before the file changes", async () => {
  const { root, envPath } = await project("block");
  const before = handlerFor(root);
  const edit = before(
    { tool: "edit", ...CALL },
    { args: { filePath: envPath, oldString: "1", newString: "2" } },
  );
  expect(await rejection(edit)).toMatch(/protected/i);
  const write = before({ tool: "write", ...CALL }, { args: { filePath: envPath, content: "X" } });
  expect(await rejection(write)).toMatch(/protected/i);
  expect(await readFile(envPath, "utf8")).toBe("SECRET=1\n");
});

test("an allowed bash call and an uncovered tool pass through", async () => {
  const { root } = await project("block");
  const before = handlerFor(root);
  await before({ tool: "bash", ...CALL }, { args: { command: "echo ok", description: "x" } });
  await before({ tool: "read", ...CALL }, { args: { filePath: join(root, ".env") } });
});

test("a gate ask decision fails closed: the host has no ask channel", async () => {
  const { root, envPath } = await project("ask");
  const edit = handlerFor(root)(
    { tool: "edit", ...CALL },
    { args: { filePath: envPath, oldString: "1", newString: "2" } },
  );
  expect(await rejection(edit)).toMatch(/protected/i);
});

test("invalid args for a gated tool throw before any gate runs", async () => {
  const { root } = await project("block");
  const call = handlerFor(root)({ tool: "bash", ...CALL }, { args: {} });
  expect(await rejection(call)).toMatch("toolu: bash call missing a string command");
});

test("a missing core plugin tree denies every tool", async () => {
  const before = createToolBeforeHandler({
    repoRoot: await mkdtemp(join(tmpBase, "toolu-oc-empty-")),
    configRoot: await mkdtemp(join(tmpBase, "toolu-oc-empty-state-")),
    permissionContext: CTX,
  });
  expect(await rejection(before({ tool: "read", ...CALL }, { args: {} }))).toMatch(
    /core plugin manifest missing/,
  );
});

test("the deny-all handler refuses every tool with its reason", async () => {
  const before = createDenyAllToolBefore("toolu: not ready: probe");
  expect(await rejection(before({ tool: "read", ...CALL }, { args: {} }))).toMatch(
    "toolu: not ready: probe",
  );
  expect(await rejection(before({ tool: "bash", ...CALL }, { args: { command: "ls" } }))).toMatch(
    "toolu: not ready: probe",
  );
});
