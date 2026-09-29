/**
 * #260: `repoRelative` strips the repo root like `to_relative_path`, and
 * `expandPattern` lists what bash's pathname expansion would write, checked
 * against real bash `echo` expansion in the same real directory tree.
 */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { expandPattern, repoRelative } from "../gate-paths.ts";

test("repoRelative strips the root prefix and nothing else", () => {
  expect(repoRelative("/r/p/.env", "/r/p")).toBe(".env");
  expect(repoRelative("/r/p/hooks/lib/x.sh", "/r/p")).toBe("hooks/lib/x.sh");
  expect(repoRelative("/r/px/.env", "/r/p")).toBe("/r/px/.env");
  expect(repoRelative("/r/p", "/r/p")).toBe("/r/p");
  expect(repoRelative(".env", "/r/p")).toBe(".env");
  expect(repoRelative("/r/p/.env", undefined)).toBe("/r/p/.env");
});

/** What bash expands `pattern` to in `cwd` (nullglob off: no match keeps the word). */
function bashExpansion(pattern: string, cwd: string): string[] {
  const res = spawnSync("bash", ["-c", `for w in ${pattern}; do printf '%s\\n' "$w"; done`], {
    cwd,
    encoding: "utf8",
  });
  return res.stdout.trimEnd().split("\n");
}

test.concurrent("expandPattern includes every file bash would expand to, plus the literal", () => {
  using sb = createSandbox({
    files: {
      ".env": "x",
      ".env.local": "x",
      "env.txt": "x",
      "apps/api/.env": "x",
      "apps/web/.env": "x",
      "apps/web/x.ts": "x",
    },
  });
  const cases = [".en[v]", ".env*", "apps/*/.env", "apps/*/.en?", "*.txt", "*", "nothing*", "a\\*"];
  for (const pattern of cases) {
    const got = expandPattern(pattern, sb.project);
    for (const path of bashExpansion(pattern, sb.project)) expect(got).toContain(path);
    expect(got).toContain(pattern);
  }
  expect(expandPattern(".en[v]", sb.project)).toEqual([".env", ".en[v]"]);
  expect(expandPattern("*", sb.project)).not.toContain(".env");
  expect(expandPattern(`${sb.project}/.en[v]`, "/")).toEqual([
    `${sb.project}/.env`,
    `${sb.project}/.en[v]`,
  ]);
});

test.concurrent("an unreadable directory expands to the literal only", () => {
  expect(expandPattern("missing/*/.env", "/nonexistent-toolu-dir")).toEqual(["missing/*/.env"]);
});
