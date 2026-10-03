import { afterEach, expect, test } from "bun:test";
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { configFiles } from "../../../../../packages/toolu-core/src/config/config-files.ts";
import { agentArgs, configPaths, loadFixerConfig, routeFix } from "../babysit/fixer-route.ts";

// OpenCode as a babysit controller and fixer host (#357): config from the
// adapter's roots, an `opencode` row, Jev only from the OpenCode data root.

const root = resolve(import.meta.dir, "../../../../..");
const fixture = join(root, "plugins/pr-babysit/scripts/__tests__/fixtures/items/review-items.json");
const answers = join(root, "plugins/pr-babysit/scripts/__tests__/fixtures/jev/fix-tiers.json");
const KEYS = [
  "HOME",
  "PATH",
  "PB_JEV",
  "TOOLU_CONFIG_DIR",
  "TOOLU_USER_CONFIG_DIR",
  "TOOLU_PROJECT_DIR",
  "TOOLU_PROJECT_CONFIG_DIRNAME",
  "TYPESAFE_API_KEY",
  "XDG_CONFIG_HOME",
] as const;
const original = Object.fromEntries(KEYS.map((key) => [key, process.env[key]]));
const temps: string[] = [];

afterEach(() => {
  for (const dir of temps.splice(0)) rmSync(dir, { recursive: true, force: true });
  for (const [key, value] of Object.entries(original)) {
    if (value === undefined) delete process.env[key];
    else process.env[key] = value;
  }
});

type Roots = { home: string; project: string; data: string; user: string; bin: string };

/** The env OpenCode's `shell.env` gives bash, rooted in a fresh sandbox. */
function opencodeEnv(): Roots {
  const dir = mkdtempSync(join(tmpdir(), "pr-babysit-opencode-"));
  temps.push(dir);
  const roots = {
    home: join(dir, "home"),
    project: join(dir, "project"),
    data: join(dir, "project/.opencode/toolu/state"),
    user: join(dir, "home/.config/opencode"),
    bin: join(dir, "bin"),
  };
  for (const path of Object.values(roots)) mkdirSync(path, { recursive: true });
  process.env.HOME = roots.home;
  process.env.TOOLU_CONFIG_DIR = roots.data;
  process.env.TOOLU_USER_CONFIG_DIR = roots.user;
  process.env.TOOLU_PROJECT_DIR = roots.project;
  process.env.TOOLU_PROJECT_CONFIG_DIRNAME = ".opencode";
  process.env.PATH = `${roots.bin}:/usr/bin:/bin`;
  delete process.env.PB_JEV;
  delete process.env.TYPESAFE_API_KEY;
  return roots;
}

/** A real executable on PATH: routing only asks whether the CLI exists. */
function onPath(roots: Roots, name: string): void {
  const file = join(roots.bin, name);
  writeFileSync(file, "#!/bin/sh\nexit 0\n");
  chmodSync(file, 0o755);
}

function writeConfig(path: string, prBabysit: unknown): void {
  mkdirSync(resolve(path, ".."), { recursive: true });
  writeFileSync(path, JSON.stringify({ version: 1, prBabysit }));
}

test("OpenCode config files are the adapter's user and .opencode project files", () => {
  const roots = opencodeEnv();
  const paths = configPaths("opencode");
  expect(paths).toEqual({
    user: join(roots.user, "toolu.config.json"),
    project: join(roots.project, ".opencode/toolu.config.json"),
  });
  const core = configFiles({ host: "opencode", env: process.env, cwd: roots.project }).files;
  expect(paths).toEqual({ user: core.user, project: core.project ?? "" });
});

