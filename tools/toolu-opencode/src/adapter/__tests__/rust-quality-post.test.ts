/** Real rust-quality bundle through OpenCode's completed-tool adapter (#354). */
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
const MODULE = join(REPO_ROOT, "plugins/rust-quality/hooks/dist/post-tool-use.js");
const Gate = z.object({
  status: z.string(),
  entries: z
    .record(z.string(), z.object({ source: z.string(), violations: z.string() }))
    .optional(),
});
const UNWRAP =
  '//! Loader.\n\n/// Load a value.\npub fn load() -> u32 {\n    "1".parse().unwrap()\n}\n';
const EXPECT = UNWRAP.replace(".unwrap()", '.expect("number")');
const SUPPRESSED = "#[allow(dead_code)]\nfn unused() {}\n";
const MOCKED =
  "use mockall::predicate;\n\n#[test]\nfn it_works() {\n    assert!(predicate::eq(1).eval(&1));\n}\n";
const CLEAN = "//! Clean.\n\n/// Answer.\npub fn answer() -> u32 {\n    42\n}\n";

function options(
  sb: Sandbox,
  projectRoot = sb.project,
  selected = true,
  env: NodeJS.ProcessEnv = process.env,
): PermissionEvaluateHandlerOptions {
  const configRoot = join(projectRoot, ".opencode/toolu/state");
  const registry = join(configRoot, "toolu/post-tools.d");
  mkdirSync(registry, { recursive: true });
  copyFileSync(MODULE, join(registry, "rust-quality@toolu__rust-quality.js"));
  return {
    repoRoot: REPO_ROOT,
    configRoot,
    userConfigRoot: join(sb.root, ".xdg/opencode"),
    permissionContext: { cwd: projectRoot, projectRoot, worktree: projectRoot },
    selectedPluginSpecs: new Set(
      selected ? ["toolu@toolu", "rust-quality@toolu"] : ["toolu@toolu"],
    ),
    env: { ...env, TOOLU_HOST_OVERRIDE: "opencode" },
  };
}

function cargoProject(sb: Sandbox): void {
  sb.write(
    "Cargo.toml",
    '[package]\nname = "rust-quality-host"\nversion = "0.1.0"\nedition = "2021"\n',
  );
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
    `*** Update File: ${sb.path("src/old.rs")}`,
    `*** Move to: ${sb.path("src/moved.rs")}`,
    "@@",
    " pub fn load() -> u32 {",
    '-    "1".parse().unwrap()',
    '+    "1".parse().expect("number")',
    " }",
    `*** Delete File: ${sb.path("src/removed.rs")}`,
    `*** Add File: ${sb.path("tests/added.rs")}`,
    ...MOCKED.trimEnd()
      .split("\n")
      .map((line) => `+${line}`),
    `*** Add File: ${sb.path("src/notes.md")}`,
    '+"1".parse().unwrap()',
    "*** End Patch",
  ].join("\n");
}

