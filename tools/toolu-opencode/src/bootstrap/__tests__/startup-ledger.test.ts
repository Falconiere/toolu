/**
 * Disabled plugins disappear (#342): what toolu published for a plugin that is
 * no longer selected is taken back, while user files and other plugins'
 * modules stay where they are.
 */
import { expect, test } from "bun:test";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readdirSync,
  readFileSync,
  rmSync,
  symlinkSync,
  writeFileSync,
} from "node:fs";
import { join } from "node:path";
import { bootstrapRuntime } from "../runtime.ts";
import type { ReadyResult } from "../result.ts";
import { PLUGINS_ROOT, REPO_ROOT, catalogPlugin, tempRoot } from "./fixtures.ts";

async function start(root: string, names: string[]): Promise<ReadyResult> {
  mkdirSync(join(root, "project"), { recursive: true });
  const result = await bootstrapRuntime({
    repoRoot: REPO_ROOT,
    projectRoot: join(root, "project"),
    dataRoot: join(root, "data"),
    plugins: names.map(catalogPlugin),
    isolatedHome: join(root, "home"),
  });
  if (result.status !== "ready") throw new Error(result.reason);
  return result;
}

function ledger(root: string): unknown {
  return JSON.parse(readFileSync(join(root, "data/toolu/startup-ledger.json"), "utf8"));
}

test.concurrent("deselected plugins lose their modules and helpers; other files stay", async () => {
  using root = tempRoot("toolu-ledger-disable-");
  const data = join(root.path, "data");
  const unrelated = join(data, "toolu/pre-tools.d/custom@local__extra.js");
  mkdirSync(join(unrelated, ".."), { recursive: true });
  writeFileSync(unrelated, "export default {};\n");
  await start(root.path, ["toolu", "ast-grep", "context7"]);
  expect(readdirSync(join(data, "toolu/pre-tools.d")).toSorted()).toEqual([
    "ast-grep@toolu__search-nudge.js",
    "custom@local__extra.js",
  ]);
  const helper = join(data, "context7/search.sh");
  expect(ledger(root.path)).toEqual({
    version: 1,
    plugins: {
      context7: {
        spec: "context7@toolu",
        helpers: [{ path: helper, source: join(PLUGINS_ROOT, "context7/hooks/dist/search.js") }],
      },
    },
  });

  const second = await start(root.path, ["toolu"]);
  expect(readdirSync(join(data, "toolu/pre-tools.d"))).toEqual(["custom@local__extra.js"]);
  expect(readdirSync(join(data, "toolu/post-tools.d"))).toEqual([]);
  expect(existsSync(helper)).toBe(false);
  expect(second.diagnostics).toContain(`context7: removed helper ${helper}`);
  expect(second.diagnostics).toContain(
    `ast-grep: removed module ${join(data, "toolu/post-tools.d/ast-grep@toolu__byte-savings.js")}`,
  );
  expect(ledger(root.path)).toEqual({ version: 1, plugins: {} });
});

test.concurrent("a user file at a deselected plugin's helper path is kept and reported", async () => {
  using root = tempRoot("toolu-ledger-user-");
  await start(root.path, ["toolu", "jira"]);
  const helper = join(root.path, "data/jira/jira.sh");
  rmSync(helper);
  writeFileSync(helper, "#!/bin/sh\necho mine\n");
  const second = await start(root.path, ["toolu"]);
  expect(lstatSync(helper).isFile()).toBe(true);
  expect(readFileSync(helper, "utf8")).toBe("#!/bin/sh\necho mine\n");
  expect(second.diagnostics).toContain(`jira: kept ${helper}, no longer toolu's`);
  expect(ledger(root.path)).toEqual({ version: 1, plugins: {} });
});

test.concurrent("a selected plugin's helper it no longer publishes is retired", async () => {
  using root = tempRoot("toolu-ledger-stale-");
  await start(root.path, ["toolu", "context7"]);
  const source = join(PLUGINS_ROOT, "context7/hooks/dist/search.js");
  const old = join(root.path, "data/context7/old-search.sh");
  symlinkSync(source, old);
  const path = join(root.path, "data/toolu/startup-ledger.json");
  const recorded = { path: join(root.path, "data/context7/search.sh"), source };
  const helpers = [recorded, { path: old, source }];
  writeFileSync(
    path,
    JSON.stringify({ version: 1, plugins: { context7: { spec: "context7@toolu", helpers } } }),
  );
  const second = await start(root.path, ["toolu", "context7"]);
  expect(existsSync(old)).toBe(false);
  expect(second.diagnostics).toContain(`context7: removed helper ${old}`);
  expect(ledger(root.path)).toEqual({
    version: 1,
    plugins: { context7: { spec: "context7@toolu", helpers: [recorded] } },
  });
});

test.concurrent("an invalid ledger is ignored with a diagnostic and rewritten", async () => {
  using root = tempRoot("toolu-ledger-invalid-");
  const path = join(root.path, "data/toolu/startup-ledger.json");
  mkdirSync(join(path, ".."), { recursive: true });
  writeFileSync(path, '{"version": 2}');
  const result = await start(root.path, ["toolu", "exa-search"]);
  expect(result.diagnostics).toContain(`startup ledger ${path} invalid; ignored`);
  expect(ledger(root.path)).toMatchObject({
    version: 1,
    plugins: { "exa-search": { spec: "exa-search@toolu" } },
  });
});

test.concurrent("a ledger path outside the data root is never removed", async () => {
  using root = tempRoot("toolu-ledger-outside-");
  const source = join(PLUGINS_ROOT, "jira/hooks/dist/jira.js");
  const victim = join(root.path, "elsewhere.sh");
  symlinkSync(source, victim);
  const path = join(root.path, "data/toolu/startup-ledger.json");
  mkdirSync(join(path, ".."), { recursive: true });
  const helpers = [{ path: victim, source }];
  writeFileSync(
    path,
    JSON.stringify({ version: 1, plugins: { jira: { spec: "jira@toolu", helpers } } }),
  );
  const result = await start(root.path, ["toolu"]);
  expect(lstatSync(victim).isSymbolicLink()).toBe(true);
  expect(result.diagnostics).toContain(`jira: ignored ledger path ${victim} outside the data root`);
});
