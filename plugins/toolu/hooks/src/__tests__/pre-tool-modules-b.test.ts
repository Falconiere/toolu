/**
 * AC-1, AC-2, AC-3, AC-9 (#261): every bash-commands, commit-gate and
 * quality-gate case runs the committed PreToolUse bundle behind its launcher
 * in a fresh sandbox. Where bash was right, the result equals its golden
 * capture: stdout as parsed JSON (the native encoder prints compact JSON where
 * jq printed it pretty), exit code and stderr exact. Where a case names a
 * deviation, its golden is the known-wrong baseline and the bundle must differ.
 */
import { expect, test } from "bun:test";
import { implementationTag, launchedArgv } from "@toolu/conformance/harness/entry-command";
import { MAX_SHELL_INPUT } from "@toolu/core/shell";
import {
  comparable,
  decisionOf,
  FAILING_GATE,
  gateMode,
  group,
  runCase,
  type Captured,
  type ModuleCase,
} from "./pre-tool-modules-b-cases.ts";
import { MODULE_CASES, readGolden } from "./pre-tool-modules-b-golden.ts";

const golden = readGolden();

/** Each case spawns git, bash and Bun; 5 s is too tight on a loaded machine. */
const CASE_TIMEOUT_MS = 60_000;

const IMPL = implementationTag("toolu", "pre-tools");
const bundle = () => launchedArgv({ plugin: "toolu", event: "PreToolUse", entry: "pre-tools" });

function expected(c: ModuleCase): Captured {
  const found = golden.cases[c.name];
  if (found === undefined) throw new Error(`no golden capture for ${c.name}`);
  return found;
}

test("golden covers every case, and case names are unique", () => {
  const names = MODULE_CASES.map((c) => c.name);
  expect(new Set(names).size).toBe(names.length);
  expect(Object.keys(golden.cases).toSorted()).toEqual(names.toSorted());
  expect(MODULE_CASES.some((c) => c.deviation !== undefined)).toBe(true);
});

for (const c of MODULE_CASES) {
  test.concurrent(
    `${c.name}${IMPL}`,
    async () => {
      const want = expected(c);
      const got = await runCase(c, bundle);
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
 * Beyond bash: a line over the shell parser's size cap cannot be analyzed.
 * Bash's lexer took minutes on such a line, so there is no golden; the gates
 * must not fail open on it.
 */
const PAD = ` # ${"x".repeat(MAX_SHELL_INPUT)}`;
const oversize = group({});
const OVERSIZE_CASES: ModuleCase[] = [
  oversize({
    name: "bash-commands: an oversize line asks as a guardrail hit",
    shippedSettings: true,
    config: { version: 1 },
    command: `ls${PAD}`,
    expect: "ask",
    has: ["SECURITY GUARDRAIL", "toolu could not analyze (oversize:"],
  }),
  oversize({
    name: "quality-gate: an oversize line is gated while the gate fails",
    gate: FAILING_GATE,
    config: gateMode("bashCommands", "off"),
    command: `git push${PAD}`,
    expect: "deny",
    has: ["BLOCKED: quality gate failing"],
  }),
  oversize({
    name: "commit-gate: an oversize commit still gets the reminder",
    settings: { "commit-prefixes.txt": "feat\n" },
    command: `git commit -m "wibble: x"${PAD}`,
    expect: "advisory",
    has: ["BEFORE COMMITTING"],
  }),
];

for (const c of OVERSIZE_CASES) {
  test.concurrent(
    `${c.name}${IMPL}`,
    async () => {
      const { outcome, text } = decisionOf((await runCase(c, bundle)).stdout);
      expect(outcome).toBe(c.expect);
      for (const needle of c.has ?? []) expect(text).toContain(needle);
    },
    CASE_TIMEOUT_MS,
  );
}
