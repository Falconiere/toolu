import { expect, test } from "bun:test";
import { mkdir, mkdtemp, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { createToolAdviceStore } from "../tool-advice.ts";
import { createToolBeforeHandler } from "../tool-before.ts";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const tmpBase = process.env.TMPDIR ?? "/tmp";
const CALL = { tool: "bash", sessionID: "session-registry", callID: "call-registry" };

function registryModule(name: string, decision: object): string {
  return `export default { spec: "toolu@toolu", name: ${JSON.stringify(name)}, event: "tool/pre", run: () => Promise.resolve(${JSON.stringify(decision)}) };\n`;
}

async function registry(): Promise<{ root: string; configRoot: string; path: string }> {
  const root = await mkdtemp(join(tmpBase, "toolu-oc-registry-decision-"));
  const configRoot = join(root, "state");
  const path = join(configRoot, "toolu/pre-tools.d");
  await mkdir(path, { recursive: true });
  return { root, configRoot, path };
}

async function rejection(pending: Promise<unknown>): Promise<string> {
  try {
    await pending;
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  throw new Error("expected the tool call to be refused");
}

test("a residual registry ask refuses execution without storing approval", async () => {
  const { root, configRoot, path } = await registry();
  await writeFile(
    join(path, "toolu@toolu__ask.js"),
    registryModule("ask", { kind: "ask", reason: "registry asks for approval" }),
  );
  const before = createToolBeforeHandler({
    repoRoot: REPO_ROOT,
    configRoot,
    permissionContext: { cwd: root, projectRoot: root, worktree: root },
    selectedPluginSpecs: new Set(["toolu@toolu"]),
  });
  expect(await rejection(before(CALL, { args: { command: "echo safe" } }))).toBe(
    "registry asks for approval",
  );
});

test("a later registry deny wins over earlier advice and delivers no stale advice", async () => {
  const { root, configRoot, path } = await registry();
  await writeFile(
    join(path, "toolu@toolu__a.js"),
    registryModule("a", { kind: "advisory", message: "earlier advice" }),
  );
  await writeFile(
    join(path, "toolu@toolu__b.js"),
    registryModule("b", { kind: "deny", reason: "later denial" }),
  );
  const advice = createToolAdviceStore();
  const before = createToolBeforeHandler(
    {
      repoRoot: REPO_ROOT,
      configRoot,
      permissionContext: { cwd: root, projectRoot: root, worktree: root },
      selectedPluginSpecs: new Set(["toolu@toolu"]),
    },
    advice,
  );
  expect(await rejection(before(CALL, { args: { command: "echo safe" } }))).toBe("later denial");
  const output = { title: "bash", output: "unrelated result", metadata: {} };
  await advice.after({ ...CALL, args: {} }, output);
  expect(output.output).toBe("unrelated result");
});
