import { afterEach, expect, test } from "bun:test";
import { mkdtempSync, readFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import type { Hooks, PluginInput } from "@opencode-ai/plugin";
import { createOpencodeClient } from "@opencode-ai/sdk";
import * as entry from "../toolu.ts";
import { parseOptions, projectRootOf, type HostBinding } from "../context.ts";
import { resolveRepoRoot } from "../enforcement.ts";

const SRC = join(import.meta.dir, "../..");
const TYPED_SOURCES = [
  "plugin/toolu.ts",
  "plugin/hooks.ts",
  "plugin/context.ts",
  "plugin/enforcement.ts",
  "plugin/once.ts",
  "adapter/tool-before.ts",
];

/** Source with block and line comments removed, so prose like "as a plugin" is not a cast. */
function code(source: string): string {
  return source.replace(/\/\*[\s\S]*?\*\//g, "").replace(/^\s*\/\/.*$/gm, "");
}

test("the root module exports one default PluginModule and nothing else", () => {
  expect(Object.keys(entry)).toEqual(["default"]);
  expect(entry.default.id).toBe("toolu");
  expect(typeof entry.default.server).toBe("function");
});

test("entry sources use the pinned SDK types with no casts, any or suppressions", () => {
  for (const rel of TYPED_SOURCES) {
    const raw = readFileSync(join(SRC, rel), "utf8");
    const source = code(raw);
    expect({ rel, cast: /\bas (?!const\b)[A-Za-z{([]/.test(source) }).toEqual({ rel, cast: false });
    expect({ rel, any: /[:<]\s*any\b/.test(source) }).toEqual({ rel, any: false });
    expect({ rel, suppress: /@ts-(expect-error|ignore|nocheck)/.test(raw) }).toEqual({
      rel,
      suppress: false,
    });
    expect({ rel, v2: source.includes('"@opencode/plugin"') }).toEqual({ rel, v2: false });
  }
  const entrySource = readFileSync(join(SRC, "plugin/toolu.ts"), "utf8");
  expect(entrySource).toContain('import type { Plugin, PluginModule } from "@opencode-ai/plugin";');
});

test("a non-VCS project (worktree '/') is rooted at the instance directory", () => {
  expect(projectRootOf("/", "/home/u/scratch")).toBe("/home/u/scratch");
  expect(projectRootOf("/repo", "/repo/packages/a")).toBe("/repo");
});

test("plugin options: absent and valid pass, a non-string repoRoot is an error", () => {
  expect(parseOptions(undefined)).toEqual({ repoRootOption: undefined, optionsError: undefined });
  expect(parseOptions({ repoRoot: "/opt/toolu", other: 1 })).toEqual({
    repoRootOption: "/opt/toolu",
    optionsError: undefined,
  });
  const bad = parseOptions({ repoRoot: "" });
  expect(bad.repoRootOption).toBeUndefined();
  expect(bad.optionsError).toMatch(/^invalid plugin options: /);
});

const servers: Array<{ stop: (force?: boolean) => unknown }> = [];
afterEach(() => {
  for (const server of servers.splice(0)) server.stop(true);
});

/** Loopback host API recording each log body. */
function hostApi(): { url: string; bodies: unknown[] } {
  const bodies: unknown[] = [];
  const server = Bun.serve({
    port: 0,
    hostname: "127.0.0.1",
    fetch: async (request) => {
      bodies.push(await request.json());
      return Response.json(true);
    },
  });
  servers.push(server);
  return { url: server.url.href, bodies };
}

function pluginInput(client: PluginInput["client"], directory: string): PluginInput {
  return {
    client,
    directory,
    worktree: directory,
    project: { id: "proj_toolu_test", worktree: directory, time: { created: 0 } },
    experimental_workspace: { register: () => undefined },
    serverUrl: new URL("http://127.0.0.1:9"),
    $: Bun.$,
  };
}

async function refusal(hooks: Hooks): Promise<string> {
  const before = hooks["tool.execute.before"];
  if (before === undefined) throw new Error("no tool.execute.before hook");
  try {
    await before({ tool: "bash", sessionID: "s", callID: "c" }, { args: { command: "ls" } });
  } catch (error) {
    return error instanceof Error ? error.message : String(error);
  }
  return "allowed";
}

test("server: invalid options deny every call and the reason reaches the host log", async () => {
  const api = hostApi();
  const directory = mkdtempSync(join(tmpdir(), "toolu-oc-server-"));
  const hooks = await entry.default.server(
    pluginInput(createOpencodeClient({ baseUrl: api.url }), directory),
    { repoRoot: 42 },
  );
  expect(await refusal(hooks)).toMatch(/^toolu: not ready: invalid plugin options: /);
  expect(api.bodies).toHaveLength(1);
  expect(JSON.stringify(api.bodies[0])).toContain('"level":"error"');
  await hooks.dispose?.();
});

test("server: a binding failure still returns a hook that denies every call", async () => {
  const directory = mkdtempSync(join(tmpdir(), "toolu-oc-server-"));
  const input: PluginInput = {
    ...pluginInput(createOpencodeClient({ baseUrl: "http://127.0.0.1:9" }), directory),
    get client(): never {
      throw new Error("client unavailable");
    },
  };
  const hooks = await entry.default.server(input, undefined);
  expect(await refusal(hooks)).toBe("toolu: not ready: client unavailable");
  expect(await hooks.dispose?.()).toBeUndefined();
});

function bundled(): string {
  return "/bundled";
}

function rootBinding(repoRootOption: string | undefined, env: Record<string, string>): HostBinding {
  return {
    directory: "/w",
    projectRoot: "/w",
    repoRootOption,
    optionsError: undefined,
    env,
    log: () => Promise.resolve(),
  };
}

test("repo root precedence: option, TOOLU_REPO_ROOT, TOOLU_ROOT, then the bundled catalog", () => {
  const env = { TOOLU_REPO_ROOT: "/env-repo", TOOLU_ROOT: "/env-root" };
  expect(resolveRepoRoot(rootBinding("/option", env), bundled)).toBe("/option");
  expect(resolveRepoRoot(rootBinding(undefined, env), bundled)).toBe("/env-repo");
  expect(resolveRepoRoot(rootBinding(undefined, { TOOLU_ROOT: "/env-root" }), bundled)).toBe(
    "/env-root",
  );
  const empty = { TOOLU_REPO_ROOT: "", TOOLU_ROOT: "" };
  expect(resolveRepoRoot(rootBinding(undefined, empty), bundled)).toBe("/bundled");
  expect(resolveRepoRoot(rootBinding(undefined, empty), () => undefined)).toBeUndefined();
});
