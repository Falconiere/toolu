/** Real ts-quality bundle through OpenCode's completed-tool adapter (#352). */
import { expect, test } from "bun:test";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { z } from "zod";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { createGateDecider, type PermissionEvaluateHandlerOptions } from "../evaluate.ts";
import { createToolAdviceStore } from "../tool-advice.ts";
import { createToolPostHandler } from "../tool-post.ts";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const MODULE = join(REPO_ROOT, "plugins/ts-quality/hooks/dist/post-tool-use.js");
const Gate = z.object({
  status: z.string(),
  entries: z
    .record(z.string(), z.object({ source: z.string(), violations: z.string() }))
    .optional(),
});

function options(
  sb: Sandbox,
  projectRoot = sb.project,
  selected = true,
): PermissionEvaluateHandlerOptions {
  const configRoot = join(projectRoot, ".opencode/toolu/state");
  const registry = join(configRoot, "toolu/post-tools.d");
  mkdirSync(registry, { recursive: true });
  copyFileSync(MODULE, join(registry, "ts-quality@toolu__ts-quality.js"));
  return {
    repoRoot: REPO_ROOT,
    configRoot,
    userConfigRoot: join(sb.root, ".xdg/opencode"),
    permissionContext: { cwd: projectRoot, projectRoot, worktree: projectRoot },
    selectedPluginSpecs: new Set(selected ? ["toolu@toolu", "ts-quality@toolu"] : ["toolu@toolu"]),
    env: { ...process.env, TOOLU_HOST_OVERRIDE: "opencode" },
  };
}

function tsProject(sb: Sandbox): void {
  sb.write("package.json", '{"name":"ts-quality-host","private":true}\n');
  sb.write("bun.lock", "lock marker\n");
  sb.write("tsconfig.json", "{}\n");
  sb.git("add", "tsconfig.json");
}

function result(): { title: string; output: string; metadata: object } {
  return { title: "tool result", output: "original result", metadata: {} };
}

function gate(root: string): z.infer<typeof Gate> {
  return Gate.parse(
    JSON.parse(readFileSync(join(root, ".opencode/tmp/quality-gate-status.json"), "utf8")),
  );
}

function patch(sb: Sandbox): string {
  return [
    "*** Begin Patch",
    `*** Update File: ${sb.path("old.tsx")}`,
    `*** Move to: ${sb.path("moved.tsx")}`,
    "@@",
    '-console.log("bad");',
    "+function f() { try { return 1; } catch {} }",
    `*** Delete File: ${sb.path("removed.ts")}`,
    `*** Add File: ${sb.path("added.ts")}`,
    '+console.log("new bad");',
    `*** Add File: ${sb.path("notes.md")}`,
    "+unrelated",
    "*** End Patch",
  ].join("\n");
}

test("two TypeScript patch violations persist while move and delete clear old entries", async () => {
  using sb = createSandbox({ git: true });
  expect(Bun.which("ast-grep")).not.toBeNull();
  tsProject(sb);
  const selected = options(sb);
  const post = createToolPostHandler(selected, createToolAdviceStore());
  await Promise.all(
    ["old.tsx", "removed.ts"].map(async (name) => {
      const path = sb.path(name);
      const bad = 'console.log("bad");\n';
      sb.write(name, bad);
      await post.after(
        { tool: "write", sessionID: "patch", callID: name, args: { filePath: path, content: bad } },
        result(),
      );
    }),
  );
  expect(Object.keys(gate(sb.project).entries ?? {}).toSorted()).toEqual(
    [sb.path("old.tsx"), sb.path("removed.ts")].toSorted(),
  );

  rmSync(sb.path("old.tsx"));
  rmSync(sb.path("removed.ts"));
  sb.write("moved.tsx", "function f() { try { return 1; } catch {} }\n");
  sb.write("added.ts", 'console.log("new bad");\n');
  sb.write("notes.md", "unrelated\n");
  const output = result();
  await post.after(
    {
      tool: "apply_patch",
      sessionID: "patch",
      callID: "completed",
      args: { patchText: patch(sb) },
    },
    output,
  );
  expect(output.output).toContain("Empty catch block");
  expect(output.output).toContain("Forbidden console.log");
  const after = gate(sb.project);
  expect(after.status).toBe("failing");
  expect(Object.keys(after.entries ?? {}).toSorted()).toEqual(
    [sb.path("added.ts"), sb.path("moved.tsx")].toSorted(),
  );
  const decider = createGateDecider(selected);
  if (!decider.ok) throw new Error(decider.reason);
  const decisions = await Promise.all(
    ['git commit -m "fix: invalid"', "git push origin HEAD"].map((command) =>
      decider.decide({ tool_name: "Bash", tool_input: { command }, cwd: sb.project }),
    ),
  );
  expect(decisions.map((decision) => decision.kind)).toEqual(["deny", "deny"]);
  post.clear();
});

