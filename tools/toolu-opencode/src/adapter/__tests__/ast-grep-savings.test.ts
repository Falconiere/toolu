/**
 * #347 AC-2/AC-3: byte-savings, as published by ast-grep's real `register.js`,
 * records each completed OpenCode read/grep/glob/ast-grep call once with the
 * bytes the host returned, and the ast-grep result carries the session report.
 */
import { expect, test } from "bun:test";
import { statSync } from "node:fs";
import { join } from "node:path";
import {
  SEARCH,
  astGrepProject,
  bytes,
  count,
  searchOutput,
  type HostCall,
} from "./ast-grep-fixture.ts";

const REPORT = "ast-grep byte savings this session:";
const POST = "[toolu post-check after execution]";

const READ_TEXT =
  "<file>\n00001| export function greet(name: string) {\n00002|   return name;\n</file>";
const GREP_TEXT = "Found 1 matches\nsrc/app.ts:\n  Line 1: export function greet";

function read(root: string): HostCall {
  return {
    tool: "read",
    args: { filePath: join(root, "src/app.ts") },
    output: READ_TEXT,
    metadata: { truncated: false },
  };
}

const grep: HostCall = {
  tool: "grep",
  args: { pattern: "export function greet" },
  output: GREP_TEXT,
  metadata: { matches: 1, truncated: true },
};

const glob = (n: number): HostCall => ({
  tool: "glob",
  args: { pattern: `src/**/*.ts${"?".repeat(n)}` },
  output: "src/app.ts",
  metadata: { count: 1, truncated: false },
});

function astGrep(output: string, metadata: Record<string, unknown> = { exit: 0 }): HostCall {
  return { tool: "bash", args: { command: SEARCH, description: "search" }, output, metadata };
}

test.concurrent("each measured call records the host's own bytes once; ast-grep reports the session", async () => {
  const p = astGrepProject();
  const found = searchOutput(p.root);
  const fullSize = statSync(join(p.root, "src/app.ts")).size;
  for (const quiet of [await p.call(read(p.root)), await p.call(grep), await p.call(glob(0))]) {
    expect(quiet).not.toContain(REPORT);
  }
  // The nudged grep's advisory is appended after measurement and never counted.
  expect(await p.call(astGrep(found), "search")).toContain(POST);
  expect(p.ledger()).toEqual([
    { kind: "read", returned: bytes(READ_TEXT), full: fullSize },
    { kind: "grep", returned: bytes(GREP_TEXT), full: 0 },
    { kind: "glob", returned: bytes("src/app.ts"), full: 0 },
    { kind: "ast-grep", returned: bytes(found), full: 0 },
  ]);
});

test.concurrent("the ast-grep result ends with one report naming every kind", async () => {
  const p = astGrepProject();
  const found = searchOutput(p.root);
  await p.call(read(p.root));
  const result = await p.call(astGrep(found));
  expect(result.startsWith(found)).toBe(true);
  expect(count(result, REPORT)).toBe(1);
  expect(result).toContain(
    `${POST}\n${REPORT}\nast-grep: returned=${bytes(found)} (n=1)\nread: returned=`,
  );
  expect(result).toContain("TOTAL returned: ");
});

test.concurrent("replayed, interrupted, unconfirmed, empty and non-text results add no line", async () => {
  const p = astGrepProject();
  const found = searchOutput(p.root);
  await p.call(astGrep(found), "once");
  expect(await p.replay(astGrep(found), "once")).not.toContain(REPORT);
  await p.call(astGrep(found, { exit: 0, interrupted: true }));
  await p.call(astGrep(found, {}));
  await p.call({ ...grep, output: "" });
  const nonText = await p.call({ ...grep, output: 42 });
  expect(nonText).toContain("Post-tool checks failed to run: output is not text");
  await p.call({
    tool: "bash",
    args: { command: "rg TODO notes.md" },
    output: "notes.md:1:TODO",
    metadata: { exit: 0 },
  });
  expect(p.ledger().map((line) => line.kind)).toEqual(["ast-grep"]);
});

test.concurrent("eight parallel glob calls append eight lines", async () => {
  const p = astGrepProject();
  await Promise.all([0, 1, 2, 3, 4, 5, 6, 7].map((n) => p.call(glob(n))));
  expect(p.ledger()).toHaveLength(8);
  expect(new Set(p.ledger().map((line) => line.kind))).toEqual(new Set(["glob"]));
});
