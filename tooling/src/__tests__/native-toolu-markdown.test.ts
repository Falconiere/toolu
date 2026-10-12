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

function staleReferences(file: string, markdown: string): string[] {
  let fence: string | undefined;
  return markdown.split("\n").flatMap((line, index) => {
    const marker = /^\s*(`{3,}|~{3,})([^\s`]*)/u.exec(line);
    if (marker !== null) {
      fence = fence === undefined ? (marker[2] ?? "").toLowerCase() : undefined;
      return [];
    }
    if (fence === "text" || fence === "ignore") return [];
    if (fence !== undefined && line.trimStart().startsWith("#")) return [];
    return RETIRED.test(line) ? [`${file}:${index + 1}: ${line}`] : [];
  });
}

test("retired command scan ignores fenced comments and text examples", () => {
  const markdown = "```sh\n# Avoid using bun here\nbun run test\n```\n```text\nscripts/old.ts\n```";
  expect(staleReferences("sample.md", markdown)).toEqual(["sample.md:3: bun run test"]);
});

test("toolu skills, commands and agents name no Bun or retired executable paths", () => {
  const files = ["skills", "commands", "agents"].flatMap((dir) => markdownFiles(join(PLUGIN, dir)));
  expect(files.length).toBeGreaterThan(0);
  const stale = files.flatMap((file) =>
    staleReferences(relative(ROOT, file), readFileSync(file, "utf8")),
  );
  expect(stale).toEqual([]);
});
