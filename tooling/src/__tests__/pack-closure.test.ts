// The published @toolu/opencode reaches nothing outside its own tarball and its
// declared dependencies (#361). Fixture packages are real directories packed by
// npm; the last case packs the real package through its own prepack.
import { afterEach, expect, test } from "bun:test";
import {
  chmodSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  rmSync,
  statSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join, resolve } from "node:path";
import { z } from "zod";
import { packedFiles, stageOpencode } from "../npm-pack.ts";
import { closureProblems } from "../pack-closure.ts";

const ROOT = resolve(import.meta.dir, "../../..");
const CORE_EXPORTS = ["./dispatch", "./startup"];
const ENTRY = { ".": "./src/plugin/toolu.ts" };
const temps: string[] = [];

afterEach(() => {
  for (const dir of temps.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function temp(prefix: string): string {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  temps.push(dir);
  return dir;
}

type Fixture = {
  files: Record<string, string>;
  executable?: readonly string[];
  exports?: Record<string, string>;
};

const BASE: Record<string, string> = {
  "src/plugin/toolu.ts":
    'import { z } from "zod";\nimport { run } from "./run.ts";\nexport { z, run };\n',
  "src/plugin/run.ts":
    'import type { Plugin } from "@opencode-ai/plugin";\nexport const run = 1;\n',
  "generated/skills/a/SKILL.md": "Run `$TOOLU_PLUGIN_ROOT_PR_BABYSIT/hooks/dist/tick.js`.\n",
  "plugins/pr-babysit/hooks/dist/tick.js":
    'import { readFileSync } from "node:fs";\nreadFileSync;\n',
};

/**
 * A package on disk: `src`, `generated` and `plugins` ship, and zod and
 * @toolu/core declared dependencies. `executable` paths are 0755 in the source
 * catalog (`<dir>/source/<path>`) but 0644 in the package.
 */
function fixture({ files, executable = [], exports = ENTRY }: Fixture): {
  problems: readonly string[];
} {
  const dir = temp("pack-closure-");
  const pkg = join(dir, "pkg");
  const manifest = {
    name: "@toolu/opencode",
    version: "0.0.0",
    files: ["src", "generated", "plugins"],
    exports,
    dependencies: { zod: "4.1.5", "@toolu/core": "^0.0.0" },
  };
  for (const [path, text] of Object.entries({ ...BASE, ...files })) {
    mkdirSync(dirname(join(pkg, path)), { recursive: true });
    writeFileSync(join(pkg, path), text);
  }
  writeFileSync(join(pkg, "package.json"), `${JSON.stringify(manifest)}\n`);
  for (const path of executable) {
    const source = join(dir, "source", path.slice("plugins/".length));
    mkdirSync(dirname(source), { recursive: true });
    writeFileSync(source, "#!/usr/bin/env bun\n");
    chmodSync(source, 0o755);
  }
  const problems = closureProblems({
    packageDir: pkg,
    files: packedFiles(pkg),
    sourcePlugins: join(dir, "source"),
    coreExports: CORE_EXPORTS,
  });
  return { problems };
}

const P = "@toolu/opencode:";

test.concurrent("a closed package reports nothing, type-only SDK imports included", () => {
  expect(fixture({ files: {} }).problems).toEqual([]);
});

test.concurrent("a referenced helper missing from the tarball is reported", () => {
  const files = {
    "generated/skills/a/SKILL.md": 'bun "$TOOLU_PLUGIN_ROOT/hooks/dist/ledger.js" status\n',
  };
  expect(fixture({ files }).problems).toEqual([
    `${P} generated/skills/a/SKILL.md references $TOOLU_PLUGIN_ROOT/hooks/dist/ledger.js, which the tarball does not contain`,
  ]);
});

test.concurrent("placeholders, bare variables and present directories are not references", () => {
  const text = [
    "`$TOOLU_PLUGIN_ROOT_PR_BABYSIT/hooks/dist/<helper>.js`",
    "`${TOOLU_PLUGIN_ROOT_<PLUGIN>}` and `$TOOLU_PLUGIN_ROOT_PR_BABYSIT` alone",
    "`${TOOLU_OPENCODE_ROOT}/generated/…` and `${TOOLU_OPENCODE_ROOT}/generated/`",
    "`${TOOLU_PLUGIN_ROOT_EPIC_ORCHESTRATOR:?not enabled}`",
    "Read ${TOOLU_PLUGIN_ROOT_PR_BABYSIT}/hooks/dist/tick.js.",
  ].join("\n");
  expect(fixture({ files: { "generated/skills/a/SKILL.md": text } }).problems).toEqual([]);
});

test.concurrent("a broken Markdown link is reported; anchors, fragments and URLs are not", () => {
  const text = "[a](../b/SKILL.md#x) [b](#top) [c](https://x.dev/y) [d](missing.md)\n";
  const files = { "generated/skills/a/SKILL.md": text, "generated/skills/b/SKILL.md": "b\n" };
  expect(fixture({ files }).problems).toEqual([
    `${P} generated/skills/a/SKILL.md links missing.md, which the tarball does not contain`,
  ]);
});

test.concurrent("a relative import outside the tarball is reported", () => {
  const files = { "src/plugin/run.ts": 'export { x } from "../../../packages/core/x.ts";\n' };
  expect(fixture({ files }).problems).toEqual([
    `${P} src/plugin/run.ts imports ../../../packages/core/x.ts, which the tarball does not contain`,
  ]);
});

test.concurrent("an executable bundle's shebang is skipped and its imports are still checked", () => {
  const files = {
    "plugins/pr-babysit/hooks/dist/tick.js":
      '#!/usr/bin/env bun\nimport pad from "left-pad";\npad;\n',
  };
  expect(fixture({ files }).problems).toEqual([
    `${P} plugins/pr-babysit/hooks/dist/tick.js imports left-pad, which is not a declared dependency`,
  ]);
});

test.concurrent("an undeclared bare import is reported", () => {
  const files = {
    "src/plugin/run.ts": 'import { run } from "@toolu/conformance/harness/spawn";\nrun;\n',
  };
  expect(fixture({ files }).problems).toEqual([
    `${P} src/plugin/run.ts imports @toolu/conformance/harness/spawn, which is not a declared dependency`,
  ]);
});

test.concurrent("a @toolu/core subpath that core does not export is reported", () => {
  const files = {
    "src/plugin/run.ts":
      'import { a } from "@toolu/core/dispatch";\nimport { b } from "@toolu/core/runner";\nexport { a, b };\n',
  };
  expect(fixture({ files }).problems).toEqual([
    `${P} src/plugin/run.ts imports @toolu/core/runner, which @toolu/core does not export`,
  ]);
});

test.concurrent("an export target missing from the tarball is reported", () => {
  const exports = { ".": "./src/plugin/toolu.ts", "./gone": "./src/gone.ts" };
  expect(fixture({ files: {}, exports }).problems).toEqual([
    `${P} export ./gone → ./src/gone.ts is not in the tarball`,
  ]);
});

test.concurrent("a bundle that is executable in the catalog but not in the tarball is reported", () => {
  const executable = ["plugins/pr-babysit/hooks/dist/tick.js"];
  expect(fixture({ files: {}, executable }).problems).toEqual([
    `${P} plugins/pr-babysit/hooks/dist/tick.js lost its executable bit (mode 644)`,
  ]);
});

function isExecutable(mode: number): boolean {
  return (mode & 0o111) !== 0;
}

const CoreManifest = z.looseObject({ exports: z.record(z.string(), z.unknown()) });

test("the real @toolu/opencode, packed through its own prepack, is closed", () => {
  const stage = stageOpencode(temp("pack-closure-real-"));
  const core = CoreManifest.parse(
    JSON.parse(readFileSync(join(ROOT, "packages/toolu-core/package.json"), "utf8")),
  );
  const files = packedFiles(stage);
  const problems = closureProblems({
    packageDir: stage,
    files,
    sourcePlugins: join(ROOT, "plugins"),
    coreExports: Object.keys(core.exports),
  });
  expect(problems).toEqual([]);
  // The mode rule has real input: the catalog's executable bundles ship 0755.
  const sourceExecutable = files
    .filter((file) => file.path.startsWith("plugins/"))
    .filter((file) => isExecutable(statSync(join(ROOT, file.path)).mode))
    .map((file) => file.path);
  expect(sourceExecutable).toContain("plugins/toolu/hooks/dist/verdict.js");
  // jev.js was the extra executable bundle; the native shim is not in this tarball.
  expect(sourceExecutable.length).toBeGreaterThanOrEqual(13);
  const packedExecutable = files.filter((file) => isExecutable(file.mode)).map((file) => file.path);
  expect(packedExecutable).toEqual(sourceExecutable);
}, 180_000);