test("a Rust patch keeps both violations while move and delete clear old entries", async () => {
  using sb = createSandbox({ git: true });
  expect(Bun.which("cargo")).not.toBeNull();
  expect(Bun.which("ast-grep")).not.toBeNull();
  cargoProject(sb);
  const selected = options(sb);
  const post = createToolPostHandler(selected, createToolAdviceStore());
  await Promise.all(
    ["src/old.rs", "src/removed.rs"].map(async (name) => {
      sb.write(name, UNWRAP);
      const seeded = result();
      await post.after(
        {
          tool: "write",
          sessionID: "patch",
          callID: name,
          args: { filePath: sb.path(name), content: UNWRAP },
        },
        seeded,
      );
      expect(seeded.output).toContain(`.unwrap() in ${sb.path(name)}`);
    }),
  );
  expect(Object.keys(gate(sb.project).entries ?? {}).toSorted()).toEqual(
    [sb.path("src/old.rs"), sb.path("src/removed.rs")].toSorted(),
  );

  rmSync(sb.path("src/old.rs"));
  rmSync(sb.path("src/removed.rs"));
  sb.write("src/moved.rs", EXPECT);
  sb.write("tests/added.rs", MOCKED);
  sb.write("src/notes.md", '"1".parse().unwrap()\n');
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
  expect(output.output).toContain(`.expect() in ${sb.path("src/moved.rs")}`);
  expect(output.output).toContain("no-mocks: mockall/faux import");
  expect(output.output).not.toContain("notes.md");
  const after = gate(sb.project);
  expect(after.status).toBe("failing");
  expect(Object.keys(after.entries ?? {}).toSorted()).toEqual(
    [sb.path("src/moved.rs"), sb.path("tests/added.rs")].toSorted(),
  );
  expect(after.entries?.[sb.path("src/moved.rs")]?.source).toBe("rust-quality-hook");
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

type Mode = "selected" | "disabled" | "no-marker" | "no-cargo";

/** A PATH holding git and ast-grep but no cargo, so the module's own prerequisite decides. */
function pathWithoutCargo(sb: Sandbox): NodeJS.ProcessEnv {
  const bin = join(sb.root, "bin-no-cargo");
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
  if (mode !== "no-marker") cargoProject(sb);
  const env = mode === "no-cargo" ? pathWithoutCargo(sb) : process.env;
  const post = createToolPostHandler(
    options(sb, sb.project, mode !== "disabled", env),
    createToolAdviceStore(),
  );
  const relative = "src/bad.rs";
  sb.write(relative, SUPPRESSED);
  const output = result();
  await post.after(
    {
      tool: "write",
      sessionID: mode,
      callID: "bad",
      args: { filePath: relative, content: SUPPRESSED },
    },
    output,
  );
  if (mode === "selected") {
    expect(output.output).toContain(
      "Forbidden lint suppression (#[allow]/#[expect]/cfg_attr allow) in src/bad.rs",
    );
    expect(gate(sb.project).entries?.[relative]?.source).toBe("rust-quality-hook");
    sb.write(relative, CLEAN);
    await post.after(
      {
        tool: "edit",
        sessionID: mode,
        callID: "clean",
        args: { filePath: relative, oldString: SUPPRESSED, newString: CLEAN },
      },
      result(),
    );
    expect(gate(sb.project).status).toBe("passing");
    // Error handling matches `/src/` in the path as given, so a relative `src/` unwrap is not reported.
    const outputs = await Promise.all(
      (
        [
          ["notes.md", sb.path("notes.md"), SUPPRESSED],
          ["src/load.rs", "src/load.rs", UNWRAP],
        ] as const
      ).map(async ([name, filePath, content]) => {
        sb.write(name, content);
        const unchecked = result();
        await post.after(
          { tool: "write", sessionID: mode, callID: filePath, args: { filePath, content } },
          unchecked,
        );
        return unchecked.output;
      }),
    );
    expect(outputs).toEqual(["original result", "original result"]);
    expect(gate(sb.project).status).toBe("passing");
  } else {
    expect(output.output).toBe("original result");
    expect(existsSync(gatePath(sb.project))).toBe(false);
  }
  post.clear();
}

test("selection, Cargo.toml and cargo control checks on relative edits", async () => {
  await Promise.all(
    (["selected", "disabled", "no-marker", "no-cargo"] as const).map(selectionCase),
  );
});

test("independent OpenCode projects keep separate Rust gate entries", async () => {
  using first = createSandbox({ git: true });
  using second = createSandbox({ git: true });
  cargoProject(first);
  cargoProject(second);
  const a = createToolPostHandler(options(first), createToolAdviceStore());
  const b = createToolPostHandler(options(second), createToolAdviceStore());
  first.write("bad.rs", SUPPRESSED);
  second.write("bad.rs", SUPPRESSED);
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
          args: { filePath: sb.path("bad.rs"), content: SUPPRESSED },
        },
        result(),
      ),
    ),
  );
  expect(gate(first.project).entries?.[first.path("bad.rs")]?.source).toBe("rust-quality-hook");
  expect(gate(second.project).entries?.[second.path("bad.rs")]?.source).toBe("rust-quality-hook");
  first.write("bad.rs", CLEAN);
  await a.after(
    {
      tool: "edit",
      sessionID: "first",
      callID: "clean",
      args: { filePath: first.path("bad.rs"), oldString: SUPPRESSED, newString: CLEAN },
    },
    result(),
  );
  expect(gate(first.project).status).toBe("passing");
  expect(gate(second.project).status).toBe("failing");
  a.clear();
  b.clear();
});

test("a linked worktree keeps rust-quality's existing check and owns its gate", async () => {
  using sb = createSandbox({ git: true });
  cargoProject(sb);
  sb.git("add", "Cargo.toml");
  sb.git("commit", "-q", "-m", "project");
  const linked = join(sb.root, "linked");
  sb.git("worktree", "add", "-q", "-b", "linked", linked);
  const post = createToolPostHandler(options(sb, linked), createToolAdviceStore());
  const path = join(linked, "bad.rs");
  writeFileSync(path, SUPPRESSED);
  const output = result();
  await post.after(
    {
      tool: "write",
      sessionID: "linked",
      callID: "bad",
      args: { filePath: path, content: SUPPRESSED },
    },
    output,
  );
  expect(output.output).toContain("Forbidden lint suppression");
  expect(gate(linked).entries?.[path]?.source).toBe("rust-quality-hook");
  expect(existsSync(gatePath(sb.project))).toBe(false);
  post.clear();
});
