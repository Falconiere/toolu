import { expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { z } from "zod";
import { createToolAdviceStore } from "../tool-advice.ts";
import { createToolPostHandler } from "../tool-post.ts";

const REPO_ROOT = join(import.meta.dir, "../../../../..");
const tmpBase = process.env.TMPDIR ?? "/tmp";

/** A temp git project whose post registry holds a module recording each `tool_response`. */
async function project(): Promise<{ root: string; trace: string }> {
  const root = await mkdtemp(join(tmpBase, "toolu-oc-post-response-"));
  execFileSync("git", ["init", "-q"], { cwd: root });
  const registry = join(root, ".opencode/toolu/state/toolu/post-tools.d");
  await mkdir(registry, { recursive: true });
  const trace = join(root, "response.jsonl");
  await writeFile(
    join(registry, "fixture@toolu__response.js"),
    `import { appendFileSync } from "node:fs";
export default {
  spec: "fixture@toolu", name: "response", event: "tool/post",
  run(event) {
    appendFileSync(${JSON.stringify(trace)}, JSON.stringify(event.toolOutput) + "\\n");
    return Promise.resolve({ kind: "allow" });
  }
};\n`,
  );
  return { root, trace };
}

function options(root: string) {
  return {
    repoRoot: REPO_ROOT,
    configRoot: join(root, ".opencode/toolu/state"),
    userConfigRoot: join(root, ".xdg/opencode"),
    permissionContext: { cwd: root, projectRoot: root, worktree: root },
    selectedPluginSpecs: new Set(["toolu@toolu", "fixture@toolu"]),
    env: { ...process.env, TOOLU_HOST_OVERRIDE: "opencode" },
  };
}

function bash(id: string) {
  return {
    tool: "bash",
    sessionID: "session-response",
    callID: id,
    args: { command: "bun run test" },
  };
}

test("post modules see the host's result text as tool_response.output; the shell exit is unchanged", async () => {
  const { root, trace } = await project();
  const post = createToolPostHandler(options(root), createToolAdviceStore());
  const path = join(root, "read.ts");
  await writeFile(path, "export const read = 1;\n");
  const read = {
    tool: "read",
    sessionID: "session-response",
    callID: "read",
    args: { filePath: path },
  };
  const readOutput = {
    title: "read",
    output: "<file>\n00001| export const read = 1;\n</file>",
    metadata: { truncated: false },
  };
  await post.after(read, readOutput);
  await post.after(bash("exit-3"), {
    title: "bash",
    output: "ran exit-3\n",
    metadata: { exit: 3 },
  });
  await post.after(bash("exit-0"), {
    title: "bash",
    output: "ran exit-0\n",
    metadata: { exit: 0 },
  });
  const Response = z.object({
    output: z.string(),
    interrupted: z.boolean(),
    metadata: z.record(z.string(), z.unknown()),
  });
  const seen = (await readFile(trace, "utf8"))
    .trim()
    .split("\n")
    .map((line) => Response.parse(JSON.parse(line)));
  expect(seen).toEqual([
    { output: readOutput.output, interrupted: false, metadata: { truncated: false } },
    { output: "ran exit-3\n", interrupted: false, metadata: { exit_code: 3 } },
    { output: "ran exit-0\n", interrupted: false, metadata: { exit_code: 0 } },
  ]);
  post.clear();
});
