import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import * as entry from "../toolu.ts";
import { parseOptions, projectRootOf } from "../context.ts";

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
