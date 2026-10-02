import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdtempSync, mkdirSync, symlinkSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { detectHost } from "../detect.ts";
import {
  opencodeConfigRoot,
  opencodeDataRoot,
  opencodePluginSelectionPath,
  opencodeProjectKey,
} from "../roots.ts";

function git(cwd: string, args: string[]): void {
  const res = spawnSync("git", ["-C", cwd, ...args], { encoding: "utf8" });
  if (res.status !== 0) throw new Error(`git ${args.join(" ")}: ${res.stderr}`);
}

const tmpBase = process.env.TMPDIR ?? "/tmp";

test("detectHost honors TOOLU_HOST_OVERRIDE=opencode", () => {
  expect(detectHost({ env: { TOOLU_HOST_OVERRIDE: "opencode" } })).toBe("opencode");
});

test("detectHost uses OPENCODE_* env and .opencode project marker", () => {
  const project = mkdtempSync(join(tmpBase, "toolu-oc-host-"));
  mkdirSync(join(project, ".opencode"), { recursive: true });
  expect(detectHost({ env: { OPENCODE_HOME: "/tmp/oc" } })).toBe("opencode");
  expect(detectHost({ env: {}, projectRoot: project })).toBe("opencode");
});

test("the global config root follows TOOLU_CONFIG_DIR, TOOLU_OPENCODE_HOME, then XDG", () => {
  const home = "/home/u";
  expect(opencodeConfigRoot({ env: { HOME: home, TOOLU_CONFIG_DIR: "/cfg" } })).toBe("/cfg");
  expect(opencodeConfigRoot({ env: { HOME: home, TOOLU_OPENCODE_HOME: "/oc-home" } })).toBe(
    "/oc-home",
  );
  expect(opencodeConfigRoot({ env: { HOME: home, XDG_CONFIG_HOME: "/xdg" } })).toBe(
    "/xdg/opencode",
  );
  expect(opencodeConfigRoot({ env: { HOME: home } })).toBe("/home/u/.config/opencode");
  expect(opencodeConfigRoot({ env: { HOME: home, TOOLU_CONFIG_DIR: "" } })).toBe(
    "/home/u/.config/opencode",
  );
});

test("without an override the data root is the project's own .opencode/toolu/state", () => {
  using sb = createSandbox();
  expect(opencodeDataRoot({ dataRoot: join(sb.root, "data") })).toBe(join(sb.root, "data"));
  expect(opencodeDataRoot({ projectRoot: sb.project, env: {} })).toBe(
    join(sb.project, ".opencode", "toolu", "state"),
  );
  expect(() => opencodeDataRoot({ env: { TOOLU_CONFIG_DIR: "/cfg" } })).toThrow(/projectRoot/);
});

test("under one override, two projects and a linked worktree get distinct keyed data roots", () => {
  using a = createSandbox({ git: true });
  using b = createSandbox({ git: true });
  const worktree = join(a.root, "linked wt");
  git(a.project, ["worktree", "add", "-q", worktree, "-b", "wt"]);
  const shared = join(a.root, "shared root");
  const env = { TOOLU_CONFIG_DIR: shared, TOOLU_OPENCODE_HOME: "/ignored" };
  const projects = [a.project, b.project, worktree];
  const roots = projects.map((projectRoot) => opencodeDataRoot({ projectRoot, env }));
  expect(new Set(roots).size).toBe(3);
  expect(roots).toEqual(
    projects.map((project) =>
      join(shared, "toolu", "opencode", "projects", opencodeProjectKey(project)),
    ),
  );
  const viaHome = opencodeDataRoot({
    projectRoot: a.project,
    env: { TOOLU_OPENCODE_HOME: shared },
  });
  expect(viaHome).toBe(opencodeDataRoot({ projectRoot: a.project, env }));
});

test("the project key is a readable slug plus a hash of the real path", () => {
  using sb = createSandbox({ git: true });
  const project = join(sb.root, "My Project!");
  mkdirSync(project);
  const link = join(sb.root, "link");
  symlinkSync(project, link);
  const key = opencodeProjectKey(project);
  expect(key).toMatch(/^my-project-[0-9a-f]{16}$/);
  expect(opencodeProjectKey(link)).toBe(key);
  expect(opencodeProjectKey(sb.project)).not.toBe(key);
  const missing = join(sb.root, "not yet", "here");
  expect(opencodeProjectKey(missing)).toMatch(/^here-[0-9a-f]{16}$/);
  expect(opencodeProjectKey(join(sb.root, "x".repeat(80)))).toMatch(/^x{32}-[0-9a-f]{16}$/);
  expect(opencodeProjectKey(join(sb.root, "日本"))).toMatch(/^project-[0-9a-f]{16}$/);
});

test("selection path lives under project .opencode", () => {
  const project = mkdtempSync(join(tmpBase, "toolu-oc-sel-"));
  mkdirSync(join(project, ".opencode", "toolu"), { recursive: true });
  writeFileSync(opencodePluginSelectionPath(project), '{"version":1,"enabled":["toolu"]}\n');
  expect(opencodePluginSelectionPath(project)).toBe(
    join(project, ".opencode", "toolu", "plugins.json"),
  );
});
