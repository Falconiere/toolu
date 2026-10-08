/**
 * The whole catalog starts on OpenCode (#342): all 12 plugins, their real
 * committed startup bundles, dependencies first, every registry module and
 * helper verified, startup context collected; a second startup changes nothing.
 */
import { expect, test } from "bun:test";
import { existsSync, lstatSync, mkdirSync, readdirSync, readFileSync, readlinkSync } from "node:fs";
import { join } from "node:path";
import { listPluginManifests } from "../../inventory/scan.ts";
import { selectPluginsByEnabledNames } from "../../select/resolve.ts";
import { bootstrapRuntime } from "../runtime.ts";
import type { ReadyResult } from "../result.ts";
import { PLUGINS_ROOT, REPO_ROOT, tempRoot } from "./fixtures.ts";

const MODULES = {
  "pre-tools.d": { "ast-grep@toolu__search-nudge.js": "ast-grep/hooks/dist/search-nudge.js" },
  "post-tools.d": {
    "ast-grep@toolu__byte-savings.js": "ast-grep/hooks/dist/byte-savings.js",
    "python-quality@toolu__python-quality.js": "python-quality/hooks/dist/post-tool-use.js",
    "rust-quality@toolu__rust-quality.js": "rust-quality/hooks/dist/post-tool-use.js",
    "ts-quality@toolu__ts-quality.js": "ts-quality/hooks/dist/post-tool-use.js",
  },
};

const HELPERS = {
  "jev/jev.sh": "jev/scripts/jev.sh",
  "statusline/statusline.sh": "statusline/hooks/dist/statusline.js",
  "toolu-review/write-state.sh": "toolu-review/hooks/dist/write-state.js",
};

const CATALOG = (listPluginManifests(PLUGINS_ROOT) ?? []).map((plugin) => plugin.name).toSorted();

function everyPlugin(): ReturnType<typeof selectPluginsByEnabledNames> {
  return selectPluginsByEnabledNames(PLUGINS_ROOT, CATALOG);
}

/** What a rewrite or relink would change: mtime and inode, of the file or the link itself. */
function stamp(path: string): [string, number, number] {
  const stat = lstatSync(path);
  return [path, stat.mtimeMs, stat.ino];
}

async function start(root: string): Promise<ReadyResult> {
  const selected = everyPlugin();
  if (!selected.ok) throw new Error(selected.reason);
  mkdirSync(join(root, "project"), { recursive: true });
  const result = await bootstrapRuntime({
    repoRoot: REPO_ROOT,
    projectRoot: join(root, "project"),
    dataRoot: join(root, "data"),
    plugins: selected.plugins,
    isolatedHome: join(root, "home"),
    env: {
      HOME: join(root, "home"),
      PATH: "/usr/bin:/bin",
      TOOLU_BIN: join(REPO_ROOT, "target/debug/toolu"),
      TOOLU_BUN: process.execPath,
    },
  });
  if (result.status !== "ready") throw new Error(result.reason);
  return result;
}

function expectCatalogContributions(data: string, result: ReadyResult): void {
  for (const [dir, modules] of Object.entries(MODULES)) {
    expect(readdirSync(join(data, "toolu", dir)).toSorted()).toEqual(Object.keys(modules));
    for (const [file, bundle] of Object.entries(modules)) {
      const bytes = readFileSync(join(data, "toolu", dir, file));
      expect(bytes.equals(readFileSync(join(PLUGINS_ROOT, bundle)))).toBe(true);
    }
  }
  for (const [path, source] of Object.entries(HELPERS)) {
    expect(readlinkSync(join(data, path))).toBe(join(PLUGINS_ROOT, source));
  }
  const expected = [
    ...Object.entries(MODULES).flatMap(([dir, modules]) =>
      Object.keys(modules).map((file) => join(data, "toolu", dir, file)),
    ),
    ...Object.keys(HELPERS).map((path) => join(data, path)),
  ];
  expect(result.artifacts.toSorted()).toEqual(expected.toSorted());
  expect(existsSync(join(data, "toolu", ".session-start-ready"))).toBe(false);
}

test("all 12 plugins start in dependency order with every contribution verified", async () => {
  using root = tempRoot("toolu-catalog-");
  const result = await start(root.path);
  const order = result.plugins.map((plugin) => plugin.plugin);
  expect(CATALOG).toHaveLength(12);
  expect(order.toSorted()).toEqual(CATALOG);
  for (const dependent of [
    "ts-quality",
    "python-quality",
    "rust-quality",
    "pr-babysit",
    "delivery-flow",
    "epic-orchestrator",
  ]) {
    expect(order.indexOf("toolu")).toBeLessThan(order.indexOf(dependent));
  }
  expect(order.indexOf("delivery-flow")).toBeLessThan(order.indexOf("epic-orchestrator"));
  const entries = Object.fromEntries(
    result.plugins.map((plugin) => [plugin.plugin, plugin.entries.map((e) => e.entry)]),
  );
  expect(entries["ts-quality"]).toEqual(["register", "check-toolu"]);
  expect(entries["python-quality"]).toEqual(["register", "check-toolu"]);
  expect(entries["rust-quality"]).toEqual(["register", "check-toolu"]);
  expect(entries["brainstorm"]).toEqual([]);
  const notices = result.plugins
    .flatMap((plugin) => plugin.entries)
    .flatMap((entry) => (entry.additionalContext === undefined ? [] : [entry.additionalContext]))
    .filter((line) => line.includes("native binary not found in the agent command shell"));
  expect(notices).toHaveLength(0);
  expectCatalogContributions(join(root.path, "data"), result);
  const context = (name: string): string =>
    result.plugins.find((plugin) => plugin.plugin === name)?.entries[0]?.additionalContext ?? "";
  expect(context("toolu")).toContain("Session Protocol");
  expect(context("jev")).toContain("Jev");
});

test("a second startup is idempotent: same artifacts, untouched files and ledger", async () => {
  using root = tempRoot("toolu-catalog-again-");
  const first = await start(root.path);
  const data = join(root.path, "data");
  const ledger = join(data, "toolu", "startup-ledger.json");
  const before = readFileSync(ledger, "utf8");
  const stamps = [ledger, ...first.artifacts].map(stamp);
  const second = await start(root.path);
  expect(second.artifacts).toEqual(first.artifacts);
  expect(second.plugins.map((p) => p.plugin)).toEqual(first.plugins.map((p) => p.plugin));
  expect(readFileSync(ledger, "utf8")).toBe(before);
  expect([ledger, ...first.artifacts].map(stamp)).toEqual(stamps);
  expectCatalogContributions(data, second);
});
