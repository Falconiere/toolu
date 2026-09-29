/**
 * AC-1 to AC-6 (#262): every push-review, plan-ledger, docs-sync and agent-tier
 * case runs the committed bundle behind its launcher in a fresh sandbox. Where
 * bash was right, the result equals its golden capture: stdout as parsed JSON
 * (the native encoder prints compact JSON where jq printed it pretty), exit
 * code, stderr and every file the hook wrote. Where a case names a deviation,
 * its golden is the known-wrong baseline and the bundle must differ.
 */
import { expect, test } from "bun:test";
import { MAX_SHELL_INPUT } from "@toolu/core/shell";
import { decisionOf } from "./pre-tool-modules-b-cases.ts";
import {
  commitFile,
  comparable,
  featureRepo,
  group,
  runCase,
  type Captured,
  type GateCase,
} from "./pre-tool-modules-c-cases.ts";
import { MODULE_CASES, readGolden } from "./pre-tool-modules-c-golden.ts";

const golden = readGolden();

/** Each case spawns git, bash and Bun; 5 s is too tight on a loaded machine. */
const CASE_TIMEOUT_MS = 60_000;

function expected(c: GateCase): Captured {
  const found = golden.cases[c.name];
  if (found === undefined) throw new Error(`no golden capture for ${c.name}`);
  return found;
}

test("golden covers every case, and case names are unique", () => {
  const names = MODULE_CASES.map((c) => c.name);
  expect(new Set(names).size).toBe(names.length);
  expect(Object.keys(golden.cases).toSorted()).toEqual(names.toSorted());
});

for (const c of MODULE_CASES) {
  test.concurrent(
    c.name,
    async () => {
      const want = expected(c);
      const got = await runCase(c);
      const { outcome, text } = decisionOf(got.stdout);
      expect(outcome).toBe(c.expect);
      for (const needle of c.has ?? []) expect(text).toContain(needle);
      for (const needle of c.lacks ?? []) expect(text).not.toContain(needle);
      if (c.deviation === undefined) expect(comparable(got)).toEqual(comparable(want));
      else expect(comparable(got)).not.toEqual(comparable(want));
    },
    CASE_TIMEOUT_MS,
  );
}

/**
 * Beyond bash: a line over the shell parser's size cap cannot be analyzed, and
 * none of these workflow gates treats it as a push (a large heredoc must not
 * nag). Bash's lexer took minutes on such a line, so there is no golden.
 */
const oversize = group({})({
  name: "workflow gates: an unanalyzable oversize push line is not judged",
  config: { version: 1, gates: { preset: "strict" } },
  setup: (sb) => {
    featureRepo(sb);
    commitFile(sb, "src/tool.ts", "export {};");
  },
  command: `git push # ${"x".repeat(MAX_SHELL_INPUT)}`,
  expect: "silent",
});

test.concurrent(
  oversize.name,
  async () => {
    const { text } = decisionOf((await runCase(oversize)).stdout);
    for (const needle of ["Code review required", "docs-sync:", "plan-ledger:"]) {
      expect(text).not.toContain(needle);
    }
  },
  CASE_TIMEOUT_MS,
);