test("a post block keeps a later registry crash visible to the model", async () => {
  using sb = createSandbox({ git: true });
  tsProject(sb);
  const registry = join(sb.project, ".opencode/toolu/state/toolu/post-tools.d");
  const selected = {
    ...options(sb),
    selectedPluginSpecs: new Set(["toolu@toolu", "ts-quality@toolu", "fixture@toolu"]),
  };
  writeFileSync(
    join(registry, "fixture@toolu__crash.js"),
    `export default { spec: "fixture@toolu", name: "crash", event: "tool/post",
      run(event) { if (event.toolInput.file_path === ${JSON.stringify(sb.path("one.ts"))})
        return Promise.resolve({ kind: "post_block", reason: "fixture first block" });
        if (event.toolInput.file_path === ${JSON.stringify(sb.path("two.ts"))})
        throw new Error("fixture post crash"); return Promise.resolve({ kind: "allow" }); } };\n`,
  );
  sb.write("one.ts", 'console.log("bad");\n');
  sb.write("two.ts", "const two = 2;\n");
  const patchText = [
    "*** Begin Patch",
    `*** Add File: ${sb.path("one.ts")}`,
    '+console.log("bad");',
    `*** Add File: ${sb.path("two.ts")}`,
    "+const two = 2;",
    "*** End Patch",
  ].join("\n");
  const post = createToolPostHandler(selected, createToolAdviceStore());
  const output = result();
  await post.after(
    { tool: "apply_patch", sessionID: "crash", callID: "patch", args: { patchText } },
    output,
  );
  expect(output.output).toContain("fixture first block");
  expect(output.output).toContain("fixture post crash");
  post.clear();
});

type Mode = "selected" | "disabled" | "no-tsconfig" | "no-lock";

async function selectionCase(mode: Mode): Promise<void> {
  using sb = createSandbox({ git: true });
  if (mode !== "no-lock") sb.write("bun.lock", "lock marker\n");
  if (mode !== "no-tsconfig") {
    sb.write("tsconfig.json", "{}\n");
    sb.git("add", "tsconfig.json");
  }
  const post = createToolPostHandler(
    options(sb, sb.project, mode !== "disabled"),
    createToolAdviceStore(),
  );
  const relative = "src/bad.ts";
  const bad = 'console.log("bad");\n';
  sb.write(relative, bad);
  const output = result();
  await post.after(
    { tool: "write", sessionID: mode, callID: "bad", args: { filePath: relative, content: bad } },
    output,
  );
  if (mode === "selected") {
    expect(output.output).toContain("Forbidden console.log");
    expect(gate(sb.project).entries?.[relative]?.source).toBe("ts-quality-hook");
    const clean = "const answer = 42;\n";
    sb.write(relative, clean);
    await post.after(
      {
        tool: "edit",
        sessionID: mode,
        callID: "clean",
        args: { filePath: relative, oldString: bad, newString: clean },
      },
      result(),
    );
    expect(gate(sb.project).status).toBe("passing");
    sb.write("notes.md", "console.log('text only')\n");
    const notes = result();
    await post.after(
      {
        tool: "write",
        sessionID: mode,
        callID: "notes",
        args: { filePath: sb.path("notes.md"), content: "console.log('text only')\n" },
      },
      notes,
    );
    expect(notes.output).toBe("original result");
    expect(gate(sb.project).status).toBe("passing");
  } else {
    expect(output.output).toBe("original result");
    expect(existsSync(join(sb.project, ".opencode/tmp/quality-gate-status.json"))).toBe(false);
  }
  post.clear();
}

test("selection and real project markers control checks on relative edits", async () => {
  await Promise.all(
    (["selected", "disabled", "no-tsconfig", "no-lock"] as const).map(selectionCase),
  );
});

test("independent OpenCode projects keep separate TypeScript gate entries", async () => {
  using first = createSandbox({ git: true });
  using second = createSandbox({ git: true });
  tsProject(first);
  tsProject(second);
  const a = createToolPostHandler(options(first), createToolAdviceStore());
  const b = createToolPostHandler(options(second), createToolAdviceStore());
  const bad = 'console.log("bad");\n';
  first.write("bad.ts", bad);
  second.write("bad.ts", bad);
  await Promise.all([
    a.after(
      {
        tool: "write",
        sessionID: "first",
        callID: "bad",
        args: { filePath: first.path("bad.ts"), content: bad },
      },
      result(),
    ),
    b.after(
      {
        tool: "write",
        sessionID: "second",
        callID: "bad",
        args: { filePath: second.path("bad.ts"), content: bad },
      },
      result(),
    ),
  ]);
  expect(gate(first.project).entries?.[first.path("bad.ts")]?.source).toBe("ts-quality-hook");
  expect(gate(second.project).entries?.[second.path("bad.ts")]?.source).toBe("ts-quality-hook");
  first.write("bad.ts", "const answer = 42;\n");
  await a.after(
    {
      tool: "edit",
      sessionID: "first",
      callID: "clean",
      args: { filePath: first.path("bad.ts"), oldString: bad, newString: "const answer = 42;\n" },
    },
    result(),
  );
  expect(gate(first.project).status).toBe("passing");
  expect(gate(second.project).status).toBe("failing");
  a.clear();
  b.clear();
});

test("a linked worktree is skipped without writing gate state", async () => {
  using sb = createSandbox({ git: true });
  tsProject(sb);
  sb.git("add", "bun.lock");
  sb.git("commit", "-q", "-m", "project");
  const linked = join(sb.root, "linked");
  sb.git("worktree", "add", "-q", "-b", "linked", linked);
  const post = createToolPostHandler(options(sb, linked), createToolAdviceStore());
  const path = join(linked, "bad.ts");
  const bad = 'console.log("bad");\n';
  writeFileSync(path, bad);
  const output = result();
  await post.after(
    { tool: "write", sessionID: "linked", callID: "bad", args: { filePath: path, content: bad } },
    output,
  );
  expect(output.output).toBe("original result");
  expect(existsSync(join(linked, ".opencode/tmp/quality-gate-status.json"))).toBe(false);
  post.clear();
});
