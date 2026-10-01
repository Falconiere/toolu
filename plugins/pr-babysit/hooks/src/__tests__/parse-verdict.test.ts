import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { spawnSync } from "node:child_process";
import { parseVerdict, type ReviewVerdict } from "../babysit/verdict";

const ROOT = resolve(import.meta.dir, "../../..");
const FIXTURES = resolve(ROOT, "scripts/__tests__/fixtures");
const ENTRY = resolve(import.meta.dir, "../babysit-parse-verdict.ts");
const GOLDEN = JSON.parse(
  readFileSync(resolve(import.meta.dir, "fixtures/parse-verdict.golden.json"), "utf8"),
) as { recorded: Record<string, ReviewVerdict>; boundary: Record<string, ReviewVerdict> };

const RECORDED = [
  "pr31-verdict.txt",
  "pr120-verdict-changes.txt",
  "pr122-verdict-approved.txt",
  "pr157-must-fix.txt",
  "provider-error.txt",
  "in-progress.txt",
  "no-checkbox.txt",
] as const;

for (const name of RECORDED) {
  test(`captured ${name}: pure parser and CLI match recorded Bash output`, () => {
    const input = readFileSync(resolve(FIXTURES, name), "utf8");
    const expected = GOLDEN.recorded[name]!;
    expect(expected).toBeDefined();
    expect(parseVerdict(input)).toEqual(expected);
    const cli = spawnSync("bun", [ENTRY], { input, encoding: "utf8" });
    expect(cli.status).toBe(0);
    expect(JSON.parse(cli.stdout)).toEqual(expected);
    expect(cli.stdout).toBe(`${JSON.stringify(expected)}\n`);
  });
}

test("captured PR #31 keeps six distinct low-severity finding keys", () => {
  const input = readFileSync(resolve(FIXTURES, "pr31-verdict.txt"), "utf8");
  const result = parseVerdict(input);
  expect(result.state).toBe("complete");
  expect(result.verdict).toBe("approved");
  expect(result.findings).toHaveLength(6);
  expect(new Set(result.findings.map((finding) => finding.key)).size).toBe(6);
  expect(result.findings[0]).toMatchObject({
    path: "plugins/toolu/hooks/session-start.sh",
    line: 17,
    severity: "low",
  });
  expect(result.findings[2]?.line).toBeNull();
});

test("captured provider failure is incomplete despite a changes label", () => {
  const input = readFileSync(resolve(FIXTURES, "provider-error.txt"), "utf8");
  expect(parseVerdict(input)).toMatchObject({
    state: "provider_error",
    complete: false,
    verdict: "changes",
    findings: [],
    must_fix: [],
  });
});

const BOUNDARY_INPUTS = {
  empty: "",
  unrelated: "fyi see actions/runs/999 and agent-merge-foo, thanks",
  "checklist label wins over finding prose": [
    "### Code Review — x",
    "- [x] Set verdict label (`merge-approved`)",
    "### Findings",
    "`a/b.sh:9`: low: the label should be `request-changes` here.",
    "`request-changes`",
  ].join("\n"),
  "standalone chip identifies a review": "some prose\n- [x] done\n`request-changes`",
  "prose verdict fallback":
    "### Code Review — x\n- [x] done\n**Verdict:** ⚠️ Changes requested\n### Findings (0)",
  "finding quotes provider error":
    "### Code Review — x\n**Verdict:** ⚠️ Changes requested\n### Findings (1)\n`a.sh:1`: medium: **Provider error:** is shown on failure\n`request-changes`",
  "Top-N bare and marked sentences":
    "### Code Review — x\n### Top-N must-fix (4)\nFix bare line.\n- Fix dash item.\n* Fix star item.\n1. Fix numbered item.\n<details>\n`request-changes`",
  "approved with Top-N":
    "### Code Review — x\n**Verdict:** ✅ Approved\n### Findings (0)\n### Top-N must-fix\nSomething the bot still wants.\n<details>\n`merge-approved`",
};

for (const [name, input] of Object.entries(BOUNDARY_INPUTS)) {
  test(`recorded Bash boundary parity: ${name}`, () => {
    expect(GOLDEN.boundary[name]).toBeDefined();
    expect(parseVerdict(input)).toEqual(GOLDEN.boundary[name]!);
  });
}
