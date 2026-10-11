// Shared sandbox pieces for the gate-reach suites: a small repo every tool reaches.
import { resolve } from "node:path";
import { run } from "@toolu/conformance/harness/spawn";
import type { RunResult } from "@toolu/conformance/harness/spawn";
import type { Sandbox } from "@toolu/conformance/harness/sandbox";

export const ROOT = resolve(import.meta.dir, "../../../..");
const SCRIPT = resolve(ROOT, "tooling/src/check-gate-reach.ts");
export const BIN_PATH = `${resolve(ROOT, "node_modules/.bin")}:${process.env["PATH"] ?? ""}`;

type Json = Record<string, unknown>;
export type RepoConfigs = {
  tsconfig: Json;
  formatCheck?: string;
  oxlint: Json;
  jscpd: Json;
  knip: Json;
  reach: Json;
};

/** Configs under which packages/a/src and packages/a/scripts are reached by all five tools. */
export function reachedConfigs(): RepoConfigs {
  return {
    tsconfig: { include: ["packages/a/src/**/*.ts", "packages/a/scripts/**/*.ts"] },
    formatCheck: "oxfmt --check packages/*/src packages/a/scripts/*.ts",
    oxlint: {},
    jscpd: { path: ["packages/a/src", "packages/a/scripts"], ignore: ["**/__tests__/**"] },
    knip: { workspaces: { "packages/a": { project: ["src/**/*.ts", "scripts/**/*.ts"] } } },
    reach: { version: 1, exclude: [], allowances: [] },
  };
}

function json(value: unknown): string {
  return `${JSON.stringify(value, null, 2)}\n`;
}

function packageScripts(configs: RepoConfigs): Record<string, string> {
  return configs.formatCheck === undefined ? {} : { "format:check": configs.formatCheck };
}

/** The tracked files of a repo using `configs`, with one module in src/ and one in scripts/. */
export function repoFiles(configs: RepoConfigs): Record<string, string> {
  return {
    "package.json": json({ scripts: packageScripts(configs) }),
    "tsconfig.json": json(configs.tsconfig),
    ".jscpd.json": json(configs.jscpd),
    "knip.json": json(configs.knip),
    "tooling/gate-reach.json": json(configs.reach),
    "packages/a/.oxlintrc.json": json(configs.oxlint),
    "packages/a/src/a.ts": "export const a = 1;\n",
    "packages/a/scripts/x.ts": "export const x = 1;\n",
  };
}

export function runReach(sb: Sandbox): Promise<RunResult> {
  return run([process.execPath, SCRIPT], {
    cwd: sb.project,
    env: { GATE_REACH_ROOT: sb.project, PATH: BIN_PATH },
  });
}
