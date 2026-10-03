import { expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { copyFile, mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { z } from "zod";
import { createGateDecider, type PermissionEvaluateHandlerOptions } from "../evaluate.ts";
import { createToolAdviceStore } from "../tool-advice.ts";
import { createToolPostHandler } from "../tool-post.ts";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const tmpBase = process.env.TMPDIR ?? "/tmp";
const Row = z.object({ path: z.string(), operation: z.string() });

async function project(): Promise<{ root: string; options: PermissionEvaluateHandlerOptions }> {
  const root = await mkdtemp(join(tmpBase, "toolu-oc-post-"));
  execFileSync("git", ["init", "-q"], { cwd: root });
  await mkdir(join(root, ".opencode/toolu/state/toolu/post-tools.d"), { recursive: true });
  return {
    root,
    options: {
      repoRoot: REPO_ROOT,
      configRoot: join(root, ".opencode/toolu/state"),
      userConfigRoot: join(root, ".xdg/opencode"),
      permissionContext: { cwd: root, projectRoot: root, worktree: root },
      selectedPluginSpecs: new Set(["toolu@toolu", "fixture@toolu"]),
      env: { ...process.env, TOOLU_HOST_OVERRIDE: "opencode" },
    },
  };
}

async function traceModule(root: string, trace: string): Promise<void> {
  const path = join(root, ".opencode/toolu/state/toolu/post-tools.d/fixture@toolu__trace.js");
  await writeFile(
    path,
    `import { appendFileSync } from "node:fs";
export default {
  spec: "fixture@toolu", name: "trace", event: "tool/post",
  run(event, ctx) {
    appendFileSync(${JSON.stringify(trace)}, JSON.stringify({
      path: event.toolInput.file_path ?? "", operation: ctx.edit?.operation ?? "none"
    }) + "\\n");
    return Promise.resolve({ kind: "advisory", message: "checked " + (event.toolInput.file_path ?? event.toolName) });
  }
};\n`,
  );
}

async function rows(trace: string): Promise<z.infer<typeof Row>[]> {
  return (await readFile(trace, "utf8"))
    .trim()
    .split("\n")
    .map((line) => Row.parse(JSON.parse(line)));
}

function result(metadata: unknown = {}): { title: string; output: string; metadata: unknown } {
  return { title: "tool result", output: "original result", metadata };
}

function qualityCall(id: string) {
  return {
    tool: "bash",
    sessionID: "session-quality",
    callID: id,
    args: { command: "bun run test" },
  };
}

test("a completed patch checks each add, move side and delete once; duplicate after is inert", async () => {
  const { root, options } = await project();
  const trace = join(root, "trace.jsonl");
  await traceModule(root, trace);
  const advice = createToolAdviceStore();
  const post = createToolPostHandler(options, advice);
  const source = join(root, "source.ts");
  const target = join(root, "moved.ts");
  const added = join(root, "added.ts");
  const deleted = join(root, "deleted.ts");
  await writeFile(target, "moved\n");
  await writeFile(added, "added\n");
  const patchText = `*** Begin Patch\n*** Add File: ${added}\n+added\n*** Update File: ${source}\n*** Move to: ${target}\n*** Delete File: ${deleted}\n*** End Patch`;
  const call = {
    tool: "apply_patch",
    sessionID: "session-a",
    callID: "call-a",
    args: { patchText },
  };
  post.begin(call);
  const output = result();
  await post.after(call, output);
  expect((await rows(trace)).map(({ path, operation }) => [path, operation])).toEqual([
    [added, "add"],
    [source, "update"],
    [target, "move"],
    [deleted, "delete"],
  ]);
  expect(output.output).toContain("original result");
  expect(output.output).toContain(`checked ${target}`);
  await post.after(call, output);
  expect((await rows(trace)).length).toBe(4);
  post.clear();
});

test("the same call ID on a different tool still runs post checks", async () => {
  const { root, options } = await project();
  const trace = join(root, "trace.jsonl");
  await traceModule(root, trace);
  const post = createToolPostHandler(options, createToolAdviceStore());
  const path = join(root, "written.ts");
  await writeFile(path, "written\n");
  const call = {
    tool: "write",
    sessionID: "session-reused",
    callID: "same-id",
    args: { filePath: path, content: "written\n" },
  };
  await post.after(call, result());
  const read = { ...call, tool: "read", args: { filePath: path } };
  const readOutput = result();
  await post.after(read, readOutput);
  expect((await rows(trace)).length).toBe(2);
  expect(readOutput.output).toContain(`checked ${path}`);
  post.clear();
});

test("a non-text host result reports failed delivery without recording a quality pass", async () => {
  const { root, options } = await project();
  const advice = createToolAdviceStore();
  const post = createToolPostHandler(options, advice);
  const call = qualityCall("non-text");
  advice.record(call, "pre advice");
  const output = result({ exit: 0 });
  Object.defineProperty(output, "output", { value: 42, writable: true });
  await post.after(call, output);
  expect(output.output).toContain("Post-tool checks failed to run: output is not text");
  expect(output.output).toContain("pre advice");
  expect(existsSync(join(root, ".opencode/tmp/quality-gate-status.json"))).toBe(false);
  post.clear();
});

test("shell quality failure blocks later commit and push; unknown outcomes cannot clear it", async () => {
  const { root, options } = await project();
  const advice = createToolAdviceStore();
  const post = createToolPostHandler(options, advice);
  const failing = result({ exit: 3 });
  await post.after(qualityCall("fail"), failing);
  const gatePath = join(root, ".opencode/tmp/quality-gate-status.json");
  const failed: unknown = JSON.parse(await readFile(gatePath, "utf8"));
  expect(failed).toMatchObject({ status: "failing" });
  expect(failing.output).toContain("Global quality gate failing");
  const decider = createGateDecider(options);
  if (!decider.ok) throw new Error(decider.reason);
  const decisions = await Promise.all(
    ["git commit -m test", "git push origin HEAD"].map((command) =>
      decider.decide({ tool_name: "Bash", tool_input: { command }, cwd: root }),
    ),
  );
  expect(decisions.map((decision) => decision.kind)).toEqual(["deny", "deny"]);
  const outcomes = [{}, { exit: 0, interrupted: true }, { exit: "0" }];
  const unknownResults = await Promise.all(
    outcomes.map(async (metadata, index) => {
      const unknown = result(metadata);
      await post.after(qualityCall(`unknown-${index}`), unknown);
      return unknown;
    }),
  );
  expect(
    unknownResults.every((unknown) => unknown.output.includes("quality state was left unchanged")),
  ).toBe(true);
  const unchanged: unknown = JSON.parse(await readFile(gatePath, "utf8"));
  expect(unchanged).toMatchObject({ status: "failing" });
  await post.after(qualityCall("pass"), result({ exit: 0 }));
  const passed: unknown = JSON.parse(await readFile(gatePath, "utf8"));
  expect(passed).toMatchObject({ status: "passing" });
  post.clear();
});

test("post diagnostic and pre advice stay on their matching completed call", async () => {
  const { root, options } = await project();
  const trace = join(root, "trace.jsonl");
  await traceModule(root, trace);
  const advice = createToolAdviceStore();
  const post = createToolPostHandler(options, advice);
  const call = {
    tool: "write",
    sessionID: "session-one",
    callID: "shared",
    args: { filePath: join(root, "one.ts"), content: "one\n" },
  };
  await writeFile(call.args.filePath, call.args.content);
  advice.record(call, "pre advice");
  const output = result();
  await post.after(call, output);
  expect(output.output).toContain("checked ");
  expect(output.output).toContain("pre advice");
  const other = {
    ...call,
    sessionID: "session-two",
    args: { ...call.args, filePath: join(root, "two.ts") },
  };
  await writeFile(other.args.filePath, other.args.content);
  const second = result();
  await post.after(other, second);
  expect(second.output).toContain("checked ");
  expect(second.output).not.toContain("pre advice");
  expect((await rows(trace)).length).toBe(2);
  post.clear();
});

test("a registry post block reports failure after the write without replacing its result", async () => {
  const { root, options } = await project();
  const modulePath = join(root, ".opencode/toolu/state/toolu/post-tools.d/fixture@toolu__block.js");
  await writeFile(
    modulePath,
    `export default { spec: "fixture@toolu", name: "block", event: "tool/post",
      run: () => Promise.resolve({ kind: "post_block", reason: "fixture quality violation" }) };\n`,
  );
  const post = createToolPostHandler(options, createToolAdviceStore());
  const path = join(root, "written.ts");
  await writeFile(path, "written\n");
  const call = {
    tool: "write",
    sessionID: "session-block",
    callID: "call-block",
    args: { filePath: path, content: "written\n" },
  };
  const output = result();
  await post.after(call, output);
  expect(output.output).toContain("original result");
  expect(output.output).toContain("[toolu post-check after execution]");
  expect(output.output).toContain("fixture quality violation");
  expect(await readFile(path, "utf8")).toBe("written\n");
  post.clear();
});

test("a registry module crash reaches the model as a post-check warning", async () => {
  const { root, options } = await project();
  const modulePath = join(root, ".opencode/toolu/state/toolu/post-tools.d/fixture@toolu__crash.js");
  await writeFile(
    modulePath,
    `export default { spec: "fixture@toolu", name: "crash", event: "tool/post",
      run: () => { throw new Error("fixture post crash"); } };\n`,
  );
  const post = createToolPostHandler(options, createToolAdviceStore());
  const path = join(root, "written.ts");
  await writeFile(path, "written\n");
  const output = result();
  await post.after(
    {
      tool: "write",
      sessionID: "session-crash",
      callID: "call-crash",
      args: { filePath: path, content: "written\n" },
    },
    output,
  );
  expect(output.output).toContain("original result");
  expect(output.output).toContain("Post-tool check warning");
  expect(output.output).toContain("fixture post crash");
  post.clear();
});

test("the shipped TypeScript quality bundle records and clears a per-file gate entry", async () => {
  const { root, options } = await project();
  await writeFile(join(root, "package.json"), '{"name":"post-quality","private":true}\n');
  await writeFile(join(root, "bun.lock"), "lock marker\n");
  await writeFile(join(root, "tsconfig.json"), "{}\n");
  execFileSync("git", ["add", "tsconfig.json"], { cwd: root });
  await copyFile(
    join(REPO_ROOT, "plugins/ts-quality/hooks/dist/post-tool-use.js"),
    join(root, ".opencode/toolu/state/toolu/post-tools.d/ts-quality@toolu__ts-quality.js"),
  );
  const post = createToolPostHandler(
    { ...options, selectedPluginSpecs: new Set(["toolu@toolu", "ts-quality@toolu"]) },
    createToolAdviceStore(),
  );
  const path = join(root, "source.ts");
  const call = (id: string, content: string) => ({
    tool: "write",
    sessionID: "session-ts-quality",
    callID: id,
    args: { filePath: path, content },
  });
  const bad = 'console.log("bad");\n';
  await writeFile(path, bad);
  const badOutput = result();
  await post.after(call("bad", bad), badOutput);
  expect(badOutput.output).toContain("QUALITY VIOLATION");
  const gatePath = join(root, ".opencode/tmp/quality-gate-status.json");
  const failed: unknown = JSON.parse(await readFile(gatePath, "utf8"));
  expect(failed).toMatchObject({ status: "failing", file: path });
  const good = "export const answer = 42;\n";
  await writeFile(path, good);
  const cancelled = result({ interrupted: true });
  await post.after(call("cancelled", good), cancelled);
  expect(cancelled.output).toContain("post-edit quality state was left unchanged");
  const stillFailed: unknown = JSON.parse(await readFile(gatePath, "utf8"));
  expect(stillFailed).toMatchObject({ status: "failing", file: path });
  await post.after(call("good", good), result());
  const passed: unknown = JSON.parse(await readFile(gatePath, "utf8"));
  expect(passed).toMatchObject({ status: "passing" });
  post.clear();
});
