import { expect, test } from "bun:test";
import { mkdir, mkdtemp, readFile, writeFile } from "node:fs/promises";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";
import type { Hooks } from "@opencode-ai/plugin";
import { definedEnv, parseOptions, type HostBinding, type LogLevel } from "../context.ts";
import { prepareEnforcement } from "../enforcement.ts";
import { createTooluHooks } from "../hooks.ts";

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
  const edit = { filePath: envPath, oldString: "1", newString: "2" };
  expect(await refusal(hooks, "edit", edit)).toMatch(/protected/i);
  expect(await refusal(hooks, "bash", { command: "echo ok", description: "x" })).toBe("allowed");
  expect(await readFile(envPath, "utf8")).toBe("SECRET=1\n");
  expect(logged).toHaveLength(1);
  expect(logged[0]?.level).toBe("info");
  expect(logged[0]?.message).toMatch(/^toolu: ready \(\d+ bootstrap artifacts\)$/);
  await hooks.dispose?.();
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
