/**
 * The single ast-grep scan (#265) against the real `ast-grep` binary: excerpt
 * lines for single- and multi-line matches, and each failure stage. The only
 * stand-in is a script that exits 0 with non-JSON output, the one ast-grep
 * failure a real run cannot be made to produce on demand.
 */
import { expect, test } from "bun:test";
import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox, type Sandbox } from "@toolu/conformance/harness/sandbox";
import { astGrepScan } from "../quality-ast-grep.ts";
import { postContext } from "./quality-harness.ts";

const HAS_AST_GREP = Bun.which("ast-grep") !== null;

const RULES = `id: throw-string
language: ts
severity: warning
message: throw of a string literal
rule:
  pattern: 'throw "$S"'
---
id: empty-catch
language: ts
severity: warning
message: empty catch
rule:
  pattern: 'try { $$$ } catch ($_) { }'`;

const SOURCE = `export function f() {
  throw "boom";
}
export function g() {
  try {
    f();
  } catch (e) { }
}
`;

function scan(sb: Sandbox, rules: string, env: Record<string, string> = {}) {
  sb.write("src/a.ts", SOURCE);
  const file = { path: "src/a.ts", absolute: sb.path("src/a.ts"), removed: false };
  return astGrepScan(file, rules, postContext(sb, { env }));
}

/** A scan's hits for one rule, in the order ast-grep reported them. */
function ofRule(result: ReturnType<typeof scan>, rule: string) {
  return result.kind === "ok" ? result.hits.filter((hit) => hit.ruleId === rule) : [];
}

test.skipIf(!HAS_AST_GREP)("every matched source line becomes a rule-tagged excerpt", () => {
  using sb = createSandbox({ git: true });
  const result = scan(sb, RULES);
  expect(result).toMatchObject({ kind: "ok", empty: false });
  // ast-grep interleaves rules in an order that varies run to run; within a
  // rule, matches come in source order.
  expect(ofRule(result, "throw-string")).toEqual([
    {
      ruleId: "throw-string",
      line: 2,
      excerpt: 'src/a.ts:2:  throw "boom";',
      text: '  throw "boom";',
      first: true,
    },
  ]);
  expect(ofRule(result, "empty-catch")).toEqual([
    { ruleId: "empty-catch", line: 5, excerpt: "src/a.ts:5:  try {", text: "  try {", first: true },
    { ruleId: "empty-catch", line: 6, excerpt: "src/a.ts:6:    f();", text: "    f();", first: false },
    {
      ruleId: "empty-catch",
      line: 7,
      excerpt: "src/a.ts:7:  } catch (e) { }",
      text: "  } catch (e) { }",
      first: false,
    },
  ]);
  expect(result.kind === "ok" ? result.hits.length : 0).toBe(4);
});

test.skipIf(!HAS_AST_GREP)(
  "adjacent multi-line matches each start with a first line; tabs stay in the text",
  () => {
    using sb = createSandbox({ git: true });
    sb.write("m.py", "def a(mocker):\n\treturn 1\ndef b(mocker):\n\treturn 2\n");
    const file = { path: "m.py", absolute: sb.path("m.py"), removed: false };
    const rules =
      "id: mocker-param\nlanguage: python\nseverity: warning\nmessage: m\nrule:\n  kind: function_definition\n  has:\n    field: parameters\n    has: {kind: identifier, regex: '^mocker$'}";
    const result = astGrepScan(file, rules, postContext(sb, {}));
    const hits = result.kind === "ok" ? result.hits : [];
    expect(hits.map(({ line, text, first }) => ({ line, text, first }))).toEqual([
      { line: 1, text: "def a(mocker):", first: true },
      { line: 2, text: "\treturn 1", first: false },
      { line: 3, text: "def b(mocker):", first: true },
      { line: 4, text: "\treturn 2", first: false },
    ]);
  },
);

test.skipIf(!HAS_AST_GREP)(
  "an invalid rule is an ast-grep failure with its exit code and stderr",
  () => {
    using sb = createSandbox({ git: true });
    const result = scan(sb, "id: broken\nlanguage: nope\nrule:\n  pattern: x");
    expect(result).toMatchObject({ kind: "failed", stage: "ast-grep" });
    if (result.kind !== "failed") return;
    expect(result.exitCode).not.toBe(0);
    expect(result.stderrFirst.length).toBeGreaterThan(0);
    expect(result.stderrFirst.length).toBeLessThanOrEqual(200);
  },
);

function stubBin(sb: Sandbox, body: string): string {
  const bin = join(sb.root, "bin");
  mkdirSync(bin, { recursive: true });
  writeFileSync(join(bin, "ast-grep"), `#!/bin/sh\n${body}\n`);
  chmodSync(join(bin, "ast-grep"), 0o755);
  return `${bin}:${process.env.PATH ?? ""}`;
}

test("non-JSON output is a parse failure; blank output is an empty scan", () => {
  using sb = createSandbox({ git: true });
  expect(scan(sb, RULES, { PATH: stubBin(sb, "echo 'not json'") })).toEqual({
    kind: "failed",
    stage: "parse",
    exitCode: 0,
    stderrFirst: "",
  });
  expect(scan(sb, RULES, { PATH: stubBin(sb, "printf '  \\n'") })).toEqual({
    kind: "ok",
    hits: [],
    empty: true,
  });
});

test("ast-grep killed by a signal reports 128 + the signal number, as bash's $? does", () => {
  using sb = createSandbox({ git: true });
  expect(scan(sb, RULES, { PATH: stubBin(sb, "kill -TERM $$") })).toEqual({
    kind: "failed",
    stage: "ast-grep",
    exitCode: 143,
    stderrFirst: "",
  });
});

test("ast-grep missing from PATH is reported as missing", () => {
  using sb = createSandbox({ git: true });
  expect(scan(sb, RULES, { PATH: join(sb.root, "empty-bin") })).toEqual({ kind: "missing" });
});
