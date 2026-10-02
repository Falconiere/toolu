import { expect, test } from "bun:test";
import { cp, mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { Hooks } from "@opencode-ai/plugin";
import { definedEnv } from "../../host/runtime-env.ts";
import { parseOptions, type HostBinding, type LogLevel } from "../context.ts";
import { prepareEnforcement } from "../enforcement.ts";
import { createTooluHooks, startupNotes } from "../hooks.ts";

const tmpBase = process.env.TMPDIR ?? "/tmp";
const REPO_ROOT = join(dirname(fileURLToPath(import.meta.url)), "../../../../..");
const CALL = { sessionID: "ses_hooks", callID: "call_hooks" };

type Logged = { level: LogLevel; message: string };

/** A real temp project: `.env`, protectedFiles in block mode, only the core plugin enabled. */
async function project(): Promise<{ root: string; envPath: string }> {
  const root = await mkdtemp(join(tmpBase, "toolu-oc-hooks-"));
  const envPath = join(root, ".env");
  await writeFile(envPath, "SECRET=1\n", "utf8");
  await mkdir(join(root, ".opencode/toolu"), { recursive: true });
  await writeFile(
    join(root, ".opencode/toolu.config.json"),
    JSON.stringify({ version: 1, gates: { protectedFiles: { mode: "block" } } }),
  );
  await writeFile(
    join(root, ".opencode/toolu/plugins.json"),
    JSON.stringify({ version: 1, enabled: ["toolu"] }),
  );
  return { root, envPath };
}

function binding(
  root: string,
  logged: Logged[],
  overrides: Partial<HostBinding> = {},
): HostBinding {
  const env = definedEnv(process.env);
  delete env.TOOLU_REPO_ROOT;
  delete env.TOOLU_ROOT;
  delete env.TOOLU_CONFIG_DIR;
  delete env.TOOLU_OPENCODE_HOME;
  // Never the developer's own global config: an empty XDG config dir inside the project.
  env.XDG_CONFIG_HOME = join(root, ".xdg");
  env.TOOLU_BUN = process.execPath;
  return {
    directory: root,
    projectRoot: root,
    repoRootOption: REPO_ROOT,
    optionsError: undefined,
    env,
    log: (level, message) => {
      logged.push({ level, message });
      return Promise.resolve();
    },
    ...overrides,
  };
}

function failing(): never {
  throw new Error("first");
}

function throwing(): Promise<void> {
  throw new Error("log transport down");
}

async function refusal(hooks: Hooks, tool: string, args: unknown): Promise<string> {
  const before = hooks["tool.execute.before"];
  if (before === undefined) throw new Error("no tool.execute.before hook");
  try {
    await before({ tool, ...CALL }, { args });
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  return "allowed";
}

test("ready: a protected .env edit is refused, an allowed bash call runs, one ready diagnostic", async () => {
  const { root, envPath } = await project();
  const logged: Logged[] = [];
  const hooks = await createTooluHooks(binding(root, logged));
  expect(hooks["tool.execute.after"]).toBeDefined();
  const edit = { filePath: envPath, oldString: "1", newString: "2" };
  expect(await refusal(hooks, "edit", edit)).toMatch(/protected/i);
  expect(await refusal(hooks, "bash", { command: "echo ok", description: "x" })).toBe("allowed");
  expect(await readFile(envPath, "utf8")).toBe("SECRET=1\n");
  expect(logged).toEqual([
    { level: "info", message: "toolu: ready (1 plugins, 0 startup artifacts)" },
  ]);
  await hooks.dispose?.();
});

test("ready: shell.env adds toolu's roots to a bash call's env and keeps what is there", async () => {
  const { root } = await project();
  const hooks = await createTooluHooks(binding(root, []));
  const shellEnv = hooks["shell.env"];
  if (shellEnv === undefined) throw new Error("no shell.env hook");
  const output: { env: Record<string, string> } = { env: { KEEP: "1", PATH: "/usr/bin" } };
  await shellEnv({ cwd: root, sessionID: CALL.sessionID, callID: CALL.callID }, output);
  expect(output.env).toMatchObject({
    KEEP: "1",
    TOOLU_HOST_OVERRIDE: "opencode",
    TOOLU_CONFIG_DIR: join(root, ".opencode/toolu/state"),
    TOOLU_USER_CONFIG_DIR: join(root, ".xdg/opencode"),
    TOOLU_PROJECT_CONFIG_DIRNAME: ".opencode",
    TOOLU_SETTINGS_DIR: join(REPO_ROOT, "plugins/toolu/settings"),
    TOOLU_OPENCODE_ROOT: join(REPO_ROOT, "tools/toolu-opencode"),
    TOOLU_PLUGIN_ROOT: join(REPO_ROOT, "plugins/toolu"),
    TOOLU_PLUGIN_ROOT_TOOLU: join(REPO_ROOT, "plugins/toolu"),
  });
  expect(output.env.TOOLU_BUN).toBe(process.execPath);
  expect(output.env.TOOLU_OPENCODE_DATA_ROOT).toBe(join(root, ".opencode/toolu/state"));
  expect(output.env.PATH).toBe(`/usr/bin:${dirname(process.execPath)}`);
  expect(output.env.TOOLU_PROJECT_DIR).toBeUndefined();
  expect(output.env.HOME).toBeUndefined();
  await hooks.dispose?.();
});

test("a selected plugin whose startup fails refuses every tool with its plugin and cause", async () => {
  const { root } = await project();
  const catalog = await mkdtemp(join(tmpBase, "toolu-oc-hooks-catalog-"));
  await cp(join(REPO_ROOT, "plugins"), join(catalog, "plugins"), { recursive: true });
  await rm(join(catalog, "plugins/ts-quality/hooks/dist/post-tool-use.js"));
  await writeFile(
    join(root, ".opencode/toolu/plugins.json"),
    JSON.stringify({ version: 1, enabled: ["toolu", "ts-quality"] }),
  );
  const logged: Logged[] = [];
  const hooks = await createTooluHooks(binding(root, logged, { repoRootOption: catalog }));
  const reason = await refusal(hooks, "bash", { command: "touch never.txt" });
  expect(reason).toStartWith("toolu: not ready: bootstrap: ts-quality/register: ");
  expect(reason).toContain("ts-quality@toolu__ts-quality.js: bundle unreadable");
  expect(logged).toEqual([{ level: "error", message: `${reason}; every tool call is denied` }]);
  await hooks.dispose?.();
  await rm(catalog, { recursive: true, force: true });
});

test("a preparation that throws yields a hook that refuses every tool", async () => {
  const { root } = await project();
  const logged: Logged[] = [];
  const hooks = await createTooluHooks(binding(root, logged), () => {
    throw new Error("boom");
  });
  expect(await refusal(hooks, "read", { filePath: "/x" })).toBe(
    "toolu: not ready: setup failed: boom",
  );
  expect(hooks["shell.env"]).toBeUndefined();
  expect(logged).toEqual([
    { level: "error", message: "toolu: not ready: setup failed: boom; every tool call is denied" },
  ]);
  await hooks.dispose?.();
});

test("invalid plugin options are reported, never silently ignored", async () => {
  const { root } = await project();
  const logged: Logged[] = [];
  const parsed = parseOptions({ repoRoot: 42 });
  const hooks = await createTooluHooks(binding(root, logged, parsed));
  const reason = await refusal(hooks, "bash", { command: "echo ok" });
  expect(reason).toMatch(/^toolu: not ready: invalid plugin options: .*repoRoot/s);
  expect(logged[0]?.level).toBe("error");
  await hooks.dispose?.();
});

test("no repo root from options, env or a bundled catalog refuses every tool", async () => {
  const { root } = await project();
  const logged: Logged[] = [];
  // A packed or prepacked checkout carries tools/toolu-opencode/plugins; this case has none.
  const hooks = await createTooluHooks(binding(root, logged, { repoRootOption: undefined }), (b) =>
    prepareEnforcement(b, () => undefined),
  );
  expect(await refusal(hooks, "bash", { command: "echo ok" })).toBe(
    "toolu: not ready: no bundled plugins/ tree; set plugin option repoRoot or TOOLU_REPO_ROOT",
  );
  await hooks.dispose?.();
});

test("a repoRoot without a plugins tree refuses every tool with the selection reason", async () => {
  const { root } = await project();
  const missing = join(root, "missing");
  const hooks = await createTooluHooks(binding(root, [], { repoRootOption: missing }));
  expect(await refusal(hooks, "bash", { command: "echo ok" })).toMatch(
    /^toolu: not ready: plugin selection: /,
  );
  await hooks.dispose?.();
});

test("a second load for the same directory is skipped until the first is disposed", async () => {
  const { root } = await project();
  const logged: Logged[] = [];
  const first = await createTooluHooks(binding(root, logged), failing);
  const second = await createTooluHooks(binding(root, logged), failing);
  expect(second["tool.execute.before"]).toBeUndefined();
  await second.dispose?.();
  expect(await createTooluHooks(binding(root, logged), failing)).not.toHaveProperty(
    "tool.execute.before",
  );
  expect(logged.at(-1)).toEqual({
    level: "info",
    message: `toolu: duplicate load skipped for ${root}`,
  });
  await first.dispose?.();
  const third = await createTooluHooks(binding(root, logged), failing);
  expect(third["tool.execute.before"]).toBeDefined();
  await third.dispose?.();
});

test("a host log that throws cannot abort init or strand the directory claim", async () => {
  const { root } = await project();
  const first = await createTooluHooks(binding(root, [], { log: throwing }), failing);
  expect(await refusal(first, "read", { filePath: "/x" })).toBe(
    "toolu: not ready: setup failed: first",
  );
  await first.dispose?.();
  const again = await createTooluHooks(binding(root, [], { log: throwing }), failing);
  expect(again["tool.execute.before"]).toBeDefined();
  await again.dispose?.();
});

test("startup notes go to the host log as one bounded line", () => {
  expect(startupNotes([])).toBeUndefined();
  expect(startupNotes(["a: kept user file /x", "b: removed module /y"])).toBe(
    "toolu: startup notes: a: kept user file /x; b: removed module /y",
  );
  const many = [...Array(23).keys()].map((i) => `n${String(i)}`);
  const shown = many.slice(0, 20).join("; ");
  expect(startupNotes(many)).toBe(`toolu: startup notes: ${shown} (3 more)`);
});
