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
import { PLUGINS_ROOT, REPO_ROOT, catalogPlugin, copiedPlugin, tempRoot } from "./fixtures.ts";

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
  writeFileSync(path, JSON.stringify({ version: 1, plugins: { context7: { helpers } } }));
  const second = await start(root.path, ["toolu", "context7"]);
  expect(existsSync(old)).toBe(false);
  expect(second.diagnostics).toContain(`context7: removed helper ${old}`);
  expect(ledger(root.path)).toEqual({
    version: 1,
    plugins: { context7: { helpers: [recorded] } },
  });
});

test.concurrent("an invalid ledger is ignored with a diagnostic and rewritten", async () => {
  using root = tempRoot("toolu-ledger-invalid-");
  const path = join(root.path, "data/toolu/startup-ledger.json");
  mkdirSync(join(path, ".."), { recursive: true });
  writeFileSync(path, '{"version": 2}');
  const result = await start(root.path, ["toolu", "exa-search"]);
  expect(result.diagnostics).toContain(`startup ledger ${path} invalid; ignored`);
  expect(ledger(root.path)).toEqual({
    version: 1,
    plugins: {
      "exa-search": {
        helpers: [
          {
            path: join(root.path, "data/exa-search/search.sh"),
            source: join(PLUGINS_ROOT, "exa-search/hooks/dist/search.js"),
          },
        ],
      },
    },
  });
});

/** Write a ledger by hand, the way a repository could commit one. */
function writeLedger(root: string, plugins: Record<string, unknown>): void {
  const path = join(root, "data/toolu/startup-ledger.json");
  mkdirSync(join(path, ".."), { recursive: true });
  writeFileSync(path, JSON.stringify({ version: 1, plugins }));
}

test.concurrent("a ledger path whose real location leaves the data root is never removed", async () => {
  using root = tempRoot("toolu-ledger-outside-");
  const source = join(PLUGINS_ROOT, "jira/hooks/dist/jira.js");
  const outside = join(root.path, "outside");
  mkdirSync(outside);
  const direct = join(outside, "direct.sh");
  const viaLink = join(outside, "via-link.sh");
  symlinkSync(source, direct);
  symlinkSync(source, viaLink);
  mkdirSync(join(root.path, "data"), { recursive: true });
  symlinkSync(outside, join(root.path, "data/escape"));
  const throughLink = join(root.path, "data/escape/via-link.sh");
  writeLedger(root.path, {
    jira: {
      helpers: [
        { path: direct, source },
        { path: throughLink, source },
      ],
    },
  });
  const result = await start(root.path, ["toolu"]);
  expect(lstatSync(direct).isSymbolicLink()).toBe(true);
  expect(lstatSync(viaLink).isSymbolicLink()).toBe(true);
  expect(result.diagnostics).toContain(`jira: ignored ledger path ${direct} outside the data root`);
  expect(result.diagnostics).toContain(
    `jira: ignored ledger path ${throughLink} outside the data root`,
  );
});

test.concurrent("a ledger cannot name modules outside a toolu plugin's own prefix", async () => {
  using root = tempRoot("toolu-ledger-spec-");
  const dir = join(root.path, "data/toolu/pre-tools.d");
  mkdirSync(dir, { recursive: true });
  writeFileSync(join(dir, "myorg__policy.js"), "export default {};\n");
  writeLedger(root.path, { myorg: { helpers: [] } });
  await start(root.path, ["toolu"]);
  expect(readdirSync(dir)).toEqual(["myorg__policy.js"]);
  writeLedger(root.path, { myorg: { spec: "myorg", helpers: [] } });
  const strict = await start(root.path, ["toolu"]);
  const path = join(root.path, "data/toolu/startup-ledger.json");
  expect(strict.diagnostics).toContain(`startup ledger ${path} invalid; ignored`);
  expect(readdirSync(dir)).toEqual(["myorg__policy.js"]);
});

test.concurrent("a registry directory that is really elsewhere is never pruned", async () => {
  using root = tempRoot("toolu-ledger-regdir-");
  const outside = join(root.path, "outside");
  mkdirSync(outside);
  writeFileSync(join(outside, "ast-grep@toolu__search-nudge.js"), "// not in the data root\n");
  mkdirSync(join(root.path, "data/toolu"), { recursive: true });
  symlinkSync(outside, join(root.path, "data/toolu/pre-tools.d"));
  await start(root.path, ["toolu"]);
  expect(readdirSync(outside)).toEqual(["ast-grep@toolu__search-nudge.js"]);
});

test.concurrent("only regular files under a deselected prefix are removed", async () => {
  using root = tempRoot("toolu-ledger-kinds-");
  await start(root.path, ["toolu", "ast-grep"]);
  const dir = join(root.path, "data/toolu/pre-tools.d");
  symlinkSync(
    join(PLUGINS_ROOT, "ast-grep/hooks/dist/search-nudge.js"),
    join(dir, "ast-grep@toolu__linked.js"),
  );
  mkdirSync(join(dir, "ast-grep@toolu__dir.js"));
  await start(root.path, ["toolu"]);
  expect(readdirSync(dir).toSorted()).toEqual([
    "ast-grep@toolu__dir.js",
    "ast-grep@toolu__linked.js",
  ]);
});

test.concurrent("a failed plugin keeps owning the helpers it published before", async () => {
  using root = tempRoot("toolu-ledger-failed-");
  await start(root.path, ["toolu", "context7"]);
  const before = ledger(root.path);
  const copies = join(root.path, "copies");
  const broken = copiedPlugin(copies, "context7");
  rmSync(join(broken.pluginDir, "hooks/dist/search.js"));
  mkdirSync(join(root.path, "project"), { recursive: true });
  const result = await bootstrapRuntime({
    repoRoot: REPO_ROOT,
    projectRoot: join(root.path, "project"),
    dataRoot: join(root.path, "data"),
    plugins: [catalogPlugin("toolu"), broken],
    isolatedHome: join(root.path, "home"),
  });
  expect(result.status).toBe("not-ready");
  expect(ledger(root.path)).toEqual(before);
});

test.concurrent("a user file already at a helper path is kept and reported at startup", async () => {
  using root = tempRoot("toolu-ledger-own-");
  const helper = join(root.path, "data/jira/jira.sh");
  mkdirSync(join(helper, ".."), { recursive: true });
  writeFileSync(helper, "#!/bin/sh\necho mine\n");
  const result = await start(root.path, ["toolu", "jira"]);
  expect(result.diagnostics).toContain(`jira: kept user file ${helper}`);
  expect(readFileSync(helper, "utf8")).toBe("#!/bin/sh\necho mine\n");
  expect(ledger(root.path)).toEqual({ version: 1, plugins: {} });
});

test.concurrent("a ledger path that is a directory leaves startup NotReady", async () => {
  using root = tempRoot("toolu-ledger-dir-");
  const path = join(root.path, "data/toolu/startup-ledger.json");
  mkdirSync(path, { recursive: true });
  mkdirSync(join(root.path, "project"), { recursive: true });
  const result = await bootstrapRuntime({
    repoRoot: REPO_ROOT,
    projectRoot: join(root.path, "project"),
    dataRoot: join(root.path, "data"),
    plugins: [catalogPlugin("toolu")],
    isolatedHome: join(root.path, "home"),
  });
  if (result.status === "ready") throw new Error("expected not-ready");
  expect(result.reason).toStartWith(`cannot write startup ledger ${path}: `);
});
