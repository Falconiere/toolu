// npm pack, the way the release publishes (#361): real fixture packages with
// their own prepack scripts, and the @toolu/opencode stage on disk.
import { afterEach, expect, test } from "bun:test";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readlinkSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";
import { packedFiles, packInto, stageOpencode } from "../npm-pack.ts";

const ROOT = resolve(import.meta.dir, "../../..");
const temps: string[] = [];

afterEach(() => {
  for (const dir of temps.splice(0)) rmSync(dir, { recursive: true, force: true });
});

function temp(prefix: string): string {
  const dir = mkdtempSync(join(tmpdir(), prefix));
  temps.push(dir);
  return dir;
}

/** A package whose prepack runs `prepack`, shipping `lib/a.js`. */
function fixture(prepack: string): string {
  const dir = temp("npm-pack-");
  mkdirSync(join(dir, "lib"));
  writeFileSync(join(dir, "lib/a.js"), "export {};\n");
  const manifest = {
    name: "npm-pack-fixture",
    version: "1.2.3",
    files: ["lib"],
    scripts: { prepack },
  };
  writeFileSync(join(dir, "package.json"), `${JSON.stringify(manifest)}\n`);
  return dir;
}

test.concurrent("a prepack that prints to stdout does not corrupt the file list", () => {
  const dir = fixture("echo staged 3 plugins");
  expect(
    packedFiles(dir)
      .map((file) => file.path)
      .toSorted(),
  ).toEqual(["lib/a.js", "package.json"]);
});

test.concurrent("packInto writes the tarball and returns its path", () => {
  const dir = fixture("echo noise");
  const out = temp("npm-pack-out-");
  const tarball = packInto(dir, out);
  expect(tarball).toBe(join(out, "npm-pack-fixture-1.2.3.tgz"));
  expect(existsSync(tarball)).toBe(true);
});

test.concurrent("a failing prepack fails the pack with its exit code", () => {
  const dir = fixture("exit 7");
  expect(() => packedFiles(dir)).toThrow(`npm pack in ${dir} failed (exit 7)`);
});

test.concurrent("the opencode stage copies the package without plugins/ and links the catalog", () => {
  const work = temp("npm-pack-stage-");
  const stage = stageOpencode(work);
  expect(stage).toBe(join(work, "repo/tools/toolu-opencode"));
  expect(existsSync(join(stage, "package.json"))).toBe(true);
  expect(existsSync(join(stage, "scripts/bundle-plugins.ts"))).toBe(true);
  expect(existsSync(join(stage, "plugins"))).toBe(false);
  expect(existsSync(join(stage, "node_modules"))).toBe(false);
  expect(lstatSync(join(work, "repo/plugins")).isSymbolicLink()).toBe(true);
  expect(readlinkSync(join(work, "repo/plugins"))).toBe(join(ROOT, "plugins"));
});
