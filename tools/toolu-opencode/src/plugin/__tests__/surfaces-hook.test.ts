/** The `config` hook contributes the selected plugins' generated surfaces (#345). */
import { expect, test } from "bun:test";
import { cpSync, mkdirSync, mkdtempSync, realpathSync, rmSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import type { Hooks } from "@opencode-ai/plugin";
import { definedEnv } from "../../host/runtime-env.ts";
import { isPlainRecord } from "../../surfaces/merge.ts";
import type { HostBinding, LogLevel } from "../context.ts";
import { prepareEnforcement } from "../enforcement.ts";
import { createTooluHooks } from "../hooks.ts";

const REPO_ROOT = join(import.meta.dir, "../../../../..");
const GENERATED = realpathSync(join(REPO_ROOT, "tools/toolu-opencode/generated"));
const tmpBase = process.env.TMPDIR ?? "/tmp";
type Logged = { level: LogLevel; message: string };
type Config = Parameters<NonNullable<Hooks["config"]>>[0];

function put(path: string, text: string): void {
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, text);
}

function selection(names: string[]): string {
  return JSON.stringify({ version: 1, enabled: names });
}

/** A real project whose HOME and XDG config live inside it, so no developer file is read. */
function project(enabled?: string[]): string {
  const root = realpathSync(mkdtempSync(join(tmpBase, "toolu-oc-surfaces-")));
  if (enabled !== undefined) put(join(root, ".opencode/toolu/plugins.json"), selection(enabled));
  return root;
}

function binding(root: string, logged: Logged[]): HostBinding {
  const env = definedEnv(process.env);
  for (const key of ["TOOLU_REPO_ROOT", "TOOLU_ROOT", "TOOLU_CONFIG_DIR", "TOOLU_OPENCODE_HOME"])
    delete env[key];
  for (const key of Object.keys(env)) if (key.startsWith("OPENCODE_")) delete env[key];
  env.HOME = join(root, ".home");
  env.XDG_CONFIG_HOME = join(root, ".xdg");
  env.TOOLU_BUN = process.execPath;
  return {
    directory: root,
    worktree: root,
    projectRoot: root,
    repoRootOption: REPO_ROOT,
    optionsError: undefined,
    env,
    log: (level, message) => {
      logged.push({ level, message });
      return Promise.resolve();
    },
  };
}

function rec(value: unknown): Record<string, unknown> {
  if (!isPlainRecord(value)) throw new Error(`not a record: ${JSON.stringify(value)}`);
  return value;
}

async function applyConfig(hooks: Hooks, config: Config): Promise<void> {
  if (hooks.config === undefined) throw new Error("no config hook");
  await hooks.config(config);
}

test("ready: the config hook adds the selection's skills, agents and commands and logs them", async () => {
  const root = project(["toolu"]);
  const logged: Logged[] = [];
  const hooks = await createTooluHooks(binding(root, logged));
  const config: Config = {};
  await applyConfig(hooks, config);
  const paths = rec(Reflect.get(config, "skills")).paths;
  expect(paths).toEqual([
    join(GENERATED, "skills/toolu-commit-e11d9d00"),
    join(GENERATED, "skills/toolu-debug"),
    join(GENERATED, "skills/toolu-deep-research"),
    join(GENERATED, "skills/toolu-orchestrator"),
    join(GENERATED, "skills/toolu-review-and-commit-1a591621"),
    join(GENERATED, "skills/toolu-setup"),
  ]);
  expect(Object.keys(rec(config.agent))).toHaveLength(5);
  expect(Object.keys(rec(config.command))).toEqual([
    "toolu-commit-1e9b92d5",
    "toolu-review-and-commit-db159d0c",
  ]);
  expect(rec(config.permission)).toEqual({
    external_directory: { [`${GENERATED}/resources/*`]: "allow" },
  });
  const surfaces = logged.find((l) => l.message.startsWith("toolu: surfaces ("));
  expect(surfaces?.message.replace(/, \d+ ms\)/, ", <ms> ms)")).toBe(
    "toolu: surfaces (project selection, <ms> ms): " +
      "skills toolu-commit-e11d9d00, toolu-debug, toolu-deep-research, toolu-orchestrator, " +
      "toolu-review-and-commit-1a591621, toolu-setup; " +
      "agents toolu-architect, toolu-deep-explore, toolu-implementer, toolu-quick-task, " +
      "toolu-research-agent; commands toolu-commit-1e9b92d5, toolu-review-and-commit-db159d0c",
  );
  expect(logged.some((l) => l.message.startsWith("toolu: surface notes"))).toBe(false);
  await hooks.dispose?.();
});

