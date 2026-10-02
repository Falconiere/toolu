import { expect, test } from "bun:test";
import { existsSync, readFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { createToolBeforeHandler, mapToolCall } from "../tool-before.ts";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const CALL = { sessionID: "ses_1", callID: "call_1" };

async function rejection(pending: Promise<unknown>): Promise<string> {
  try {
    await pending;
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  throw new Error("expected the tool call to be refused");
}

test("task mapping preserves model, effort and call identity and rejects invalid optional values", () => {
  const ctx = { cwd: "/work/app", projectRoot: "/work/app", worktree: "/work/app" };
  const args = {
    description: "inspect",
    prompt: "inspect the project",
    subagent_type: "explore",
    model: "wrong-model",
    reasoning_effort: "high",
  };
  expect(mapToolCall({ tool: "task", ...CALL }, args, ctx)).toEqual({
    kind: "request",
    request: {
      session_id: CALL.sessionID,
      tool_use_id: CALL.callID,
      cwd: ctx.cwd,
      tool_name: "Task",
      tool_input: args,
    },
  });
  expect(mapToolCall({ tool: "task", ...CALL }, { ...args, model: 42 }, ctx).kind).toBe("deny");
  expect(
    mapToolCall({ tool: "task", ...CALL }, { ...args, reasoning_effort: false }, ctx).kind,
  ).toBe("deny");
});

test("a task model mismatch blocks once and records one real git-project delegation", async () => {
  using sb = createSandbox({ git: true, branch: "task-branch" });
  const root = sb.project;
  const marker = sb.path("registry-ran");
  sb.write(
    "state/toolu/pre-tools.d/fixture@toolu__ask.js",
    `import { appendFileSync } from "node:fs"; export default { spec: "fixture@toolu", name: "ask", event: "tool/pre", run() { appendFileSync(${JSON.stringify(marker)}, "x"); return { kind: "ask", reason: "registry asks" }; } };`,
  );
  sb.write(".opencode/toolu.config.json", {
    version: 1,
    gates: { agentTier: { mode: "block" } },
  });
  sb.write("ledgers/task-branch.json", {
    next: "inspect",
    steps: [{ id: "inspect", status: "running", model: "expected-model" }],
  });
  const options = {
    repoRoot: REPO_ROOT,
    configRoot: sb.path("state"),
    permissionContext: { cwd: root, projectRoot: root, worktree: root },
    env: { LEDGER_DIR: sb.path("ledgers"), TELEMETRY_DIR: sb.path("telemetry") },
  };
  const before = createToolBeforeHandler({
    ...options,
    selectedPluginSpecs: new Set(["fixture@toolu"]),
  });
  const args = {
    description: "inspect",
    prompt: "inspect the project",
    subagent_type: "explore",
    model: "wrong-model",
    reasoning_effort: "high",
  };
  expect(await rejection(before({ tool: "task", ...CALL }, { args }))).toContain(
    'plan step "inspect" expects model tier "expected-model"',
  );
  expect(readFileSync(marker, "utf8")).toBe("x");
  const lines = sb.read("telemetry/task-branch.jsonl").trim().split("\n");
  expect(lines).toHaveLength(1);
  expect(JSON.parse(lines[0] ?? "{}")).toMatchObject({
    event: "delegation",
    model: "wrong-model",
    reasoning_effort: "high",
    step_id: "inspect",
    step_model: "expected-model",
  });
  const inherited = createToolBeforeHandler({ ...options, selectedPluginSpecs: new Set() });
  await inherited(
    { tool: "task", ...CALL },
    {
      args: {
        description: args.description,
        prompt: args.prompt,
        subagent_type: args.subagent_type,
      },
    },
  );
  expect(readFileSync(marker, "utf8")).toBe("x");
});

test("MCP calls use the standalone policy and skip the ordinary registry walk", async () => {
  using sb = createSandbox();
  const root = sb.project;
  const marker = sb.path("registry-ran");
  sb.write("opencode.json", { mcp: { probe: {} } });
  sb.write(
    "state/toolu/pre-tools.d/fixture@toolu__deny.js",
    `import { appendFileSync } from "node:fs"; export default { spec: "fixture@toolu", name: "deny", event: "tool/pre", run() { appendFileSync(${JSON.stringify(marker)}, "x"); return { kind: "deny", reason: "registry denied" }; } };`,
  );
  const before = createToolBeforeHandler({
    repoRoot: REPO_ROOT,
    configRoot: sb.path("state"),
    permissionContext: { cwd: root, projectRoot: root, worktree: root },
    selectedPluginSpecs: new Set(["fixture@toolu"]),
  });
  await before({ tool: "probe_touch", ...CALL }, { args: { name: "okay" } });
  expect(existsSync(marker)).toBe(false);
  sb.write(".opencode/toolu.config.json", {
    version: 1,
    mcp: { probe: false },
    gates: { mcpBlocker: { mode: "block" } },
  });
  expect(await rejection(before({ tool: "probe_touch", ...CALL }, { args: {} }))).toContain(
    "mcp.probe=false",
  );
  expect(existsSync(marker)).toBe(false);
});
