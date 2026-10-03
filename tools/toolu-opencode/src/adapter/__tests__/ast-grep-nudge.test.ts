/**
 * #347 AC-1: OpenCode's own search calls take search-nudge's rule paths, with
 * the modules ast-grep's real `register.js` publishes. The advice reaches the
 * model once, appended to the tool result.
 */
import { expect, test } from "bun:test";
import { astGrepProject, count, type HostCall } from "./ast-grep-fixture.ts";

const ADVISORY = "[toolu advisory]";
const GREP_STOP = "STOP: Structural code pattern detected. Use ast-grep";
const BASH_STOP = "STOP: grep/rg for structural code search. Use ast-grep";
const GENERIC = "grep/rg in Bash detected. Use ast-grep for structural patterns";
const GENERIC_OPT_OUT =
  "grep/rg in Bash detected. Use Grep tool for exact literals on non-code files. Bash grep/rg only for piping command output.";

const grep = (args: Record<string, unknown>): HostCall => ({
  tool: "grep",
  args,
  output: "Found 1 matches\nsrc/app.ts:\n  Line 1: export function greet",
  metadata: { matches: 1, truncated: false },
});

const bash = (command: string): HostCall => ({
  tool: "bash",
  args: { command, description: "search" },
  output: "src/app.ts:1:export function greet\n",
  metadata: { exit: 0 },
});

test.concurrent("structural and file searches get one nudge; piped, literal and non-code ones none", async () => {
  const p = astGrepProject();
  const structural = await p.call(grep({ pattern: "export function greet", path: p.root }));
  expect(count(structural, ADVISORY)).toBe(1);
  expect(structural).toContain(GREP_STOP);
  const shell = await p.call(bash("rg 'function greet' src"));
  expect(count(shell, ADVISORY)).toBe(1);
  expect(shell).toContain(BASH_STOP);
  const literalFiles = await p.call(bash("rg TODO notes.md"));
  expect(count(literalFiles, ADVISORY)).toBe(1);
  expect(literalFiles).toContain(GENERIC);
  const quiet = await Promise.all(
    [
      grep({ pattern: "export function greet", include: "*.md" }),
      grep({ pattern: "TODO" }),
      bash("git log --oneline | grep init"),
    ].map((c) => p.call(c)),
  );
  for (const result of quiet) expect(result).not.toContain(ADVISORY);
});

test.concurrent("without ast-grep on PATH the structural nudges warn instead", async () => {
  const p = astGrepProject({ astGrep: "missing" });
  expect(await p.call(grep({ pattern: "export function greet" }))).toContain(
    "WARN: structural code pattern detected but ast-grep is not installed.",
  );
  expect(await p.call(bash("rg 'function greet' src"))).toContain(
    "WARN: structural grep/rg detected but ast-grep is not installed.",
  );
});

test.concurrent("the skills.ast-grep opt-out drops structural nudges and the ast-grep half of the generic one", async () => {
  const p = astGrepProject({ astGrep: "opt-out" });
  expect(await p.call(grep({ pattern: "export function greet" }))).not.toContain(ADVISORY);
  const generic = await p.call(bash("rg TODO notes.md"));
  expect(generic).toContain(GENERIC_OPT_OUT);
  expect(generic).not.toContain("ast-grep");
});

test.concurrent("a replayed after delivers no second nudge", async () => {
  const p = astGrepProject();
  const call = grep({ pattern: "export function greet" });
  expect(count(await p.call(call, "dup"), ADVISORY)).toBe(1);
  expect(await p.replay(call, "dup")).not.toContain(ADVISORY);
});
