import { expect, test } from "bun:test";
import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { BUILTIN_MODULES } from "../../../../../plugins/toolu/hooks/src/pre-tools/builtins.ts";
import { createGateDecider, nativeGates } from "../evaluate.ts";
import { createDenyAllToolBefore, createToolBeforeHandler, mapToolCall } from "../tool-before.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";
const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const CTX = { cwd: "/work/app", projectRoot: "/work/app", worktree: "/work/app" };
const CALL = { sessionID: "ses_1", callID: "call_1" };

test("OpenCode uses the same ordered nine native gates as the core hook", () => {
  expect(nativeGates(join(REPO_ROOT, "plugins/toolu")).map((gate) => gate.name)).toEqual(
    BUILTIN_MODULES,
  );
  expect(BUILTIN_MODULES).toHaveLength(9);
});

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
    old_string: "a",
    new_string: "b",
  });
  const write = mapToolCall(
    { tool: "write", ...CALL },
    { filePath: "/work/app/b.ts", content: "x" },
    CTX,
  );
  expect(write.kind === "request" && write.request.tool_input).toEqual({
    file_path: "/work/app/b.ts",
    content: "x",
  });
});

test("skips unrelated tools and denies gated tools with invalid args", () => {
  expect(mapToolCall({ tool: "webfetch", ...CALL }, { url: "https://example.com" }, CTX)).toEqual({
    kind: "skip",
  });
  expect(mapToolCall({ tool: "fixture_echo", ...CALL }, {}, CTX)).toEqual({ kind: "skip" });
  expect(mapToolCall({ tool: "probe_touch", ...CALL }, { name: "x" }, CTX)).toEqual({
    kind: "skip",
  });
  const noCommand = mapToolCall({ tool: "bash", ...CALL }, { description: "x" }, CTX);
  expect(noCommand.kind).toBe("deny");
  const badPath = mapToolCall({ tool: "edit", ...CALL }, { filePath: 5 }, CTX);
  expect(badPath.kind).toBe("deny");
  expect(mapToolCall({ tool: "write", ...CALL }, null, CTX).kind).toBe("deny");
  expect(
    mapToolCall({ tool: "apply_patch", ...CALL }, { patchText: "not a patch" }, CTX).kind,
  ).toBe("deny");
  expect(
    mapToolCall({ tool: "bash", sessionID: "", callID: "c" }, { command: "ls" }, CTX).kind,
  ).toBe("deny");
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

test("an allowed bash call, a read, and an unrelated tool pass through", async () => {
  const { root } = await project("block");
  const before = handlerFor(root);
  await before({ tool: "bash", ...CALL }, { args: { command: "echo ok", description: "x" } });
  await before({ tool: "read", ...CALL }, { args: { filePath: join(root, ".env") } });
  await before({ tool: "webfetch", ...CALL }, { args: { url: "https://example.com" } });
  await before({ tool: "probe_touch", ...CALL }, { args: null });
});

test("a guardrail ask degrades to deny before the file changes", async () => {
  const { root, envPath } = await project("ask");
  const decider = createGateDecider({
    repoRoot: REPO_ROOT,
    configRoot: join(root, ".opencode/toolu/state"),
    permissionContext: { cwd: root, projectRoot: root, worktree: root },
  });
  if (!decider.ok) throw new Error(decider.reason);
  const editRequest = mapToolCall(
    { tool: "edit", ...CALL },
    { filePath: envPath, oldString: "1", newString: "2" },
    { cwd: root, projectRoot: root, worktree: root },
  );
  if (editRequest.kind !== "request") throw new Error("edit was not mapped");
  const decision = await decider.decide(editRequest.request);
  expect(decision.kind).toBe("deny");
  if (decision.kind !== "deny") return;
  const edit = handlerFor(root)(
    { tool: "edit", ...CALL },
    { args: { filePath: envPath, oldString: "1", newString: "2" } },
  );
  expect(await rejection(edit)).toBe(decision.reason);
  expect(await readFile(envPath, "utf8")).toBe("SECRET=1\n");
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

test("the longest MCP server prefix wins", () => {
  const mapped = mapToolCall({ tool: "probe_extra_touch", ...CALL }, { name: "x" }, CTX, [
    "probe",
    "probe_extra",
  ]);
  expect(mapped.kind === "request" && mapped.request.tool_name).toBe("mcp__probe_extra__touch");
});

test("a protected .env patch throws and writes nothing", async () => {
  const { root, envPath } = await project("block");
  const added = join(root, "added.txt");
  const patch = [
    "*** Begin Patch",
    `*** Add File: ${added}`,
    "+x",
    `*** Update File: ${envPath}`,
    "*** End Patch",
    "",
  ].join("\n");
  const call = handlerFor(root)({ tool: "apply_patch", ...CALL }, { args: { patchText: patch } });
  expect(await rejection(call)).toMatch(/protected/i);
  expect(await readFile(envPath, "utf8")).toBe("SECRET=1\n");
  expect(existsSync(added)).toBe(false);
});

test("opencode.json mcp keys gate probe_touch and a missing mcp object does not", async () => {
  const listed = await mkdtemp(join(tmpBase, "toolu-oc-mcp-"));
  await writeFile(
    join(listed, "opencode.json"),
    JSON.stringify({ mcp: { probe: { type: "local", command: ["true"] } } }),
  );
  const denied = createToolBeforeHandler({
    repoRoot: REPO_ROOT,
    configRoot: join(listed, "state"),
    permissionContext: { cwd: listed, projectRoot: listed, worktree: listed },
  });
  expect(await rejection(denied({ tool: "probe_touch", ...CALL }, { args: null }))).toMatch(
    /^toolu:/,
  );

  const open = await mkdtemp(join(tmpBase, "toolu-oc-nomcp-"));
  await writeFile(join(open, "opencode.json"), "{}\n");
  const passed = createToolBeforeHandler({
    repoRoot: REPO_ROOT,
    configRoot: join(open, "state"),
    permissionContext: { cwd: open, projectRoot: open, worktree: open },
  });
  await passed({ tool: "probe_touch", ...CALL }, { args: null });

  const broken = await mkdtemp(join(tmpBase, "toolu-oc-badmcp-"));
  await writeFile(join(broken, "opencode.json"), "{", "utf8");
  const skipped = createToolBeforeHandler({
    repoRoot: REPO_ROOT,
    configRoot: join(broken, "state"),
    permissionContext: { cwd: broken, projectRoot: broken, worktree: broken },
  });
  await skipped({ tool: "probe_touch", ...CALL }, { args: null });
});

test("OpenCode dispatch runs a selected real registry bundle but skips its stale disabled copy", async () => {
  const root = await mkdtemp(join(tmpBase, "toolu-oc-registry-"));
  const configRoot = join(root, "state");
  const registry = join(configRoot, "toolu/pre-tools.d");
  await mkdir(registry, { recursive: true });
  await copyFile(
    join(REPO_ROOT, "plugins/ast-grep/hooks/dist/search-nudge.js"),
    join(registry, "ast-grep@toolu__search-nudge.js"),
  );
  const options = {
    repoRoot: REPO_ROOT,
    configRoot,
    permissionContext: { cwd: root, projectRoot: root, worktree: root },
  };
  const request = {
    tool_name: "Grep",
    tool_input: { pattern: "class Foo" },
    session_id: "session-registry",
    tool_use_id: "call-registry",
    cwd: root,
  };
  const selected = createGateDecider({
    ...options,
    selectedPluginSpecs: new Set(["toolu@toolu", "ast-grep@toolu"]),
  });
  const disabled = createGateDecider({
    ...options,
    selectedPluginSpecs: new Set(["toolu@toolu"]),
  });
  if (!selected.ok || !disabled.ok) throw new Error("core decider was not ready");
  expect((await selected.decide(request)).kind).toBe("advisory");
  expect(await disabled.decide(request)).toEqual({ kind: "allow" });
});
