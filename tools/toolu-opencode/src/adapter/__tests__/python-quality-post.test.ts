/** Real python-quality bundle through OpenCode's completed-tool adapter (#353). */
import { expect, test } from "bun:test";
import {
  copyFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import { z } from "zod";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { createGateDecider, type PermissionEvaluateHandlerOptions } from "../evaluate.ts";
import { createToolAdviceStore } from "../tool-advice.ts";
import { createToolPostHandler } from "../tool-post.ts";

const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const MODULE = join(REPO_ROOT, "plugins/python-quality/hooks/dist/post-tool-use.js");
const Gate = z.object({
  status: z.string(),
  entries: z
    .record(z.string(), z.object({ source: z.string(), violations: z.string() }))
    .optional(),
});
const BARE = "def load():\n    try:\n        return 1\n    except:\n        return 0\n";
const SWALLOW = "try:\n    run()\nexcept Exception: pass\n";
const MOCKED = "from unittest import mock\n\n\ndef test_it():\n    assert mock\n";
const CLEAN = '"""Clean."""\n\n\ndef answer():\n    """Answer."""\n    return 42\n';

function options(
  sb: Sandbox,
  projectRoot = sb.project,
  selected = true,
  env: NodeJS.ProcessEnv = process.env,
): PermissionEvaluateHandlerOptions {
  const configRoot = join(projectRoot, ".opencode/toolu/state");
  const registry = join(configRoot, "toolu/post-tools.d");
  mkdirSync(registry, { recursive: true });
  copyFileSync(MODULE, join(registry, "python-quality@toolu__python-quality.js"));
  return {
    repoRoot: REPO_ROOT,
    configRoot,
    userConfigRoot: join(sb.root, ".xdg/opencode"),
    permissionContext: { cwd: projectRoot, projectRoot, worktree: projectRoot },
    selectedPluginSpecs: new Set(
      selected ? ["toolu@toolu", "python-quality@toolu"] : ["toolu@toolu"],
    ),
    env: { ...env, TOOLU_HOST_OVERRIDE: "opencode" },
  };
}

function pyProject(sb: Sandbox): void {
  sb.write("pyproject.toml", '[project]\nname = "python-quality-host"\n');
}

function result(): { title: string; output: string; metadata: object } {
  return { title: "tool result", output: "original result", metadata: {} };
}

function gatePath(root: string): string {
  return join(root, ".opencode/tmp/quality-gate-status.json");
}

function gate(root: string): z.infer<typeof Gate> {
  return Gate.parse(JSON.parse(readFileSync(gatePath(root), "utf8")));
}

function patch(sb: Sandbox): string {
  return [
    "*** Begin Patch",
    `*** Update File: ${sb.path("pkg/old.py")}`,
    `*** Move to: ${sb.path("pkg/moved.py")}`,
    "@@",
    " def load():",
    "     try:",
    "         return 1",
    "-    except:",
    "-        return 0",
    "+    except Exception: pass",
    `*** Delete File: ${sb.path("pkg/removed.py")}`,
    `*** Add File: ${sb.path("pkg/test_added.py")}`,
    ...MOCKED.trimEnd()
      .split("\n")
      .map((line) => `+${line}`),
    `*** Add File: ${sb.path("pkg/notes.md")}`,
    "+except: pass",
    "*** End Patch",
  ].join("\n");
}

test("a Python patch keeps both violations while move and delete clear old entries", async () => {
  using sb = createSandbox({ git: true });
  expect(Bun.which("python3")).not.toBeNull();
  expect(Bun.which("ast-grep")).not.toBeNull();
  pyProject(sb);
  const selected = options(sb);
  const post = createToolPostHandler(selected, createToolAdviceStore());
  await Promise.all(
    ["pkg/old.py", "pkg/removed.py"].map(async (name) => {
      sb.write(name, BARE);
      await post.after(
        {
          tool: "write",
          sessionID: "patch",
          callID: name,
          args: { filePath: sb.path(name), content: BARE },
        },
        result(),
      );
    }),
  );
  expect(Object.keys(gate(sb.project).entries ?? {}).toSorted()).toEqual(
    [sb.path("pkg/old.py"), sb.path("pkg/removed.py")].toSorted(),
  );

  rmSync(sb.path("pkg/old.py"));
  rmSync(sb.path("pkg/removed.py"));
  const moved = BARE.replace("    except:\n        return 0\n", "    except Exception: pass\n");
  sb.write("pkg/moved.py", moved);
  sb.write("pkg/test_added.py", MOCKED);
  sb.write("pkg/notes.md", "except: pass\n");
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
  expect(output.output).toContain("one-line except ...: pass");
  expect(output.output).toContain("no-mocks: mock import");
  expect(output.output).not.toContain("notes.md");
  const after = gate(sb.project);
  expect(after.status).toBe("failing");
  expect(Object.keys(after.entries ?? {}).toSorted()).toEqual(
    [sb.path("pkg/moved.py"), sb.path("pkg/test_added.py")].toSorted(),
  );
  expect(after.entries?.[sb.path("pkg/moved.py")]?.source).toBe("python-quality-hook");
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

type Mode = "selected" | "disabled" | "no-marker" | "no-python3";

/** A PATH holding git and ast-grep but no python3, so the module's own prerequisite decides. */
function pathWithoutPython(sb: Sandbox): NodeJS.ProcessEnv {
  const bin = join(sb.root, "bin-no-python");
  mkdirSync(bin, { recursive: true });
  for (const tool of ["git", "ast-grep"]) {
    const found = Bun.which(tool);
    if (found === null) throw new Error(`${tool} is required on PATH`);
    symlinkSync(found, join(bin, tool));
  }
  return { ...process.env, PATH: bin };
}

async function selectionCase(mode: Mode): Promise<void> {
  using sb = createSandbox({ git: true });
  if (mode !== "no-marker") pyProject(sb);
  const env = mode === "no-python3" ? pathWithoutPython(sb) : process.env;
  const post = createToolPostHandler(
    options(sb, sb.project, mode !== "disabled", env),
    createToolAdviceStore(),
  );
  const relative = "src/bad.py";
  sb.write(relative, SWALLOW);
  const output = result();
  await post.after(
    {
      tool: "write",
      sessionID: mode,
      callID: "bad",
      args: { filePath: relative, content: SWALLOW },
    },
    output,
  );
  if (mode === "selected") {
    expect(output.output).toContain("Forbidden suppression in src/bad.py");
    expect(gate(sb.project).entries?.[relative]?.source).toBe("python-quality-hook");
    sb.write(relative, CLEAN);
    await post.after(
      {
        tool: "edit",
        sessionID: mode,
        callID: "clean",
        args: { filePath: relative, oldString: SWALLOW, newString: CLEAN },
      },
      result(),
    );
    expect(gate(sb.project).status).toBe("passing");
    sb.write("notes.md", SWALLOW);
    const notes = result();
    await post.after(
      {
        tool: "write",
        sessionID: mode,
        callID: "notes",
        args: { filePath: sb.path("notes.md"), content: SWALLOW },
      },
      notes,
    );
    expect(notes.output).toBe("original result");
    expect(gate(sb.project).status).toBe("passing");
  } else {
    expect(output.output).toBe("original result");
    expect(existsSync(gatePath(sb.project))).toBe(false);
  }
  post.clear();
}

test("selection, project markers and python3 control checks on relative edits", async () => {
  await Promise.all(
    (["selected", "disabled", "no-marker", "no-python3"] as const).map(selectionCase),
  );
});

test("independent OpenCode projects keep separate Python gate entries", async () => {
  using first = createSandbox({ git: true });
  using second = createSandbox({ git: true });
  pyProject(first);
  pyProject(second);
  const a = createToolPostHandler(options(first), createToolAdviceStore());
  const b = createToolPostHandler(options(second), createToolAdviceStore());
  first.write("bad.py", SWALLOW);
  second.write("bad.py", SWALLOW);
  await Promise.all(
    [
      { post: a, sb: first },
      { post: b, sb: second },
    ].map(({ post, sb }) =>
      post.after(
        {
          tool: "write",
          sessionID: sb.project,
          callID: "bad",
          args: { filePath: sb.path("bad.py"), content: SWALLOW },
        },
        result(),
      ),
    ),
  );
  expect(gate(first.project).entries?.[first.path("bad.py")]?.source).toBe("python-quality-hook");
  expect(gate(second.project).entries?.[second.path("bad.py")]?.source).toBe("python-quality-hook");
  first.write("bad.py", CLEAN);
  await a.after(
    {
      tool: "edit",
      sessionID: "first",
      callID: "clean",
      args: { filePath: first.path("bad.py"), oldString: SWALLOW, newString: CLEAN },
    },
    result(),
  );
  expect(gate(first.project).status).toBe("passing");
  expect(gate(second.project).status).toBe("failing");
  a.clear();
  b.clear();
});

test("a linked worktree keeps python-quality's existing check and owns its gate", async () => {
  using sb = createSandbox({ git: true });
  pyProject(sb);
  sb.git("add", "pyproject.toml");
  sb.git("commit", "-q", "-m", "project");
  const linked = join(sb.root, "linked");
  sb.git("worktree", "add", "-q", "-b", "linked", linked);
  const post = createToolPostHandler(options(sb, linked), createToolAdviceStore());
  const path = join(linked, "bad.py");
  writeFileSync(path, SWALLOW);
  const output = result();
  await post.after(
    {
      tool: "write",
      sessionID: "linked",
      callID: "bad",
      args: { filePath: path, content: SWALLOW },
    },
    output,
  );
  expect(output.output).toContain("Forbidden suppression");
  expect(gate(linked).entries?.[path]?.source).toBe("python-quality-hook");
  expect(existsSync(gatePath(sb.project))).toBe(false);
  post.clear();
});
