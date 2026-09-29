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

test.skipIf(!HAS_AST_GREP)("every matched source line becomes a rule-tagged excerpt", () => {
  using sb = createSandbox({ git: true });
  expect(scan(sb, RULES)).toEqual({
    kind: "ok",
    empty: false,
    hits: [
      { ruleId: "throw-string", excerpt: 'src/a.ts:2:  throw "boom";' },
      { ruleId: "empty-catch", excerpt: "src/a.ts:5:  try {" },
      { ruleId: "empty-catch", excerpt: "src/a.ts:6:    f();" },
      { ruleId: "empty-catch", excerpt: "src/a.ts:7:  } catch (e) { }" },
    ],
  });
});

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

test("ast-grep missing from PATH is reported as missing", () => {
  using sb = createSandbox({ git: true });
  expect(scan(sb, RULES, { PATH: join(sb.root, "empty-bin") })).toEqual({ kind: "missing" });
});
