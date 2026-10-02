/**
 * Projects never share startup contributions (#343). Under one explicit
 * override each project starts in its own keyed data root, so a project with
 * fewer plugins cannot prune another's modules; two sessions starting at once
 * in one project both end ready with a valid ledger.
 */
import { expect, test } from "bun:test";
import { mkdirSync, readdirSync, readFileSync } from "node:fs";
import { join } from "node:path";
import { opencodeDataRoot } from "../../host/roots.ts";
import { selectPluginsByEnabledNames } from "../../select/resolve.ts";
import { bootstrapRuntime } from "../runtime.ts";
import type { BootstrapResult } from "../result.ts";
import { PLUGINS_ROOT, REPO_ROOT, tempRoot } from "./fixtures.ts";

const AST_GREP_MODULES: Record<string, string> = {
  "pre-tools.d/ast-grep@toolu__search-nudge.js": "ast-grep/hooks/dist/search-nudge.js",
  "post-tools.d/ast-grep@toolu__byte-savings.js": "ast-grep/hooks/dist/byte-savings.js",
};

function start(
  project: string,
  names: string[],
  env: Record<string, string>,
): Promise<BootstrapResult> {
  const selected = selectPluginsByEnabledNames(PLUGINS_ROOT, names);
  if (!selected.ok) throw new Error(selected.reason);
  mkdirSync(project, { recursive: true });
  return bootstrapRuntime({
    repoRoot: REPO_ROOT,
    projectRoot: project,
    plugins: selected.plugins,
    env,
  });
}

function modules(data: string): string[] {
  return ["pre-tools.d", "post-tools.d"].flatMap((dir) => {
    try {
      return readdirSync(join(data, "toolu", dir)).map((file) => `${dir}/${file}`);
    } catch {
      return [];
    }
  });
}

test("under one TOOLU_CONFIG_DIR, a later project cannot prune an earlier one's modules", async () => {
  using root = tempRoot("toolu-shared-override-");
  const env = {
    HOME: join(root.path, "home"),
    TOOLU_BUN: process.execPath,
    TOOLU_CONFIG_DIR: join(root.path, "shared"),
  };
  const a = join(root.path, "project a");
  const b = join(root.path, "project b");
  const first = await start(a, ["toolu", "ast-grep"], env);
  const second = await start(b, ["toolu"], env);
  expect(first.status).toBe("ready");
  expect(second.status).toBe("ready");
  const dataA = opencodeDataRoot({ projectRoot: a, env });
  const dataB = opencodeDataRoot({ projectRoot: b, env });
  expect(dataA).not.toBe(dataB);
  for (const [file, bundle] of Object.entries(AST_GREP_MODULES)) {
    const bytes = readFileSync(join(dataA, "toolu", file));
    expect(bytes.equals(readFileSync(join(PLUGINS_ROOT, bundle)))).toBe(true);
  }
  expect(modules(dataB).filter((file) => file.includes("ast-grep@toolu__"))).toEqual([]);
  expect(readdirSync(join(root.path, "shared"))).toEqual(["toolu"]);
}, 120_000);

test("two sessions starting at once in one project are both ready with a valid ledger", async () => {
  using root = tempRoot("toolu-concurrent-start-");
  const env = { HOME: join(root.path, "home"), TOOLU_BUN: process.execPath };
  const project = join(root.path, "project");
  const names = ["toolu", "ast-grep", "context7"];
  const [one, two] = await Promise.all([start(project, names, env), start(project, names, env)]);
  expect(one?.status).toBe("ready");
  expect(two?.status).toBe("ready");
  const data = opencodeDataRoot({ projectRoot: project, env });
  const ledger: unknown = JSON.parse(
    readFileSync(join(data, "toolu", "startup-ledger.json"), "utf8"),
  );
  expect(ledger).toEqual({
    version: 1,
    plugins: {
      context7: {
        helpers: [
          {
            path: join(data, "context7", "search.sh"),
            source: join(PLUGINS_ROOT, "context7/hooks/dist/search.js"),
          },
        ],
      },
    },
  });
  expect(modules(data).toSorted()).toEqual(Object.keys(AST_GREP_MODULES).toSorted());
}, 120_000);
