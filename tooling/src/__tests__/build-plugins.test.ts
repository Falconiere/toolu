import { afterEach, expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import {
  chmodSync,
  existsSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import {
  buildPlugins,
  checkPluginBundles,
  discoverEntries,
  stageBundles,
} from "../build-plugins.ts";

const ROOT = resolve(import.meta.dir, "../../..");
const CLI = join(ROOT, "tooling/src/build-plugins.ts");
const temps: string[] = [];

function temp(): string {
  const dir = mkdtempSync(join(tmpdir(), "build-plugins-"));
  temps.push(dir);
  return dir;
}

function write(root: string, path: string, body: string): void {
  mkdirSync(dirname(join(root, path)), { recursive: true });
  writeFileSync(join(root, path), body);
}

/** A plugin tree with one entry, one helper module, and files that are not entries. */
function demoTree(): string {
  const root = temp();
  write(
    root,
    "plugins/demo/hooks/src/entry.ts",
    'import { greet } from "./lib/helper.ts";\nprocess.stdout.write(greet("demo"));\n',
  );
  write(
    root,
    "plugins/demo/hooks/src/lib/helper.ts",
    "export function greet(name: string): string {\n  return `hi ${name}\\n`;\n}\n",
  );
  write(root, "plugins/demo/hooks/src/types.d.ts", "export type Unused = string;\n");
  write(root, "plugins/demo/hooks/src/entry.test.ts", "export {};\n");
  return root;
}

afterEach(() => {
  for (const dir of temps.splice(0)) rmSync(dir, { recursive: true, force: true });
});

test("discovers top-level entries only, skipping helpers, tests and declarations", () => {
  expect(discoverEntries(demoTree())).toEqual([
    { plugin: "demo", name: "entry", source: "plugins/demo/hooks/src/entry.ts" },
  ]);
});

test("a tree with no hooks/src has nothing to build and no drift", () => {
  const root = temp();
  write(root, "plugins/empty/README.md", "# empty\n");
  expect(discoverEntries(root)).toEqual([]);
  expect(checkPluginBundles(root)).toEqual([]);
});

test("drift check fails after a source edit and passes after a rebuild", () => {
  const root = demoTree();
  buildPlugins(root);
  expect(existsSync(join(root, "plugins/demo/hooks/dist/entry.js"))).toBe(true);
  expect(existsSync(join(root, "plugins/demo/hooks/dist/helper.js"))).toBe(false);
  expect(checkPluginBundles(root)).toEqual([]);

  write(
    root,
    "plugins/demo/hooks/src/lib/helper.ts",
    "export function greet(name: string): string {\n  return `hello ${name}\\n`;\n}\n",
  );
  expect(checkPluginBundles(root)).toEqual([
    { kind: "drift", path: "plugins/demo/hooks/dist/entry.js" },
  ]);

  buildPlugins(root);
  expect(checkPluginBundles(root)).toEqual([]);
});

test("a removed source leaves an orphan that the next build deletes; a new entry is missing", () => {
  const root = demoTree();
  write(root, "plugins/demo/hooks/src/second.ts", 'process.stdout.write("second\\n");\n');
  buildPlugins(root);
  rmSync(join(root, "plugins/demo/hooks/src/second.ts"));
  write(root, "plugins/other/hooks/src/fresh.ts", 'process.stdout.write("fresh\\n");\n');

  expect(checkPluginBundles(root)).toEqual([
    { kind: "orphan", path: "plugins/demo/hooks/dist/second.js" },
    { kind: "missing", path: "plugins/other/hooks/dist/fresh.js" },
  ]);

  buildPlugins(root);
  expect(existsSync(join(root, "plugins/demo/hooks/dist/second.js"))).toBe(false);
  expect(checkPluginBundles(root)).toEqual([]);
});

test("removing a plugin's last entry removes its hooks/dist directory on the next build", () => {
  const root = demoTree();
  write(root, "plugins/other/hooks/src/only.ts", 'process.stdout.write("only\\n");\n');
  buildPlugins(root);
  expect(existsSync(join(root, "plugins/other/hooks/dist/only.js"))).toBe(true);

  rmSync(join(root, "plugins/other/hooks/src/only.ts"));
  buildPlugins(root);
  expect(existsSync(join(root, "plugins/other/hooks/dist"))).toBe(false);
  expect(existsSync(join(root, "plugins/demo/hooks/dist/entry.js"))).toBe(true);
  expect(checkPluginBundles(root)).toEqual([]);
});

test("a failing entry rejects and leaves the committed bundles untouched", () => {
  const root = demoTree();
  buildPlugins(root);
  const before = readFileSync(join(root, "plugins/demo/hooks/dist/entry.js"));
  write(root, "plugins/demo/hooks/src/lib/helper.ts", "export function greet( {\n");

  expect(() => buildPlugins(root)).toThrow("plugins/demo/hooks/src/entry.ts");
  expect(readFileSync(join(root, "plugins/demo/hooks/dist/entry.js"))).toEqual(before);
});

test("a shebang entry builds to an executable bundle; a plain entry does not", () => {
  const root = demoTree();
  write(
    root,
    "plugins/demo/hooks/src/tool.ts",
    '#!/usr/bin/env bun\nprocess.stdout.write("tool\\n");\n',
  );
  buildPlugins(root);
  const tool = join(root, "plugins/demo/hooks/dist/tool.js");
  expect(readFileSync(tool, "utf8")).toStartWith("#!/usr/bin/env bun\n");
  expect(statSync(tool).mode & 0o777).toBe(0o755);
  expect(statSync(join(root, "plugins/demo/hooks/dist/entry.js")).mode & 0o777).toBe(0o644);
  expect(spawnSync(tool, { encoding: "utf8" }).stdout).toBe("tool\n");
});

test("a shebang bundle that lost its exec bit is drift", () => {
  const root = demoTree();
  write(
    root,
    "plugins/demo/hooks/src/tool.ts",
    '#!/usr/bin/env bun\nprocess.stdout.write("tool\\n");\n',
  );
  buildPlugins(root);
  expect(checkPluginBundles(root)).toEqual([]);
  chmodSync(join(root, "plugins/demo/hooks/dist/tool.js"), 0o644);
  expect(checkPluginBundles(root)).toEqual([
    { kind: "drift", path: "plugins/demo/hooks/dist/tool.js" },
  ]);
});

test("the CLI exits non-zero on drift and names the stale bundle", () => {
  const root = demoTree();
  buildPlugins(root);
  write(root, "plugins/demo/hooks/src/entry.ts", 'process.stdout.write("changed\\n");\n');

  const run = spawnSync(process.execPath, [CLI, "--check", "--root", root], {
    cwd: temp(),
    encoding: "utf8",
  });
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("RED  drift plugins/demo/hooks/dist/entry.js");
});

test("the CLI rejects an unknown argument without touching committed bundles", () => {
  const root = demoTree();
  buildPlugins(root);
  const before = readFileSync(join(root, "plugins/demo/hooks/dist/entry.js"));
  write(root, "plugins/demo/hooks/src/entry.ts", 'process.stdout.write("changed\\n");\n');

  const run = spawnSync(process.execPath, [CLI, "--chek", "--root", root], {
    cwd: temp(),
    encoding: "utf8",
  });
  expect(run.status).toBe(1);
  expect(run.stderr).toContain("unknown argument: --chek");
  expect(readFileSync(join(root, "plugins/demo/hooks/dist/entry.js"))).toEqual(before);
});

test("repository bundles are byte-identical across builds and match the committed output", () => {
  const first = temp();
  const second = temp();
  const entries = stageBundles(ROOT, first);
  stageBundles(ROOT, second);
  expect(entries.length).toBeGreaterThan(0);
  for (const entry of entries) {
    const staged = readFileSync(join(first, entry.plugin, `${entry.name}.js`));
    expect(readFileSync(join(second, entry.plugin, `${entry.name}.js`))).toEqual(staged);
    const committed = join(ROOT, "plugins", entry.plugin, "hooks/dist", `${entry.name}.js`);
    expect(readFileSync(committed)).toEqual(staged);
  }
});

test("the CLI check passes on the repository regardless of the caller's cwd", () => {
  const run = spawnSync(process.execPath, [CLI, "--check", "--root", ROOT], {
    cwd: temp(),
    encoding: "utf8",
  });
  expect(run.stderr).toBe("");
  expect(run.status).toBe(0);
});

test("the committed sample bundle runs with no node_modules anywhere above it", () => {
  const dir = temp();
  for (let up = dir; up !== dirname(up); up = dirname(up)) {
    expect(existsSync(join(up, "node_modules"))).toBe(false);
  }
  writeFileSync(
    join(dir, "sample.js"),
    readFileSync(join(ROOT, "plugins/toolu/hooks/dist/sample.js")),
  );

  const run = spawnSync(process.execPath, ["sample.js"], { cwd: dir, encoding: "utf8" });
  expect(run.stderr).toBe("");
  expect(run.stdout).toBe('{"kind":"allow"}\n');
  expect(run.status).toBe(0);
});
