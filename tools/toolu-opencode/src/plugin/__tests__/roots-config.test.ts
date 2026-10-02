/**
 * Config and gate state on their OpenCode roots (#343), through the real
 * hooks: the global config comes from OpenCode's config directory or the
 * explicit override, the project config wins over it, and one project's
 * failing gate never reaches another project.
 */
import { expect, test } from "bun:test";
import { mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import type { Hooks } from "@opencode-ai/plugin";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { opencodeDataRoot } from "../../host/roots.ts";
import { definedEnv, type HostBinding } from "../context.ts";
import { createTooluHooks } from "../hooks.ts";

const REPO_ROOT = join(import.meta.dir, "../../../../..");
const CALL = { sessionID: "ses_roots", callID: "call_roots" };
const ENV_BYTES = "SECRET=1\n";
const COMMIT = { command: "git commit --allow-empty -m 'fix: roots'", description: "commit" };

function hostEnv(extra: Record<string, string>): Record<string, string> {
  const env = definedEnv(process.env);
  for (const key of ["TOOLU_REPO_ROOT", "TOOLU_ROOT", "TOOLU_CONFIG_DIR", "TOOLU_OPENCODE_HOME"])
    delete env[key];
  return { ...env, TOOLU_BUN: process.execPath, ...extra };
}

function binding(dir: string, env: Record<string, string>): HostBinding {
  return {
    directory: dir,
    projectRoot: dir,
    repoRootOption: REPO_ROOT,
    optionsError: undefined,
    env,
    log: () => Promise.resolve(),
  };
}

/** A git project that enables only toolu and has a `.env` to protect. */
function project(): Sandbox {
  return createSandbox({
    git: true,
    files: {
      ".env": ENV_BYTES,
      ".opencode/toolu/plugins.json": JSON.stringify({ version: 1, enabled: ["toolu"] }),
    },
  });
}

function writeConfig(dir: string, mode: string): void {
  mkdirSync(dir, { recursive: true });
  writeFileSync(
    join(dir, "toolu.config.json"),
    JSON.stringify({ version: 1, gates: { protectedFiles: { mode } } }),
  );
}

async function verdict(hooks: Hooks, tool: string, args: unknown): Promise<string> {
  const before = hooks["tool.execute.before"];
  if (before === undefined) throw new Error("no tool.execute.before hook");
  try {
    await before({ tool, ...CALL }, { args });
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  return "allowed";
}

/** Fresh hooks for one call, disposed after, so each case reads the config files anew. */
async function decide(
  dir: string,
  env: Record<string, string>,
  tool: string,
  args: unknown,
): Promise<string> {
  const hooks = await createTooluHooks(binding(dir, env));
  try {
    return await verdict(hooks, tool, args);
  } finally {
    await hooks.dispose?.();
  }
}

/**
 * Each layer is checked in both directions, `block` refusing and `off` allowing,
 * so the case discriminates whatever the default `ask` degrades to.
 */
test("the global config comes from OpenCode's config dir, the project config wins, the override replaces it", async () => {
  using sb = project();
  const xdg = join(sb.root, "xdg config");
  const global = join(xdg, "opencode");
  const projectDir = join(sb.project, ".opencode");
  const override = join(sb.root, "toolu override");
  const edit = { filePath: join(sb.project, ".env"), oldString: "1", newString: "2" };
  const env = hostEnv({ XDG_CONFIG_HOME: xdg });
  const overridden = hostEnv({ XDG_CONFIG_HOME: xdg, TOOLU_CONFIG_DIR: override });
  const refused = /protected/i;

  writeConfig(global, "block");
  expect(await decide(sb.project, env, "edit", edit)).toMatch(refused);
  writeConfig(global, "off");
  expect(await decide(sb.project, env, "edit", edit)).toBe("allowed");

  writeConfig(projectDir, "block");
  expect(await decide(sb.project, env, "edit", edit)).toMatch(refused);
  writeConfig(global, "block");
  writeConfig(projectDir, "off");
  expect(await decide(sb.project, env, "edit", edit)).toBe("allowed");
  rmSync(join(projectDir, "toolu.config.json"));

  writeConfig(override, "off");
  expect(await decide(sb.project, overridden, "edit", edit)).toBe("allowed");
  writeConfig(global, "off");
  writeConfig(override, "block");
  expect(await decide(sb.project, overridden, "edit", edit)).toMatch(refused);

  const keyed = opencodeDataRoot({ projectRoot: sb.project, env: overridden });
  expect(keyed.startsWith(join(override, "toolu", "opencode", "projects"))).toBe(true);
  expect(readFileSync(join(keyed, "toolu", "startup-ledger.json"), "utf8")).toContain('"version"');
  expect(sb.read(".env")).toBe(ENV_BYTES);
}, 180_000);

test("one project's failing quality gate refuses its commit and never reaches another project", async () => {
  using a = project();
  using b = project();
  const gate = join(a.project, ".opencode", "tmp", "quality-gate-status.json");
  mkdirSync(join(a.project, ".opencode", "tmp"), { recursive: true });
  const failing = JSON.stringify({ status: "failing", reason: "project a is failing" });
  writeFileSync(gate, failing);
  const env = hostEnv({ XDG_CONFIG_HOME: join(a.root, "xdg") });
  const [inA, inB] = await Promise.all([
    decide(a.project, env, "bash", COMMIT),
    decide(b.project, env, "bash", COMMIT),
  ]);
  expect(inA).toContain("project a is failing");
  expect(inB).toBe("allowed");
  expect(readFileSync(gate, "utf8")).toBe(failing);
}, 120_000);
