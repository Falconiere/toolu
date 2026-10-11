/** Core plugin agent surfaces use native commands after the hook switch. */
import { expect, test } from "bun:test";
import { readdirSync, readFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "../../..");
const PLUGIN = join(ROOT, "plugins/toolu");
const RETIRED = /\bbun\b|(?:scripts|hooks\/dist)\//i;

function markdownFiles(dir: string): string[] {
  return readdirSync(dir, { withFileTypes: true }).flatMap((entry) => {
    const path = join(dir, entry.name);
    if (entry.isDirectory()) return markdownFiles(path);
    return entry.isFile() && entry.name.endsWith(".md") ? [path] : [];
  });
}

test("toolu skills, commands and agents name no Bun or retired executable paths", () => {
  const files = ["skills", "commands", "agents"].flatMap((dir) => markdownFiles(join(PLUGIN, dir)));
  expect(files.length).toBeGreaterThan(0);
  const stale = files.flatMap((file) =>
    readFileSync(file, "utf8")
      .split("\n")
      .flatMap((line, index) =>
        RETIRED.test(line) ? [`${relative(ROOT, file)}:${index + 1}: ${line}`] : [],
      ),
  );
  expect(stale).toEqual([]);
});
