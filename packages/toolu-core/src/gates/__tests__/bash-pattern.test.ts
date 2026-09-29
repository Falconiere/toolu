/**
 * AC-8 (#260): `compileBashPattern` answers what `[[ text == $pattern ]]`
 * answers in real bash with `extglob` on, over every shipped protected-files
 * and code-edit-rules pattern and the syntax corners, against real paths.
 */
import { expect, test } from "bun:test";
import { spawnSync } from "node:child_process";
import { readFileSync } from "node:fs";
import { join, resolve } from "node:path";
import { readList } from "../../config/settings.ts";
import { bashPatternMatch, compileBashPattern } from "../bash-pattern.ts";

const SETTINGS = resolve(import.meta.dir, "../../../../../plugins/toolu/settings");

function shippedRulePatterns(): string[] {
  const raw: unknown = JSON.parse(readFileSync(join(SETTINGS, "code-edit-rules.json"), "utf8"));
  const rules = typeof raw === "object" && raw !== null && "rules" in raw ? raw.rules : [];
  if (!Array.isArray(rules)) return [];
  return rules.flatMap((rule: unknown) => {
    if (typeof rule !== "object" || rule === null) return [];
    const match = "match" in rule && typeof rule.match === "string" ? [rule.match] : [];
    const when =
      "when_path_matches" in rule && Array.isArray(rule.when_path_matches)
        ? rule.when_path_matches.filter((p: unknown): p is string => typeof p === "string")
        : [];
    return [...match, ...when];
  });
}

const SHIPPED = readList(join(SETTINGS, "protected-files.txt"));
const PATTERNS = [
  ...SHIPPED,
  ...SHIPPED.filter((p) => !p.startsWith("**/")).map((p) => `**/${p}`),
  ...shippedRulePatterns(),
  "*",
  "?",
  "a?c",
  "*.env",
  "[abc]x",
  "[!abc]x",
  "[^abc]x",
  "[]a]",
  "[!]a]",
  "[a-c]*",
  "[[:alpha:]]*",
  "[[:digit:]]",
  "[[:upper:]][[:lower:]]",
  "[[:space:]]x",
  "[[:punct:]]",
  "\\*",
  "a\\?b",
  "[\\]]",
  "[a",
  "a[",
  "a]",
  "@(foo|bar).cfg",
  "!(foo).cfg",
  "!(*.txt)",
  "*(ab)",
  "+(ab)c",
  "?(x)y",
  "@(a|b|)z",
  "!(a)*",
  "*.@(ts|tsx)",
  "src/**/*.rs",
  "!(src)/*",
  "@(a*(b))",
  "x!(y|z)",
  "?(",
  "@(a",
  "*(a|b",
  "a-b",
  "[z-a]",
  "[a-]",
  "**",
];
const TEXTS = [
  "",
  "a",
  "abc",
  "x",
  "ax",
  "dx",
  "]",
  "*",
  "a?b",
  "axb",
  "abb",
  ".env",
  ".env.example",
  "apps/api/.env",
  "hooks/lib/detect.sh",
  "sub/hooks/lib/x.sh",
  "hooks/post-tools/modules/q.sh",
  "config/secrets/key.txt",
  ".git/config",
  "a/.git/HEAD",
  "foo.cfg",
  "bar.cfg",
  "baz.cfg",
  "notes.txt",
  "notes.md",
  "ababc",
  "abc",
  "c",
  "y",
  "xy",
  "z",
  "az",
  "bz",
  "ab",
  "abbb",
  "src/foo/bar.rs",
  "src/a.rs",
  "a.rs",
  "src/features/x.ts",
  "x.tsx",
  "lib/x.ts",
  "x.oxlintrc.json",
  ".oxlintrc.json",
  "skills/ast-grep/scripts/run.sh",
  "Ab",
  "1",
  " x",
  "!",
  "[a",
  "a[",
  "a]",
  "?(",
  "@(a",
  "*(a|b",
  "a-b",
  "-",
  "src/x",
  "srcx/y",
  "\\",
  "é",
];

/** Real bash, one process: NUL-separated pattern/text pairs in, one 0/1 per line out. */
function bashAnswers(pairs: readonly [string, string][]): boolean[] {
  const input = pairs.flatMap(([p, t]) => [p, t]).join("\0") + "\0";
  const script =
    'while IFS= read -r -d "" p && IFS= read -r -d "" t; do if [[ $t == $p ]]; then echo 1; else echo 0; fi; done';
  const res = spawnSync("bash", ["-O", "extglob", "-c", script], {
    input,
    encoding: "utf8",
    env: { ...process.env, LC_ALL: "en_US.UTF-8" },
    maxBuffer: 64 * 1024 * 1024,
  });
  if (res.status !== 0) throw new Error(`bash exited ${String(res.status)}: ${res.stderr}`);
  return res.stdout
    .trimEnd()
    .split("\n")
    .map((line) => line === "1");
}

test("every pattern x path answer equals real bash", () => {
  const pairs = PATTERNS.flatMap((p) => TEXTS.map((t): [string, string] => [p, t]));
  const want = bashAnswers(pairs);
  expect(want.length).toBe(pairs.length);
  const wrong = pairs.flatMap(([p, t], i) => {
    const got = compileBashPattern(p)(t);
    return got === want[i]
      ? []
      : [`[[ ${JSON.stringify(t)} == ${p} ]]: bash ${String(want[i])}, ts ${String(got)}`];
  });
  expect(wrong).toEqual([]);
  expect(want.filter(Boolean).length).toBeGreaterThan(100);
});

test("the shipped protected patterns match what they guard", () => {
  expect(bashPatternMatch("hooks/**/*.sh", "hooks/post-tools/modules/q.sh")).toBe(true);
  expect(bashPatternMatch(".env.*", ".env.example")).toBe(true);
  expect(bashPatternMatch("**/secrets/**", "config/secrets/key.txt")).toBe(true);
  expect(bashPatternMatch(".env", "apps/.env")).toBe(false);
});
