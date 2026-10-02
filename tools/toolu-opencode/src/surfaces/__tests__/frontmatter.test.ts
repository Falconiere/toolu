import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { frontmatterName, parseCanonical } from "../frontmatter.ts";

const GENERATED = join(import.meta.dir, "../../../generated");

test("the committed agent parses in canonical form with its body", () => {
  const parsed = parseCanonical(
    readFileSync(join(GENERATED, "agents/toolu-quick-task.md"), "utf8"),
  );
  if (!parsed.ok) throw new Error(parsed.reason);
  expect(parsed.data.mode).toBe("subagent");
  expect(parsed.data.permission).toEqual({
    "*": "deny",
    bash: "allow",
    glob: "allow",
    grep: "allow",
    read: "allow",
  });
  expect(parsed.body.startsWith("\n## Instructions")).toBe(true);
});

test("non-canonical frontmatter is rejected with the reason", () => {
  const cases: Array<[string, string]> = [
    ["no fence here\n", "no frontmatter fence"],
    ["---\nmode: subagent\n---\nbody\n", 'value of "mode" is not JSON'],
    ['---\nmode: "subagent"\ndescription: "d"\n---\nbody\n', 'keys not sorted at "description"'],
    ['---\ndescription: "a"\ndescription: "b"\n---\nbody\n', 'keys not sorted at "description"'],
    ['---\n  description: "d"\n---\nbody\n', 'not a "key: <JSON>" line'],
  ];
  for (const [text, reason] of cases) {
    const parsed = parseCanonical(text);
    expect({ text, ok: parsed.ok }).toEqual({ text, ok: false });
    if (!parsed.ok) expect(parsed.reason).toContain(reason);
  }
});

test("frontmatterName reads skill names the way the pinned host does", () => {
  const named: Array<[string, string | undefined]> = [
    ["---\nname: toolu-debug\ndescription: d\n---\nbody\n", "toolu-debug"],
    ["---\nname: 'toolu-debug'\n---\n", "toolu-debug"],
    ['---\nname: "toolu-debug"\n---\n', "toolu-debug"],
    ["---\nname: toolu-debug # mine\n---\n", "toolu-debug"],
    ["﻿---\r\nname: toolu-debug\r\ndescription: d\r\n---\r\nbody\r\n", "toolu-debug"],
    ["---\nname: toolu-debug\ndescription: Use: when broken\n---\n", "toolu-debug"],
    ["---\nname: toolu-debug\nname: other\n---\n", undefined],
    ["---\nname: 123\n---\n", undefined],
    ["---\ndescription: no name\n---\n", undefined],
    ["---\nname: [unclosed\n---\n", undefined],
    ["no frontmatter at all\n", undefined],
    ["---\nname: toolu-debug\n", undefined],
  ];
  for (const [text, name] of named)
    expect({ text, name: frontmatterName(text) }).toEqual({ text, name });
});
