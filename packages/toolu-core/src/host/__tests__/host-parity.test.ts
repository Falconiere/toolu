/**
 * Differential parity with `plugins/toolu/hooks/lib/host.sh`: the same env and
 * cwd go to a real `bash` that sources host.sh and to the TypeScript port, and
 * every Claude/Codex branch must print the same value.
 */
import { afterAll, describe, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { mkdirSync, mkdtempSync, realpathSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { detectHost } from "../host-detect.ts";
import type { HostEnv } from "../host-name.ts";
import {
  configRoot,
  invocation,
  pluginData,
  pluginInstallCommand,
  pluginRoot,
  projectConfigPath,
  projectDirname,
  projectRoot,
  projectStateDir,
  projectStateRoot,
} from "../host-roots.ts";

const HOST_SH = resolve(import.meta.dir, "../../../../../plugins/toolu/hooks/lib/host.sh");
const root = realpathSync(mkdtempSync(join(tmpdir(), "host-parity-")));
afterAll(() => rmSync(root, { recursive: true, force: true }));

const home = join(root, "home");
const repo = join(root, "repo");
const outside = join(root, "outside");
for (const dir of [home, join(repo, "sub"), outside]) mkdirSync(dir, { recursive: true });
expect(spawnSync("git", ["init", "-q", repo]).status).toBe(0);

/** Each bash function call, in the order both sides print them. */
const CALLS = [
  "toolu_host",
  "toolu_config_root",
  "toolu_project_root",
  "toolu_project_dirname",
  "toolu_project_config",
  "toolu_project_state_root",
  "toolu_project_state_dir telemetry",
  "toolu_plugin_root",
  "toolu_plugin_data",
  "toolu_invocation toolu setup",
  "toolu_plugin_install_command toolu@toolu",
] as const;

function bashValues(env: HostEnv, cwd: string): string[] {
  const script = `. "$1"; ${CALLS.map((call) => `printf '%s\\n' "$(${call} 2>/dev/null)"`).join("; ")}`;
  const res = spawnSync("bash", ["-c", script, "_", HOST_SH], {
    cwd,
    env: bashEnv(env),
    encoding: "utf8",
  });
  expect(res.status).toBe(0);
  return res.stdout.split("\n").slice(0, CALLS.length);
}

function bashEnv(env: HostEnv): Record<string, string> {
  const out: Record<string, string> = { PATH: process.env.PATH ?? "/usr/bin:/bin", HOME: home };
  for (const [key, value] of Object.entries(env)) {
    if (value !== undefined) out[key] = value;
  }
  return out;
}

function tsValues(envIn: HostEnv, cwd: string): string[] {
  const env = bashEnv(envIn);
  const o = { env, cwd, host: detectHost({ env, warn: () => {} }) };
  return [
    o.host,
    configRoot(o),
    projectRoot(o) ?? "",
    projectDirname(o),
    projectConfigPath(o) ?? "",
    projectStateRoot(o) ?? "",
    projectStateDir("telemetry", o) ?? "",
    pluginRoot(o) ?? "",
    pluginData(o) ?? "",
    invocation("toolu", "setup", o),
    pluginInstallCommand("toolu@toolu", o) ?? "",
  ];
}

const ROWS: ReadonlyArray<readonly [string, HostEnv]> = [
  ["no host signal (Claude default)", {}],
  [
    "Codex plugin hook with Claude compatibility vars",
    {
      PLUGIN_ROOT: "/cx/root",
      PLUGIN_DATA: "/cx/data",
      CLAUDE_PLUGIN_ROOT: "/c/root",
      CLAUDE_PLUGIN_DATA: "/c/data",
    },
  ],
  ["override codex", { TOOLU_HOST_OVERRIDE: "codex", CODEX_HOME: "/codex-home" }],
  ["override claude beats PLUGIN_ROOT", { TOOLU_HOST_OVERRIDE: "claude", PLUGIN_ROOT: "/cx" }],
  ["invalid override falls back", { TOOLU_HOST_OVERRIDE: "bogus", PLUGIN_ROOT: "/cx" }],
  [
    "Claude-native roots",
    {
      CLAUDE_CONFIG_DIR: "/cc",
      CLAUDE_PROJECT_DIR: "/cp",
      CLAUDE_PLUGIN_ROOT: "/c/root",
      CLAUDE_PLUGIN_DATA: "/c/data",
    },
  ],
  ["Codex ignores CLAUDE_PROJECT_DIR", { PLUGIN_ROOT: "/cx", CLAUDE_PROJECT_DIR: "/cp" }],
  [
    "explicit TOOLU_* overrides",
    {
      TOOLU_CONFIG_DIR: "/tc",
      TOOLU_PROJECT_DIR: "/tp",
      TOOLU_PROJECT_CONFIG_DIRNAME: ".state",
      CODEX_HOME: "/wrong",
      PLUGIN_ROOT: "/cx",
    },
  ],
  [
    "empty values count as unset",
    { TOOLU_HOST_OVERRIDE: "", PLUGIN_ROOT: "", CLAUDE_PROJECT_DIR: "", TOOLU_CONFIG_DIR: "" },
  ],
];

describe("host.sh parity", () => {
  for (const [name, env] of ROWS) {
    for (const [where, cwd] of [
      ["in a git repo subdir", join(repo, "sub")],
      ["outside git", outside],
    ] as const) {
      test(`${name}, ${where}`, () => {
        expect(tsValues(env, cwd)).toEqual(bashValues(env, cwd));
      });
    }
  }

  test("the invalid-override warning text matches bash", () => {
    const env = { TOOLU_HOST_OVERRIDE: "bogus" };
    const res = spawnSync("bash", ["-c", '. "$1"; toolu_host >/dev/null', "_", HOST_SH], {
      env: bashEnv(env),
      encoding: "utf8",
    });
    const warnings: string[] = [];
    detectHost({ env, warn: (line) => warnings.push(line) });
    expect(warnings.map((line) => `${line}\n`).join("")).toBe(res.stderr);
  });
});
