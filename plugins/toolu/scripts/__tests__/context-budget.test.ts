/** context-budget.ts on real fixture files, so the guard is verified
 * independent of the live harness docs, plus the CLI on this repo. */

import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { createSandbox } from "@toolu/conformance/harness/sandbox";
import { run } from "@toolu/conformance/harness/spawn";
import {
  DOC_BUDGETS,
  checkDoc,
  checkSkill,
  countWords,
  extractDescription,
} from "../context-budget.ts";

const SCRIPT = join(import.meta.dir, "..", "context-budget.ts");
const reds = (lines: { red: boolean }[]) => lines.filter((line) => line.red).length;

test.concurrent("extractDescription reads a single-line description", () => {
  using sb = createSandbox();
  const file = sb.write("single.md", "---\nname: x\ndescription: hello there world\n---\n");
  expect(extractDescription(file)).toBe("hello there world");
});

test.concurrent("extractDescription joins a folded (>) block scalar", () => {
  using sb = createSandbox();
  const file = sb.write(
    "folded.md",
    "---\nname: x\ndescription: >\n  alpha beta\n  gamma delta\n---\n",
  );
  expect(extractDescription(file)).toBe("alpha beta gamma delta");
});

test.concurrent("extractDescription fails on a missing file", () => {
  using sb = createSandbox();
  expect(extractDescription(sb.path("nope.md"))).toBeNull();
});

test.concurrent("countWords fails on a missing file", () => {
  using sb = createSandbox();
  expect(countWords(sb.path("nope.md"))).toBeNull();
});

test.concurrent("checkDoc flags an over-budget file as RED", () => {
  using sb = createSandbox();
  sb.write("d/x.md", "one two three four five\n");
  expect(checkDoc(sb.project, "x", "d/x.md", 3)).toEqual([
    { red: true, text: "x 5w EXCEEDS 3 (d/x.md)" },
  ]);
});

test.concurrent("checkDoc passes an under-budget file", () => {
  using sb = createSandbox();
  sb.write("d/x.md", "one two\n");
  expect(checkDoc(sb.project, "x", "d/x.md", 3)).toEqual([{ red: false, text: "x 2w (<= 3)" }]);
});

test.concurrent("checkDoc fails closed when the target file is missing", () => {
  using sb = createSandbox();
  expect(checkDoc(sb.project, "gone", "d/missing.md", 9999)).toEqual([
    { red: true, text: "gone: MISSING d/missing.md" },
  ]);
});

test.concurrent("checkSkill flags a missing trigger phrase even when under budget", () => {
  using sb = createSandbox();
  sb.write("s/SKILL.md", "---\nname: s\ndescription: short blurb without the marker\n---\n");
  const lines = checkSkill(sb.project, "s", "s/SKILL.md", 50, ["must keep this"]);
  expect(reds(lines)).toBe(1);
  expect(lines.at(-1)?.text).toBe('s: missing trigger phrase "must keep this" (s/SKILL.md)');
});

test.concurrent("checkSkill passes when under budget and phrase present", () => {
  using sb = createSandbox();
  sb.write(
    "s/SKILL.md",
    "---\nname: s\ndescription: blurb that must keep this marker intact\n---\n",
  );
  expect(reds(checkSkill(sb.project, "s", "s/SKILL.md", 50, ["must keep this"]))).toBe(0);
});

test.concurrent("checkSkill reports an absent description as unparseable", () => {
  using sb = createSandbox();
  sb.write("s/SKILL.md", "---\nname: s\n---\n");
  expect(checkSkill(sb.project, "s", "s/SKILL.md", 50, [])).toEqual([
    { red: true, text: "s: empty/unparseable description (s/SKILL.md)" },
  ]);
});

test.concurrent("the CLI passes on this repository's real docs and skills", async () => {
  const res = await run(["bun", SCRIPT], { cwd: import.meta.dir });
  expect(res.stderr).toBe("");
  expect(res.exitCode).toBe(0);
  expect(res.stdout).toContain("ok   session-start ");
  expect(res.stdout).toContain("ok   jev desc ");
});

test.concurrent("the CLI fails closed on a root with none of the targets", async () => {
  using sb = createSandbox();
  const res = await run(["bun", SCRIPT, "docs"], { env: { CONTEXT_BUDGET_ROOT: sb.project } });
  expect(res.exitCode).toBe(1);
  expect(res.stderr).toContain(
    "RED  session-start: MISSING plugins/toolu/hooks/docs/session-start.md\n",
  );
  expect(res.stdout).toBe("");
});

test.concurrent("the CLI rejects an unknown mode with exit 2", async () => {
  const res = await run(["bun", SCRIPT, "bogus"]);
  expect(res.exitCode).toBe(2);
  expect(res.stderr).toBe("usage: context-budget.ts [docs|skills]\n");
});

test.concurrent("no-break and other Unicode spaces separate words, as wc -w counts them", () => {
  using sb = createSandbox();
  const file = sb.write("u.md", "a b　c d\n");
  expect(countWords(file)).toBe(4);
});

test.concurrent("every doc the SessionStart hook renders has a word ceiling", () => {
  const source = readFileSync(
    join(import.meta.dir, "..", "..", "hooks", "src", "lifecycle", "session-docs.ts"),
    "utf8",
  );
  const rendered = [...source.matchAll(/"([a-z-]+)\.md"/g)].map((m) => m[1]);
  expect(rendered.length).toBeGreaterThan(0);
  expect(new Set(rendered)).toEqual(new Set(DOC_BUDGETS.map(([name]) => name)));
});