test("--host opencode merges both files, accepts routing.opencode and defaults to the controller host", () => {
  const roots = opencodeEnv();
  onPath(roots, "opencode");
  expect(loadFixerConfig("opencode").hosts).toEqual(["opencode"]);
  expect(loadFixerConfig("opencode").routing.opencode).toEqual([{}, {}, {}, {}]);
  writeConfig(join(roots.user, "toolu.config.json"), { unattended: false });
  writeConfig(join(roots.project, ".opencode/toolu.config.json"), {
    routing: {
      opencode: [
        { model: "probe/small" },
        { model: "probe/mid" },
        { model: "probe/large", effort: "high" },
        { model: "probe/large", effort: "max" },
      ],
    },
  });
  const route = routeFix({ itemsFile: fixture, host: "opencode", answersFile: answers });
  expect(route.dispatch).toBe("herdr");
  expect(route.unattended).toBe(false);
  expect(route.groups).toEqual([
    {
      seq: 1,
      tier: "complex",
      class: "architecture",
      host: "opencode",
      model: "probe/large",
      effort: "high",
      items: expect.any(Array),
    },
    {
      seq: 2,
      tier: "trivial",
      class: "mechanical",
      host: "opencode",
      model: "probe/small",
      effort: null,
      items: expect.any(Array),
    },
  ]);
  writeConfig(join(roots.project, ".opencode/toolu.config.json"), {
    routing: { opencode: [{ model: "probe/a b" }, {}, {}, {}] },
  });
  expect(() => loadFixerConfig("opencode")).toThrow("shell-unsafe");
});

test("an opencode CLI missing from PATH drops the host and routes inline", () => {
  opencodeEnv();
  const route = routeFix({ itemsFile: fixture, host: "opencode", noJev: true });
  expect(route.dispatch).toBe("inline");
  expect(route.note).toContain("CLI not on PATH: opencode");
  expect((route.groups as { host: string | null }[]).every((g) => g.host === null)).toBe(true);
});

test("Jev on OpenCode comes only from the data root, never a Claude or Codex wrapper", () => {
  const roots = opencodeEnv();
  onPath(roots, "opencode");
  for (const host of [".claude", ".codex"]) {
    mkdirSync(join(roots.home, host, "jev"), { recursive: true });
    writeFileSync(join(roots.home, host, "jev/jev.sh"), "#!/bin/sh\nexit 0\n");
  }
  const absent = routeFix({ itemsFile: fixture, host: "opencode" });
  expect(absent.note).toContain("jev unavailable (jev.sh not installed)");
  mkdirSync(join(roots.data, "jev"), { recursive: true });
  writeFileSync(join(roots.data, "jev/jev.sh"), "#!/bin/sh\nexit 0\n");
  const present = routeFix({ itemsFile: fixture, host: "opencode" });
  expect(present.note).toContain("jev unavailable (TYPESAFE_API_KEY not set)");
});

test("fixer flags: OpenCode uses --auto, --model and --variant; other hosts are unchanged", () => {
  expect(agentArgs("opencode", "pb-1", "probe/m", "high", true)).toEqual([
    "--auto",
    "--model",
    "probe/m",
    "--variant",
    "high",
  ]);
  expect(agentArgs("opencode", "pb-1", null, null, false)).toEqual([]);
  expect(agentArgs("claude", "pb-1", "opus", "xhigh", true)).toEqual([
    "--dangerously-skip-permissions",
    "-n",
    "pb-1",
    "--model",
    "opus",
    "--effort",
    "xhigh",
  ]);
  expect(agentArgs("claude", "pb-1", null, null, false)).toEqual([
    "--permission-mode",
    "auto",
    "-n",
    "pb-1",
  ]);
  expect(agentArgs("codex", "pb-1", "gpt-6-sol", "high", false)).toEqual([
    "--ask-for-approval",
    "on-request",
    "--sandbox",
    "workspace-write",
    "--model",
    "gpt-6-sol",
    "-c",
    "model_reasoning_effort=high",
  ]);
  expect(agentArgs("cursor", "pb-1", "composer-2.5", "high", true)).toEqual([
    "--yolo",
    "--trust",
    "--approve-mcps",
    "--model",
    "composer-2.5",
  ]);
  expect(() => agentArgs("opencode", "pb-1", "probe/m", "a;b", true)).toThrow("unsafe opencode arg");
});