test("global selection applies without a project file; overlaps are noted", async () => {
  const root = project();
  put(join(root, ".xdg/opencode/toolu/plugins.json"), selection(["jev", "gone"]));
  put(
    join(root, ".opencode/skills/jev-jev/SKILL.md"),
    "---\nname: jev-jev\ndescription: mine\n---\n",
  );
  const logged: Logged[] = [];
  const hooks = await createTooluHooks(binding(root, logged));
  const config: Config = {};
  await applyConfig(hooks, config);
  expect(Reflect.get(config, "skills")).toBeUndefined();
  const messages = logged.map((l) => l.message);
  expect(messages.find((m) => m.startsWith("toolu: surfaces ("))).toMatch(
    /^toolu: surfaces \(global selection, \d+ ms\): skills none; agents none; commands none$/,
  );
  expect(messages.find((m) => m.startsWith("toolu: startup notes"))).toContain(
    `enabled plugin "gone" in ${join(root, ".xdg/opencode/toolu/plugins.json")} is not installed`,
  );
  expect(messages.find((m) => m.startsWith("toolu: surface notes"))).toBe(
    `toolu: surface notes: skill jev-jev kept from ${join(root, ".opencode/skills/jev-jev/SKILL.md")}`,
  );
  await hooks.dispose?.();
});

test("a corrupt generated catalog or an invalid selection leaves toolu not ready with no config hook", async () => {
  const corrupt = join(realpathSync(mkdtempSync(join(tmpBase, "toolu-oc-gen-"))), "generated");
  cpSync(GENERATED, corrupt, { recursive: true });
  rmSync(join(corrupt, "agents/toolu-quick-task.md"));
  const root = project(["toolu"]);
  const logged: Logged[] = [];
  const hooks = await createTooluHooks(binding(root, logged), (b) =>
    prepareEnforcement(b, { generatedDir: corrupt }),
  );
  expect(hooks.config).toBeUndefined();
  expect(logged[0]?.message).toStartWith(
    `toolu: not ready: surfaces: catalog ${join(corrupt, "opencode.toolu.json")}: agents toolu-quick-task: missing file`,
  );
  await hooks.dispose?.();

  const invalid = project();
  put(join(invalid, ".opencode/toolu/plugins.json"), "{ broken");
  const invalidLog: Logged[] = [];
  const refused = await createTooluHooks(binding(invalid, invalidLog));
  expect(refused.config).toBeUndefined();
  expect(invalidLog[0]?.message).toContain(
    `toolu: not ready: plugin selection: invalid ${join(invalid, ".opencode/toolu/plugins.json")}`,
  );
  await refused.dispose?.();
});

test("a second instance on the same directory adds no config hook", async () => {
  const root = project(["toolu"]);
  const first = await createTooluHooks(binding(root, []));
  const second = await createTooluHooks(binding(root, []));
  expect(first.config).toBeDefined();
  expect(second.config).toBeUndefined();
  await second.dispose?.();
  await first.dispose?.();
});

test("a config the hook cannot read is reported, never thrown", async () => {
  const root = project(["toolu"]);
  const logged: Logged[] = [];
  const hooks = await createTooluHooks(binding(root, logged));
  const config: Config = {};
  Object.defineProperty(config, "skills", {
    get(): never {
      throw new Error("config getter failed");
    },
  });
  await applyConfig(hooks, config);
  expect(logged.at(-1)).toEqual({
    level: "error",
    message: "toolu: surfaces not applied: config getter failed",
  });
  await hooks.dispose?.();
});
