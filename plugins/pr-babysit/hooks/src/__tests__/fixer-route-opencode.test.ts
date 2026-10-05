import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { entryArgv } from "@toolu/conformance/harness/entry-command";
import { dirname, join, resolve } from "node:path";
import { configFiles } from "../../../../../packages/toolu-core/src/config/config-files.ts";
import { agentArgs, configPaths, jevArgv } from "../babysit/fixer-route.ts";

// OpenCode as a babysit controller and fixer host (#357): config from the
// adapter's roots, an `opencode` row, Jev only from the OpenCode data root.
// Routing runs the shipped bundle with an explicit env, never process.env.

const root = resolve(import.meta.dir, "../../../../..");
const fixture = join(root, "plugins/pr-babysit/scripts/__tests__/fixtures/items/review-items.json");
const answers = join(root, "plugins/pr-babysit/scripts/__tests__/fixtures/jev/fix-tiers.json");

type Roots = {
  home: string;
  project: string;
  data: string;
  user: string;
  bin: string;
  env: Record<string, string>;
  [Symbol.dispose](): void;
};

/** The env OpenCode's `shell.env` gives bash, rooted in a fresh sandbox. */
function opencodeRoots(): Roots {
  const dir = mkdtempSync(join(tmpdir(), "pr-babysit-opencode-"));
  const roots = {
    home: join(dir, "home"),
    project: join(dir, "project"),
    data: join(dir, "project/.opencode/toolu/state"),
    user: join(dir, "home/.config/opencode"),
    bin: join(dir, "bin"),
  };
  for (const path of Object.values(roots)) mkdirSync(path, { recursive: true });
  return {
    ...roots,
    env: {
      HOME: roots.home,
      TOOLU_CONFIG_DIR: roots.data,
      TOOLU_USER_CONFIG_DIR: roots.user,
      TOOLU_PROJECT_DIR: roots.project,
      TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
      PATH: `${roots.bin}:/usr/bin:/bin`,
    },
    [Symbol.dispose]: () => rmSync(dir, { recursive: true, force: true }),
  };
}

/** A real executable on PATH: routing only asks whether the CLI exists. */
function onPath(roots: Roots, name: string): void {
  const file = join(roots.bin, name);
  writeFileSync(file, "#!/bin/sh\nexit 0\n");
  chmodSync(file, 0o755);
}

function writeConfig(path: string, prBabysit: unknown): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, JSON.stringify({ version: 1, prBabysit }));
}

/** `babysit-route-fix.js --host opencode` with exactly the sandbox env; its exit and parsed stdout. */
function route(
  roots: Roots,
  args: string[],
): { status: number | null; out: Record<string, unknown> } {
  const [command = "", ...prefix] = entryArgv("pr-babysit", "babysit-route-fix");
  const res = spawnSync(command, [...prefix, "--items", fixture, "--host", "opencode", ...args], {
    cwd: roots.project,
    env: roots.env,
    encoding: "utf8",
  });
  const out: Record<string, unknown> = JSON.parse(res.stdout);
  return { status: res.status, out };
}

test.concurrent("OpenCode config files are the adapter's user and .opencode project files", () => {
  using roots = opencodeRoots();
  const paths = configPaths("opencode", roots.env);
  expect(paths).toEqual({
    user: join(roots.user, "toolu.config.json"),
    project: join(roots.project, ".opencode/toolu.config.json"),
  });
  const core = configFiles({ host: "opencode", env: roots.env, cwd: roots.project }).files;
  expect(paths).toEqual({ user: core.user, project: core.project ?? "" });
});

test.concurrent("--host opencode merges both files, accepts routing.opencode and defaults to the controller host", () => {
  using roots = opencodeRoots();
  onPath(roots, "opencode");
  const defaults = route(roots, ["--jev-answers-in", answers]);
  expect(defaults.status).toBe(0);
  expect(
    (defaults.out.groups as { host: string; model: unknown; effort: unknown }[]).map((g) => [
      g.host,
      g.model,
      g.effort,
    ]),
  ).toEqual([
    ["opencode", null, null],
    ["opencode", null, null],
  ]);
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
  const routed = route(roots, ["--jev-answers-in", answers]);
  expect(routed.status).toBe(0);
  expect(routed.out.dispatch).toBe("herdr");
  expect(routed.out.unattended).toBe(false);
  expect(routed.out.groups).toEqual([
    {
      seq: 1,
      tier: "complex",
      class: "architecture",
      host: "opencode",
      model: "probe/large",
      effort: "high",
      items: ["PRRT_kwDOSzYYFc6jy6Au", "PRRT_kwDOSzYYFc6jy5_u"],
    },
    {
      seq: 2,
      tier: "trivial",
      class: "mechanical",
      host: "opencode",
      model: "probe/small",
      effort: null,
      items: [
        "PRRT_kwDOSzUwAc6K6nEk",
        "PRRT_kwDOSzUwAc6d2Ypf",
        "PRRT_kwDOSzYYFc6jy6BF",
        "PRRT_kwDOSzUwAc6K6VND",
      ],
    },
  ]);
  writeConfig(join(roots.project, ".opencode/toolu.config.json"), {
    routing: { opencode: [{ model: "probe/a b" }, {}, {}, {}] },
  });
  const unsafe = route(roots, ["--no-jev"]);
  expect(unsafe.status).toBe(3);
  expect(JSON.stringify(unsafe.out)).toContain("shell-unsafe");
});

test.concurrent("an opencode CLI missing from PATH drops the host and routes inline", () => {
  using roots = opencodeRoots();
  const { status, out } = route(roots, ["--no-jev"]);
  expect(status).toBe(0);
  expect(out.dispatch).toBe("inline");
  expect(out.note).toContain("CLI not on PATH: opencode");
  expect((out.groups as { host: string | null }[]).every((g) => g.host === null)).toBe(true);
});

test.concurrent("Jev on OpenCode comes only from the data root, never a Claude or Codex wrapper", () => {
  using roots = opencodeRoots();
  onPath(roots, "opencode");
  for (const host of [".claude", ".codex"]) {
    mkdirSync(join(roots.home, host, "jev"), { recursive: true });
    writeFileSync(join(roots.home, host, "jev/jev.sh"), "#!/bin/sh\nexit 0\n");
  }
  expect(route(roots, []).out.note).toContain("jev unavailable (jev.sh not installed)");
  mkdirSync(join(roots.data, "jev"), { recursive: true });
  writeFileSync(join(roots.data, "jev/jev.sh"), "#!/bin/sh\nexit 0\n");
  expect(route(roots, []).out.note).toContain("jev unavailable (TYPESAFE_API_KEY not set)");
});

test.concurrent("Jev runs without .env loading on OpenCode only", () => {
  expect(jevArgv("opencode", "/j/jev.sh", "/s.json")).toEqual([
    "--no-env-file",
    "/j/jev.sh",
    "ask",
    "-",
    "-s",
    "@/s.json",
  ]);
  for (const host of ["claude", "codex"] as const)
    expect(jevArgv(host, "/j/jev.sh", "/s.json")).toEqual([
      "/j/jev.sh",
      "ask",
      "-",
      "-s",
      "@/s.json",
    ]);
});

test.concurrent("fixer flags: OpenCode uses --auto, --model and --variant; other hosts keep theirs", () => {
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
    "--no-daemon",
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
  expect(() => agentArgs("opencode", "pb-1", "probe/m", "a;b", true)).toThrow(
    "unsafe opencode arg",
  );
});
